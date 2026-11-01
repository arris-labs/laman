//! Analytic experiments for ADR-0002/0003, not the production compiler or
//! solver. Expectations come from isolated tangent geometry and C1 construction.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use laman::model::{Constraint, Coordinate, Entity, Sketch};
use laman::residual::{equations, intrinsic};
use laman_debug::{fd, fixture::Fixture};
use std::path::PathBuf;

// Candidate policy, frozen by ADR-0003. Production adoption belongs to M2
// step 2 (shared model constants and independent checker measurements).
const LENGTH_FLOOR: f64 = 1e-12;
const LENGTH_RELATIVE: f64 = 1e-11;
const RANK: f64 = 1e-10;

fn length_tolerance(scale: f64) -> f64 {
    LENGTH_FLOOR.max(LENGTH_RELATIVE * scale)
}

// Twice-reorthogonalized ordered rows: an independent small-matrix rank
// experiment, not the production pivoted QR. Unit row norms remove units.
fn rank(rows: &[Vec<f64>]) -> usize {
    let mut basis: Vec<Vec<f64>> = Vec::new();
    for row in rows {
        let norm = libm::sqrt(row.iter().map(|x| x * x).sum());
        if norm == 0.0 {
            continue;
        }
        let mut v: Vec<_> = row.iter().map(|x| x / norm).collect();
        for _ in 0..2 {
            for q in &basis {
                let dot: f64 = v.iter().zip(q).map(|(a, b)| a * b).sum();
                for (a, b) in v.iter_mut().zip(q) {
                    *a -= dot * b;
                }
            }
        }
        let norm = libm::sqrt(v.iter().map(|x| x * x).sum());
        if norm > RANK {
            basis.push(v.iter().map(|x| x / norm).collect());
        }
    }
    basis.len()
}

// x = [a.x,a.y,b.x,b.y,c.x,c.y]; p is line endpoint a or b.
// h = unit(b-a) dot (p-c), with the full analytic gradient.
fn contact(x: &[f64], end: usize) -> (f64, Vec<f64>) {
    let u = [x[2] - x[0], x[3] - x[1]];
    let len = libm::hypot(u[0], u[1]);
    let t = [u[0] / len, u[1] / len];
    let v = [x[2 * end] - x[4], x[2 * end + 1] - x[5]];
    let h = t[0] * v[0] + t[1] * v[1];
    let w = [(v[0] - t[0] * h) / len, (v[1] - t[1] * h) / len];
    let mut g = vec![-w[0], -w[1], w[0], w[1], -t[0], -t[1]];
    g[2 * end] += t[0];
    g[2 * end + 1] += t[1];
    (h, g)
}

#[test]
fn contact_gradient_includes_both_endpoints_and_centre() {
    for scale in [1e-6, 1.0, 1e3] {
        for offset in [0.0, 1e3] {
            for short in [1.0, 1e-3] {
                for end in [0, 1] {
                    let x: Vec<_> = [0.0, 0.0, short, short, 0.3, 1.0]
                        .iter()
                        .map(|v| scale * (v + offset))
                        .collect();
                    let (_, g) = contact(&x, end);
                    fd::check(
                        |x| contact(x, end).0,
                        &x,
                        &g,
                        fd::Rule::length(scale * short),
                    )
                    .unwrap();
                }
            }
        }
    }
}

#[test]
fn circle_and_both_arc_endpoints_require_membership_and_sense() {
    for sign in [-1.0, 1.0] {
        for arc_end in [None, Some(false), Some(true)] {
            let mut sk = Sketch::new();
            let c = sk.point_fixed(0.0, sign).unwrap();
            let p = sk.point(0.0, 0.0).unwrap();
            let q = sk.point(2.0, 0.0).unwrap();
            let line = sk.line(p, q).unwrap();
            let opposite = sk.point(0.0, 2.0 * sign).unwrap();
            let round: laman::model::Round = match arc_end {
                None => {
                    let circle = sk.circle(c, 1.0).unwrap();
                    sk.point_on_circle(p, circle).unwrap();
                    circle.into()
                }
                Some(false) => sk.arc(c, p, opposite).unwrap().into(),
                Some(true) => sk.arc(c, opposite, p).unwrap().into(),
            };
            let tangent = sk.tangent_line_circle(line, round).unwrap();
            let at = sk.coordinates();
            assert!(laman_check::check(&sk, &at).is_clean());
            assert_eq!(contact(&[0., 0., 2., 0., 0., sign], 0).0, 0.0);
            assert_eq!(rank(&[vec![0., -sign], vec![0., 1.]]), 1);
            assert_eq!(rank(&[vec![0., -sign], vec![1., 0.]]), 2);

            // Reflect the entire geometry: h and circle membership still
            // hold, but the original tangent's stored sense must reject it.
            let mut reflected = at.clone();
            for ((_, co), v) in &mut reflected {
                if *co == Coordinate::Y {
                    *v = -*v;
                }
            }
            assert_eq!(contact(&[0., 0., 2., 0., 0., -sign], 0).0, 0.0);
            let report = laman_check::check(&sk, &reflected);
            assert_eq!(report.violations.len(), 1);
            assert_eq!(
                report.violations[0].id,
                laman_check::Source::Constraint(tangent)
            );
            if arc_end.is_some() {
                // At the start membership is definitional; at the end it
                // needs the intrinsic equation. Neither permits dropping it.
                let mut broken = at;
                broken.insert((opposite.id(), Coordinate::Y), 3.0 * sign);
                let report = laman_check::check(&sk, &broken);
                assert!(
                    report
                        .violations
                        .iter()
                        .any(|v| v.id == laman_check::Source::Arc(round.id()))
                );
            }
        }
    }
}

#[test]
fn c1_original_and_contact_ranks_are_analytically_distinct() {
    for (name, parameters, original_rank) in [
        ("sss-triangle", 4, 4),
        ("belt", 12, 8),
        ("slotted-plate", 22, 18),
    ] {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let f = Fixture::load(&root.join("tests/fixtures/c1").join(name)).unwrap();
        let sk = &f.sketch;
        let mut at = sk.coordinates();
        at.extend(f.expected.coordinates);
        let report = laman_check::check(sk, &at);
        assert!(report.is_clean(), "{name}: {report:?}");
        let keys: Vec<_> = at
            .keys()
            .copied()
            .filter(|(id, c)| !sk.is_fixed(*id, *c))
            .collect();
        assert_eq!(keys.len(), parameters);
        let expand = |eq: &laman::residual::Equation| {
            keys.iter()
                .map(|key| {
                    eq.terms()
                        .iter()
                        .filter(|(k, _)| k == key)
                        .map(|(_, v)| *v)
                        .sum()
                })
                .collect::<Vec<f64>>()
        };
        let mut original = Vec::new();
        let mut regular = Vec::new();
        for (id, _) in sk.entities() {
            if let Some(eq) = intrinsic(sk, &at, id) {
                original.push(expand(&eq));
                regular.push(expand(&eq));
            }
        }
        for (id, c) in sk.constraints() {
            let eqs = equations(sk, &at, id).unwrap();
            original.extend(eqs.as_slice().iter().map(&expand));
            if let Constraint::TangentLineCircle { line, round, .. } = c {
                let Entity::Line { a, b } = sk.entity(line.id()).unwrap() else {
                    panic!("line")
                };
                let (centre, endpoints) = match sk.entity(round.id()).unwrap() {
                    Entity::Circle { c, .. } => (*c, vec![]),
                    Entity::Arc { c, s, e } => (*c, vec![s.id(), e.id()]),
                    Entity::Point { .. } | Entity::Line { .. } => panic!("round"),
                };
                let end = [a, b]
                    .iter()
                    .position(|p| {
                        endpoints.contains(&p.id())
                            || sk.constraints().any(|(_, c)| {
                                matches!(c,Constraint::PointOnCircle {p: q,on}
                        if q.id()==p.id() && on.id()==round.id())
                            })
                    })
                    .unwrap();
                let local_keys: Vec<_> = [a.id(), b.id(), centre.id()]
                    .into_iter()
                    .flat_map(|id| [(id, Coordinate::X), (id, Coordinate::Y)])
                    .collect();
                let x: Vec<_> = local_keys.iter().map(|key| at[key]).collect();
                let (h, g) = contact(&x, end);
                assert!(h.abs() < length_tolerance(120.0));
                regular.push(
                    keys.iter()
                        .map(|key| {
                            local_keys
                                .iter()
                                .zip(&g)
                                .filter(|(k, _)| *k == key)
                                .map(|(_, v)| v)
                                .sum()
                        })
                        .collect(),
                );
            } else {
                regular.extend(eqs.as_slice().iter().map(&expand));
            }
        }
        assert_eq!(rank(&original), original_rank, "{name}");
        assert_eq!(rank(&regular), parameters, "{name}");
        println!(
            "{name}: parameters={parameters}, original rank={}, contact rank={}",
            rank(&original),
            rank(&regular)
        );
        let dir = root.join("target/inspect/m2-contract").join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("expected.svg"),
            laman_debug::dump::svg(sk, &at, &report),
        )
        .unwrap();
        std::fs::write(
            dir.join("expected.txt"),
            laman_debug::dump::text(sk, &at, &report),
        )
        .unwrap();
    }
}

#[test]
fn scale_translation_and_near_contact_have_explicit_resolution_limits() {
    let mut worst = 0.0_f64;
    for scale in [1e-6, 1.0, 1e3, 1e6] {
        for translation in [0.0, 1.0, 1e4] {
            // A translated 3-4-5 radius, evaluated as a distance.
            let origin = translation * scale;
            let dx = (origin + 3.0 * scale) - origin;
            let dy = (origin + 4.0 * scale) - origin;
            let error = (libm::hypot(dx, dy) - 5.0 * scale).abs();
            worst = worst.max(error / length_tolerance(5.0 * scale));
            assert!(error <= length_tolerance(5.0 * scale));
            let displaced = libm::hypot(dx, dy) + 100.0 * length_tolerance(5.0 * scale);
            assert!((displaced - 5.0 * scale).abs() > length_tolerance(5.0 * scale));
        }
    }
    // At a contact the original residual is quadratic in tangential motion.
    // Regularization detects it linearly, even where subtraction rounds away.
    for motion in [1e-3, 1e-6, 1e-9] {
        let original = libm::hypot(1.0, motion) - 1.0;
        let regular = motion;
        assert!(original <= motion * motion);
        assert!(regular > length_tolerance(1.0));
    }
    // A tiny independent direction is resolved above, not below, rank cutoff.
    assert_eq!(rank(&[vec![1., 0.], vec![1., 100.0 * RANK]]), 2);
    assert_eq!(rank(&[vec![1., 0.], vec![1., RANK / 100.0]]), 1);
    for scale in [1e-300, 1e300, f64::MAX] {
        assert!(length_tolerance(scale).is_finite());
        assert!(length_tolerance(scale) > 0.0);
    }
    assert!(1e-300 < length_tolerance(1e-300)); // finite does not imply resolved geometry
    assert_eq!(1e300 + 1.0, 1e300); // translation can destroy an input difference
    println!("largest translated distance error / length tolerance: {worst:e}");
}
