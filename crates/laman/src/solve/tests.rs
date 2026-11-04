#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use super::*;
use crate::model::{Coordinate, EntityId};

use laman_debug::{dump, fixture::Fixture};
use std::path::PathBuf;

const COORDINATE_ERROR: f64 = 1e-9; // Closed-form fixture acceptance, independent of solver thresholds.

/// Every analytic case runs under both engines (plans/m2 step 4).
const ALGORITHMS: [Algorithm; 2] = [Algorithm::LevenbergMarquardt, Algorithm::DogLeg];

fn with(algorithm: Algorithm) -> Options {
    Options {
        algorithm,
        ..Options::default()
    }
}

// Unit tests exercise the private engine. The checker links the normal library,
// so cross the test/library type boundary through the public serde form.
fn fixture_at(sketch: &Sketch, values: &Coordinates) -> Fixture {
    let mut fixture = Fixture::parse(
        &serde_json::to_string(sketch).unwrap(),
        r#"{"status":"UnderConstrained","dof":0,"coordinates":[]}"#,
        None,
    )
    .unwrap();
    fixture.expected.coordinates = fixture.sketch.coordinates();
    assert_eq!(fixture.expected.coordinates.len(), values.len());
    for (to, from) in fixture
        .expected
        .coordinates
        .values_mut()
        .zip(values.values())
    {
        *to = *from;
    }
    fixture
}
fn check(sketch: &Sketch, values: &Coordinates) -> laman_check::Report {
    let f = fixture_at(sketch, values);
    laman_check::check(&f.sketch, &f.expected.coordinates)
}
fn inspect(sketch: &Sketch, out: &Outcome, slug: &str) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/inspect")
        .join(slug);
    std::fs::create_dir_all(&path).unwrap();
    let f = fixture_at(sketch, &out.coordinates);
    let report = laman_check::check(&f.sketch, &f.expected.coordinates);
    std::fs::write(
        path.join("solved.txt"),
        dump::text(&f.sketch, &f.expected.coordinates, &report),
    )
    .unwrap();
    std::fs::write(
        path.join("solved.svg"),
        dump::svg(&f.sketch, &f.expected.coordinates, &report),
    )
    .unwrap();
}
fn constraint_ids(report: laman_check::Report) -> Vec<u32> {
    report
        .violations
        .iter()
        .map(|v| match v.id {
            laman_check::Source::Constraint(id) => id.0,
            laman_check::Source::Arc(_) => panic!("unexpected intrinsic violation"),
        })
        .collect()
}

#[test]
fn analytic_distance_minimum_motion_and_zero_step() {
    for algorithm in ALGORITHMS {
        let mut sk = Sketch::new();
        let a = sk.point(0.0, 0.0).unwrap();
        let b = sk.point(3.0, 4.0).unwrap();
        sk.distance(a, b, 10.0).unwrap();
        let stray = sk.point(-0.0, 200.0).unwrap();
        let out = solve(&sk, with(algorithm));
        assert!(out.converged, "{out:?}");
        assert!(check(&sk, &out.coordinates).is_clean());
        for (id, xy) in [(a.id(), [-1.5, -2.0]), (b.id(), [4.5, 6.0])] {
            for (c, v) in [Coordinate::X, Coordinate::Y].into_iter().zip(xy) {
                assert!(
                    (out.coordinates[&(id, c)] - v).abs() < COORDINATE_ERROR,
                    "{algorithm:?} {out:?}"
                );
            }
        }
        assert_eq!(
            out.coordinates[&(stray.id(), Coordinate::X)].to_bits(),
            (-0.0f64).to_bits()
        );
        inspect(&sk, &out, &format!("{algorithm:?}-distance"));
        let mut solved = Sketch::new();
        let a = solved.point(-1.5, -2.0).unwrap();
        let b = solved.point(4.5, 6.0).unwrap();
        solved.distance(a, b, 10.0).unwrap();
        let out = solve(&solved, with(algorithm));
        assert!(out.converged);
        assert!(check(&solved, &out.coordinates).is_clean());
        assert_eq!(out.iterations, 0);
        assert_eq!(out.coordinates, solved.coordinates());
    }
}

#[test]
fn c1_contacts_from_guesses() {
    for algorithm in ALGORITHMS {
        for slug in ["sss-triangle", "belt", "slotted-plate"] {
            let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/c1")
                .join(slug);
            let fixture = Fixture::load(&path).unwrap();
            let sk: Sketch =
                serde_json::from_str(&serde_json::to_string(&fixture.sketch).unwrap()).unwrap();
            let out = solve(&sk, with(algorithm));
            inspect(&sk, &out, &format!("{algorithm:?}-{slug}"));
            eprintln!(
                "{algorithm:?} {slug}: iterations={} evaluations={} rejected={}",
                out.iterations, out.evaluations, out.rejected
            );
            assert!(out.converged, "{slug}: {out:?}");
            assert!(
                check(&sk, &out.coordinates).is_clean(),
                "{slug}: {:?}",
                check(&sk, &out.coordinates)
            );
            let solved = reseed(&sk, &out.coordinates);
            let again = solve(&solved, with(algorithm));
            assert!(again.converged);
            assert!(check(&solved, &again.coordinates).is_clean());
            assert_eq!(again.iterations, 0);
            assert_eq!(
                out.coordinates
                    .values()
                    .map(|x| x.to_bits())
                    .collect::<Vec<_>>(),
                again
                    .coordinates
                    .values()
                    .map(|x| x.to_bits())
                    .collect::<Vec<_>>()
            );
            let public = fixture_at(&sk, &out.coordinates);
            for (key, want) in &fixture.expected.coordinates {
                assert!(
                    (public.expected.coordinates[key] - want).abs() <= fixture.expected.tolerance,
                    "{slug}: {key:?}: {} != {want}",
                    public.expected.coordinates[key]
                );
            }
        }
    }
}

/// Fixed o = (0, 0) and q = (10, 0); p at distance 6 from o and 8 from q,
/// so p = (3.6, ±4.8) (a 6-8-10 right triangle), guessed at (x, y).
fn trilateration(x: f64, y: f64) -> (Sketch, crate::model::Point, [u32; 2]) {
    let mut sk = Sketch::new();
    let o = sk.point_fixed(0.0, 0.0).unwrap();
    let q = sk.point_fixed(10.0, 0.0).unwrap();
    let p = sk.point(x, y).unwrap();
    let a = sk.distance(o, p, 6.0).unwrap();
    let b = sk.distance(q, p, 8.0).unwrap();
    (sk, p, [a.0, b.0])
}

fn residual_norm(sk: &Sketch, values: &Coordinates, p: crate::model::Point) -> f64 {
    let (x, y) = (
        values[&(p.id(), Coordinate::X)],
        values[&(p.id(), Coordinate::Y)],
    );
    let _ = sk;
    libm::hypot(libm::hypot(x, y) - 6.0, libm::hypot(x - 10.0, y) - 8.0)
}

#[test]
fn budgets_fixed_only_and_finite_extremes() {
    for algorithm in ALGORITHMS {
        // Trilateration, nonlinear for both engines: from (3, 4) no single
        // step reaches the construction's (3.6, 4.8), so a two-evaluation
        // budget (the initial one and one trial) is exhausted, not met.
        let (sk, p, ids) = trilateration(3.0, 4.0);
        let start = residual_norm(&sk, &sk.coordinates(), p);
        for options in [
            Options {
                max_iterations: 0,
                ..with(algorithm)
            },
            Options {
                max_evaluations: 0,
                ..with(algorithm)
            },
            Options {
                max_evaluations: 2,
                ..with(algorithm)
            },
        ] {
            let out = solve(&sk, options);
            assert!(!out.converged, "{algorithm:?} {options:?}");
            assert!(out.evaluations <= options.max_evaluations);
            assert!(out.iterations <= options.max_iterations);
            let flagged = constraint_ids(check(&sk, &out.coordinates));
            assert!(!flagged.is_empty() && flagged.iter().all(|id| ids.contains(id)));
            assert!(out.coordinates.values().all(|x| x.is_finite()));
            // Best effort is never worse than the start.
            assert!(residual_norm(&sk, &out.coordinates, p) <= start);
        }
        for scale in [1e-300, 1e300] {
            let mut sk = Sketch::new();
            let a = sk.point_fixed(0.0, 0.0).unwrap();
            let b = sk.point(3.0 * scale, 4.0 * scale).unwrap();
            sk.distance(a, b, 5.0 * scale).unwrap();
            let out = solve(&sk, with(algorithm));
            assert!(out.converged);
            assert!(check(&sk, &out.coordinates).is_clean());
            assert_eq!(out.iterations, 0);
        }
        let mut sk = Sketch::new();
        let a = sk.point_fixed(-f64::MAX, 0.0).unwrap();
        let b = sk.point_fixed(f64::MAX, 0.0).unwrap();
        let id = sk.distance(a, b, 1.0).unwrap();
        let out = solve(&sk, with(algorithm));
        assert!(!out.converged);
        assert_eq!(out.coordinates, sk.coordinates());
        assert_eq!(constraint_ids(check(&sk, &out.coordinates)), [id.0]);
        let out = solve(&Sketch::new(), with(algorithm));
        assert!(out.converged);
        assert!(check(&Sketch::new(), &out.coordinates).is_clean());
    }
}

#[test]
fn rejects_steps_and_retains_finite_best_effort() {
    for algorithm in ALGORITHMS {
        let mut sk = Sketch::new();
        let p = sk.point(0.0, 0.0).unwrap();
        let a = sk.fix(p, 1.0, 0.0).unwrap();
        let b = sk.fix(p, 2.0, 0.0).unwrap();
        let out = solve(&sk, with(algorithm));
        eprintln!(
            "{algorithm:?} conflicting fixes: iterations={} evaluations={} rejected={}",
            out.iterations, out.evaluations, out.rejected
        );
        assert!(!out.converged);
        assert!(out.rejected > 0);
        assert!(out.iterations <= MAX_ITERATIONS);
        assert!(
            (out.coordinates[&(p.id(), Coordinate::X)] - 1.5).abs() < COORDINATE_ERROR,
            "{:?}",
            out.coordinates
        );
        assert_eq!(constraint_ids(check(&sk, &out.coordinates)), [a.0, b.0]);
        inspect(&sk, &out, &format!("{algorithm:?}-conflicting-fixes"));
    }
}

#[test]
fn damped_step_has_no_null_motion() {
    let e = Evaluation {
        values: vec![-2.0],
        jacobian: vec![vec![1.0, 1.0]],
        valid: true,
        satisfied: false,
    };
    let step = damped_step(&e, INITIAL_DAMPING).unwrap();
    assert!((step[0] - step[1]).abs() < COORDINATE_ERROR);
    assert!((step[0] - 2.0 / (2.0 + INITIAL_DAMPING)).abs() < COORDINATE_ERROR);
    // Matrix-level test does not solve a sketch; geometry checks are above.
}

#[test]
fn stopping_regression() {
    for algorithm in ALGORITHMS {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/regression/lm-coupled-stopping");
        let f = Fixture::load(&path).unwrap();
        let sk: Sketch = serde_json::from_str(&serde_json::to_string(&f.sketch).unwrap()).unwrap();
        let out = solve(&sk, with(algorithm));
        assert!(out.converged);
        assert!(check(&sk, &out.coordinates).is_clean());
        let actual = fixture_at(&sk, &out.coordinates);
        for (key, want) in &f.expected.coordinates {
            assert!((actual.expected.coordinates[key] - want).abs() <= f.expected.tolerance);
        }
    }
}

fn reseed(sk: &Sketch, values: &Coordinates) -> Sketch {
    let mut form = serde_json::to_value(sk).unwrap();
    for entity in form["entities"].as_array_mut().unwrap() {
        let id = crate::model::EntityId(entity["id"].as_u64().unwrap() as u32);
        for (coordinate, key) in [
            (Coordinate::X, "x"),
            (Coordinate::Y, "y"),
            (Coordinate::R, "r"),
        ] {
            if let Some(value) = values.get(&(id, coordinate)) {
                entity[key] = serde_json::json!(value);
            }
        }
    }
    serde_json::from_value(form).unwrap()
}

#[test]
fn original_tangent_sense_is_required() {
    for algorithm in ALGORITHMS {
        use crate::model::{Constraint, Sense};
        let mut sk = Sketch::new();
        let c = sk.point_fixed(0.0, 1.0).unwrap();
        let p = sk.point(0.0, 0.0).unwrap();
        let q = sk.point(2.0, 0.0).unwrap();
        let line = sk.line(p, q).unwrap();
        let circle = sk.circle(c, 1.0).unwrap();
        sk.point_on_circle(p, circle).unwrap();
        let id = sk
            .add_constraint(Constraint::TangentLineCircle {
                line,
                round: circle.into(),
                sense: Sense::Negative,
            })
            .unwrap();
        let out = solve(&sk, with(algorithm));
        assert!(!out.converged);
        assert_eq!(out.coordinates, sk.coordinates());
        assert_eq!(constraint_ids(check(&sk, &out.coordinates)), [id.0]);
        inspect(&sk, &out, &format!("{algorithm:?}-wrong-sense"));
    }
}

#[test]
fn extreme_nonzero_steps_and_nonfinite_transform() {
    for algorithm in ALGORITHMS {
        let mut sk = Sketch::new();
        let a = sk.point_fixed(0.0, 0.0).unwrap();
        let b = sk.point(3e300, 4e300).unwrap();
        sk.distance(a, b, 1e301).unwrap();
        let out = solve(&sk, with(algorithm));
        assert!(out.converged);
        assert!(out.iterations > 0);
        assert!(check(&sk, &out.coordinates).is_clean());
        assert!(out.coordinates.values().all(|x| x.is_finite()));
        let mut sk = Sketch::new();
        let a = sk.point_fixed(-f64::MAX, 0.0).unwrap();
        let b = sk.point(f64::MAX, 0.0).unwrap();
        let id = sk.distance(a, b, 1.0).unwrap();
        let out = solve(&sk, with(algorithm));
        assert!(!out.converged);
        assert_eq!(out.coordinates, sk.coordinates());
        assert_eq!(constraint_ids(check(&sk, &out.coordinates)), [id.0]);
    }
}

#[test]
fn vector_acceptance_is_not_two_scalar_acceptances() {
    for algorithm in ALGORITHMS {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/regression/lm-vector-stop");
        let f = Fixture::load(&path).unwrap();
        let sk: Sketch = serde_json::from_str(&serde_json::to_string(&f.sketch).unwrap()).unwrap();
        let out = solve(&sk, with(algorithm));
        assert!(out.converged);
        assert!(check(&sk, &out.coordinates).is_clean());
        assert!(out.iterations > 0);
    }
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config::with_cases(128))]
    #[test]
    fn radial_construction_property(x in 1.0f64..10.0, y in 1.0f64..10.0,
                                    multiplier in 0.5f64..2.0, exponent in -6i32..7) {
        let scale=libm::pow(10.0,exponent as f64);
        let mut sk=Sketch::new();
        let a=sk.point_fixed(0.0,0.0).unwrap();
        let b=sk.point(x*scale,y*scale).unwrap();
        sk.distance(a,b,multiplier*libm::hypot(x*scale,y*scale)).unwrap();
        for algorithm in ALGORITHMS {
        let out=solve(&sk,with(algorithm));
        proptest::prop_assert!(out.converged);
        proptest::prop_assert!(check(&sk,&out.coordinates).is_clean());
        for (coordinate,want) in [(Coordinate::X,x*scale*multiplier),(Coordinate::Y,y*scale*multiplier)] {
            proptest::prop_assert!((out.coordinates[&(b.id(),coordinate)]-want).abs()<COORDINATE_ERROR*scale.max(1.0));
        }
        }
    }
}

#[test]
fn far_guess_does_not_loosen_acceptance() {
    for algorithm in ALGORITHMS {
        // check-guess-scale: a length-1 distance guessed 1e6 away. The frozen
        // input scale would accept a 1e-6 error; acceptance at the iterate's own
        // scale must reach the construction's (1, 0) as the checker measures it.
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/regression/check-guess-scale");
        let f = Fixture::load(&path).unwrap();
        let sk: Sketch = serde_json::from_str(&serde_json::to_string(&f.sketch).unwrap()).unwrap();
        let out = solve(&sk, with(algorithm));
        assert!(out.converged, "{out:?}");
        assert!(check(&sk, &out.coordinates).is_clean());
        let actual = fixture_at(&sk, &out.coordinates);
        for (key, want) in &f.expected.coordinates {
            let got = actual.expected.coordinates[key];
            assert!(
                (got - want).abs() <= f.expected.tolerance,
                "{key:?}: {got} != {want}"
            );
        }
        inspect(&sk, &out, &format!("{algorithm:?}-check-guess-scale"));
    }
}

#[test]
fn trust_region_rejects_overshoot() {
    // From (3.6, 1e-3), p is nearly on the line through o and q, where both
    // distance gradients are almost horizontal: the Gauss–Newton step is
    // about 1e3 long in y. A trust region of 1e3 component scales admits it,
    // the objective grows, and the step is rejected until the region fits.
    let (sk, p, _) = trilateration(3.6, 1e-3);
    let options = Options {
        algorithm: Algorithm::DogLeg,
        initial_radius: 1e3,
        ..Options::default()
    };
    let out = solve(&sk, options);
    eprintln!(
        "overshoot: iterations={} evaluations={} rejected={}",
        out.iterations, out.evaluations, out.rejected
    );
    assert!(out.converged, "{out:?}");
    assert!(out.rejected > 0);
    assert!(check(&sk, &out.coordinates).is_clean());
    // The branch the guess is on: y > 0.
    for (c, want) in [(Coordinate::X, 3.6), (Coordinate::Y, 4.8)] {
        assert!((out.coordinates[&(p.id(), c)] - want).abs() < COORDINATE_ERROR);
    }
    for algorithm in ALGORITHMS {
        let out = solve(&sk, with(algorithm));
        assert!(out.converged, "{algorithm:?} {out:?}");
        assert!(check(&sk, &out.coordinates).is_clean());
        assert!((out.coordinates[&(p.id(), Coordinate::Y)] - 4.8).abs() < COORDINATE_ERROR);
    }
}

#[test]
fn singular_start_is_stagnation_not_convergence() {
    // At (4, 0) both residuals are -2 (|p| = 4 against 6, |p - q| = 6 against
    // 8) and the two gradients are opposite and horizontal: J J^T is singular,
    // J^T f = 0 and neither branch is preferred. No step can be justified;
    // the start is returned bit for bit as best effort.
    let (sk, _, ids) = trilateration(4.0, 0.0);
    for algorithm in ALGORITHMS {
        let out = solve(&sk, with(algorithm));
        assert!(!out.converged, "{algorithm:?}");
        assert_eq!(out.coordinates, sk.coordinates());
        assert!(out.iterations <= MAX_ITERATIONS);
        assert_eq!(constraint_ids(check(&sk, &out.coordinates)), ids);
    }
}

#[test]
fn dog_leg_branches_stay_in_the_region() {
    // f = (1, 1), J = diag(1, 10): GN step (-1, -0.1), length 1.005;
    // g = (1, 10), Cauchy step length |g|^3 / |Jg|^2 = 101^1.5 / 10001 ≈ 0.1005.
    let e = Evaluation {
        values: vec![1.0, 1.0],
        jacobian: vec![vec![1.0, 0.0], vec![0.0, 10.0]],
        valid: true,
        satisfied: false,
    };
    let gn = gauss_newton(&e, GN_CUTOFF).unwrap();
    for radius in [2.0, 0.5, 0.05] {
        let h = dog_leg(&e, radius).unwrap();
        let length = norm(&h);
        if radius > norm(&gn) {
            assert!(
                (h[0] - gn[0]).abs() < COORDINATE_ERROR && (h[1] - gn[1]).abs() < COORDINATE_ERROR
            );
        } else {
            assert!(
                (length - radius).abs() < COORDINATE_ERROR * radius,
                "{radius}: {length}"
            );
        }
        assert!(predicted_decrease(&e, &h) > 0.0);
    }
    // Below the Cauchy length the step is steepest descent: parallel to -g.
    let h = dog_leg(&e, 0.05).unwrap();
    assert!((h[1] / h[0] - 10.0).abs() < COORDINATE_ERROR);
    // A rank-deficient Jacobian: two identical rows (two fixes of one x).
    // The pseudo-inverse lands on their midpoint and moves nothing outside
    // the row space (y untouched).
    let e = Evaluation {
        values: vec![-1.0, -2.0],
        jacobian: vec![vec![1.0, 0.0], vec![1.0, 0.0]],
        valid: true,
        satisfied: false,
    };
    let h = gauss_newton(&e, GN_CUTOFF).unwrap();
    const ROUNDING: f64 = 1e-15; // a few ulps of an order-one step
    assert!(h[1].abs() <= ROUNDING, "{h:?}");
    assert!((h[0] - 1.5).abs() <= ROUNDING, "{h:?}");
}

#[test]
fn c1_twins_under_both_engines() {
    // Redundant twins are consistent: they solve to their base's coordinates.
    // Conflicting twins cannot: finite best effort, not converged, and the
    // checker flags something. Conflict sets are step 6's.
    for slug in [
        "sss-triangle-redundant",
        "belt-redundant",
        "slotted-plate-redundant",
        "sss-triangle-conflicting",
        "belt-conflicting",
        "slotted-plate-conflicting",
    ] {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/c1")
            .join(slug);
        let fixture = Fixture::load(&path).unwrap();
        let sk: Sketch =
            serde_json::from_str(&serde_json::to_string(&fixture.sketch).unwrap()).unwrap();
        for algorithm in ALGORITHMS {
            let out = solve(&sk, with(algorithm));
            eprintln!(
                "{algorithm:?} {slug}: converged={} iterations={} evaluations={} rejected={}",
                out.converged, out.iterations, out.evaluations, out.rejected
            );
            assert!(out.coordinates.values().all(|x| x.is_finite()));
            let report = check(&sk, &out.coordinates);
            if slug.ends_with("redundant") {
                assert!(out.converged, "{algorithm:?} {slug}");
                assert!(report.is_clean(), "{algorithm:?} {slug}: {report:?}");
                let public = fixture_at(&sk, &out.coordinates);
                for (key, want) in &fixture.expected.coordinates {
                    assert!(
                        (public.expected.coordinates[key] - want).abs()
                            <= fixture.expected.tolerance,
                        "{algorithm:?} {slug}: {key:?}"
                    );
                }
            } else {
                assert!(!out.converged, "{algorithm:?} {slug}");
                assert!(!report.violations.is_empty());
            }
        }
    }
}

/// A coordinate of the other `laman` (the one `laman-debug` links) in this
/// crate's types; its Debug names it.
fn ours(id: u32, coordinate: &dyn std::fmt::Debug) -> (EntityId, Coordinate) {
    let coordinate = match format!("{coordinate:?}").as_str() {
        "X" => Coordinate::X,
        "Y" => Coordinate::Y,
        "R" => Coordinate::R,
        other => panic!("unknown coordinate {other}"),
    };
    (EntityId(id), coordinate)
}

fn goal_at(p: crate::model::Point, [x, y]: [f64; 2]) -> [Goal; 2] {
    [
        Goal {
            param: (p.id(), Coordinate::X),
            target: x,
        },
        Goal {
            param: (p.id(), Coordinate::Y),
            target: y,
        },
    ]
}

/// ROADMAP fixture 4 on the driver: from the closed-form "Before", each of
/// the ten targets is pursued from the previous step's solution (the warm
/// start a drag session makes). The plate's width follows the target's x;
/// its y is unreachable (`height` and `top` hold it), so the goal ends
/// stationary, converged. `P3` stays, `Q` keeps its bits, the checker is
/// clean at every step, under both engines.
#[test]
fn goal_drags_the_slotted_plate_along_its_width() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/c1/slotted-plate-drag");
    let fixture = Fixture::load(&path).unwrap();
    let drag = fixture.drag.as_ref().unwrap();
    let mut base: Sketch =
        serde_json::from_str(&serde_json::to_string(&fixture.sketch).unwrap()).unwrap();
    base.place(
        &fixture
            .expected
            .coordinates
            .iter()
            .map(|(k, v)| (ours(k.0.0, &k.1), *v))
            .collect(),
    );
    let p2 = base.point_handle(EntityId(drag.dragged.0)).unwrap();
    let q = EntityId(21);
    for algorithm in ALGORITHMS {
        let mut sk = base.clone();
        let mut counts = vec![];
        for (k, (target, after)) in drag.targets.iter().zip(&drag.after).enumerate() {
            let out = solve_toward(&sk, &goal_at(p2, *target), with(algorithm));
            assert!(out.converged, "{algorithm:?} step {}", k + 1);
            assert!(
                check(&sk, &out.coordinates).is_clean(),
                "{algorithm:?} step {}",
                k + 1
            );
            for (key, want) in &after.coordinates {
                let got = out.coordinates[&ours(key.0.0, &key.1)];
                assert!(
                    (got - want).abs() <= fixture.expected.tolerance,
                    "{algorithm:?} step {}: {key:?} {got} != {want}",
                    k + 1
                );
            }
            for c in [Coordinate::X, Coordinate::Y] {
                assert_eq!(
                    out.coordinates[&(q, c)].to_bits(),
                    base.coordinates()[&(q, c)].to_bits()
                );
            }
            counts.push((out.iterations, out.evaluations, out.rejected));
            sk.place(&out.coordinates);
        }
        eprintln!(
            "{algorithm:?} fixture 4 per step (iterations, evaluations, rejected): {counts:?}"
        );
    }
}

/// A target off a circle the point is held to is unreachable: the goal ends
/// stationary at the closed-form nearest point, `c + r (t − c) / |t − c|`,
/// converged, the circle held.
#[test]
fn unreachable_goal_converges_stationary_at_the_nearest_point() {
    for algorithm in ALGORITHMS {
        for (start, target) in [
            ([10.0, 0.0], [30.0, 40.0]),
            ([10.0, 0.0], [0.0, 1.0]),
            ([0.0, -10.0], [-3.0, 0.5]),
        ] {
            let mut sk = Sketch::new();
            let c = sk.point_fixed(0.0, 0.0).unwrap();
            let p = sk.point(start[0], start[1]).unwrap();
            sk.distance(c, p, 10.0).unwrap();
            let out = solve_toward(&sk, &goal_at(p, target), with(algorithm));
            eprintln!(
                "{algorithm:?} circle {target:?}: iterations={} evaluations={} rejected={}",
                out.iterations, out.evaluations, out.rejected
            );
            assert!(out.converged, "{algorithm:?} {target:?}");
            assert!(check(&sk, &out.coordinates).is_clean());
            let d = libm::hypot(target[0], target[1]);
            // At stationarity the first-order distance left is within the
            // polishing target of length acceptance (1e-12 of the scale here)
            // plus GOAL_ROUNDING of |t − nearest|; it moves the point by at
            // most r / |r − d| of itself, under 10 / 9 for these targets.
            let bound = COORDINATE_ERROR;
            for (coordinate, want) in [
                (Coordinate::X, 10.0 * target[0] / d),
                (Coordinate::Y, 10.0 * target[1] / d),
            ] {
                let got = out.coordinates[&(p.id(), coordinate)];
                assert!(
                    (got - want).abs() <= bound,
                    "{algorithm:?} {target:?} {got} {want}"
                );
            }
        }
    }
}

/// A free point with no constraint reaches its target; with x fixed it
/// moves along y only; a goal already met takes no step and keeps every
/// bit; a goal elsewhere leaves other components' bits alone.
#[test]
fn goals_on_free_fixed_and_met_points() {
    for algorithm in ALGORITHMS {
        let mut sk = Sketch::new();
        let p = sk.point(1.0, 2.0).unwrap();
        let other = sk.point(-0.0, 7.0).unwrap();
        let out = solve_toward(&sk, &goal_at(p, [4.0, -6.0]), with(algorithm));
        assert!(out.converged);
        assert!((out.coordinates[&(p.id(), Coordinate::X)] - 4.0).abs() < COORDINATE_ERROR);
        assert!((out.coordinates[&(p.id(), Coordinate::Y)] + 6.0).abs() < COORDINATE_ERROR);
        assert!(out.coordinates[&(other.id(), Coordinate::X)].is_sign_negative());

        sk.set_fixed(p.id(), Coordinate::X, true).unwrap();
        let out = solve_toward(&sk, &goal_at(p, [4.0, -6.0]), with(algorithm));
        assert!(out.converged);
        assert_eq!(out.coordinates[&(p.id(), Coordinate::X)], 1.0);
        assert!((out.coordinates[&(p.id(), Coordinate::Y)] + 6.0).abs() < COORDINATE_ERROR);

        let out = solve_toward(&sk, &goal_at(p, [1.0, 2.0]), with(algorithm));
        assert!(out.converged);
        assert_eq!((out.iterations, out.evaluations), (0, 1));
        assert_eq!(out.coordinates, sk.coordinates());

        // One goal on a point free in both: the other coordinate stays.
        let q = sk.point(3.0, 5.0).unwrap();
        let only_x = [Goal {
            param: (q.id(), Coordinate::X),
            target: -2.0,
        }];
        let out = solve_toward(&sk, &only_x, with(algorithm));
        assert!(out.converged);
        assert!((out.coordinates[&(q.id(), Coordinate::X)] + 2.0).abs() < COORDINATE_ERROR);
        assert_eq!(out.coordinates[&(q.id(), Coordinate::Y)], 5.0);
    }
}

/// A free point frozen 3.8e74 away from its target, its component at the
/// length floor (found by `any_point_of_any_sketch_drags_to_any_targets`):
/// the first exact step lands on 0, within rounding of the origin, and every
/// later step rounds away against it, so the goal's objective neither falls
/// nor rises. Such a step is no progress: the pursuit stops, not converged,
/// instead of accepting it until the iteration budget is spent.
#[test]
fn a_goal_step_lost_to_rounding_ends_the_pursuit() {
    for algorithm in ALGORITHMS {
        let mut sk = Sketch::new();
        let p = sk
            .point(-22.212_764_685_787_416, 3.830_903_802_466_835_6e74)
            .unwrap();
        let goal = [Goal {
            param: (p.id(), Coordinate::Y),
            target: 683.049_825_835_683_7,
        }];
        let out = solve_toward(&sk, &goal, with(algorithm));
        assert!(!out.converged, "{algorithm:?}: {out:?}");
        assert!(out.iterations <= 8, "{algorithm:?}: {out:?}");
        assert!(out.coordinates.values().all(|v| v.is_finite()));
    }
}

/// A goal on a component whose required equations cannot hold is not
/// pursued: the component reports not converged as without the goal, with
/// the same coordinates.
#[test]
fn goal_on_an_unsatisfiable_component_is_not_pursued() {
    for algorithm in ALGORITHMS {
        let mut sk = Sketch::new();
        let p = sk.point(0.0, 0.0).unwrap();
        sk.fix(p, 1.0, 0.0).unwrap();
        sk.fix(p, 2.0, 0.0).unwrap();
        let plain = solve(&sk, with(algorithm));
        let out = solve_toward(&sk, &goal_at(p, [5.0, 5.0]), with(algorithm));
        assert!(!out.converged);
        assert_eq!(out.coordinates, plain.coordinates);
    }
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config::with_cases(128))]
    /// A two-bar linkage built satisfied (ground `O`, `A` at `r1` from it, `B`
    /// at `r2` from `A`), its tip dragged to any target near the drawing: the
    /// goal never leaves a required residual over acceptance (the checker is
    /// clean), and the drag converges, met or stationary.
    #[test]
    fn a_goal_never_leaves_a_required_residual(
        r1 in 1.0f64..10.0, r2 in 1.0f64..10.0, a1 in -3.0f64..3.0, a2 in -3.0f64..3.0,
        dx in -5.0f64..5.0, dy in -5.0f64..5.0, exponent in -3i32..4,
    ) {
        let scale = libm::pow(10.0, exponent as f64);
        let mut sk = Sketch::new();
        let o = sk.point_fixed(0.0, 0.0).unwrap();
        let a = [scale * r1 * libm::cos(a1), scale * r1 * libm::sin(a1)];
        let b = [a[0] + scale * r2 * libm::cos(a2), a[1] + scale * r2 * libm::sin(a2)];
        let pa = sk.point(a[0], a[1]).unwrap();
        let pb = sk.point(b[0], b[1]).unwrap();
        sk.distance(o, pa, scale * r1).unwrap();
        sk.distance(pa, pb, scale * r2).unwrap();
        let target = [b[0] + scale * dx, b[1] + scale * dy];
        for algorithm in ALGORITHMS {
            let out = solve_toward(&sk, &goal_at(pb, target), with(algorithm));
            proptest::prop_assert!(check(&sk, &out.coordinates).is_clean());
            proptest::prop_assert!(out.converged, "{algorithm:?}: {out:?}");
        }
    }
}

/// `a` fixed, `b` three below it on a vertical, `c` one from `b`: stages
/// `b.x` (the vertical), `b.y` (the distance), then `c`, the remainder or,
/// with `far`, a stage whose distance from `a` is ten, which no `c` one from
/// `b` can meet.
fn two_stages(far: bool) -> (Sketch, crate::model::Point, crate::model::Point) {
    let mut sk = Sketch::new();
    let a = sk.point_fixed(0.0, 0.0).unwrap();
    let b = sk.point(0.3, -2.5).unwrap();
    let c = sk.point(1.2, -3.4).unwrap();
    let l = sk.line(a, b).unwrap();
    sk.vertical(l).unwrap();
    sk.distance(a, b, 3.0).unwrap();
    sk.distance(b, c, 1.0).unwrap();
    if far {
        sk.distance(a, c, 10.0).unwrap();
    }
    (sk, b, c)
}

#[test]
fn a_failed_part_sends_its_component_to_the_whole_solve_from_its_start() {
    let (sk, _, _) = two_stages(true);
    let system = System::new(&sk);
    assert!(parts_of(&system, &system.components[0]).is_some_and(|p| p.len() == 3));
    for algorithm in ALGORITHMS {
        let staged = solve(&sk, with(algorithm));
        let whole = solve(
            &sk,
            Options {
                path: Path::Whole,
                ..with(algorithm)
            },
        );
        assert!(!staged.converged, "{algorithm:?}");
        // Coordinates and counters as the whole solve's: the staged attempt
        // is forgotten, its budget restored.
        let bits =
            |o: &Outcome| -> Vec<u64> { o.coordinates.values().map(|v| v.to_bits()).collect() };
        assert_eq!(bits(&staged), bits(&whole), "{algorithm:?}");
        assert_eq!(
            (staged.iterations, staged.evaluations, staged.rejected),
            (whole.iterations, whole.evaluations, whole.rejected),
            "{algorithm:?}"
        );
    }
}

#[test]
fn a_goal_on_an_earlier_part_is_stationary_and_one_on_the_last_is_pursued() {
    let (sk, b, c) = two_stages(false);
    let system = System::new(&sk);
    let parts = parts_of(&system, &system.components[0]).unwrap();
    assert_eq!(parts.len(), 3);
    assert!(parts[2].params.contains(&(c.id(), Coordinate::X)));
    for algorithm in ALGORITHMS {
        let plain = solve(&sk, with(algorithm));
        assert!(plain.converged, "{algorithm:?}");
        // `b` is determined: a goal on it moves nothing.
        let held = solve_toward(&sk, &goal_at(b, [5.0, 5.0]), with(algorithm));
        assert!(held.converged, "{algorithm:?}");
        let bits =
            |o: &Outcome| -> Vec<u64> { o.coordinates.values().map(|v| v.to_bits()).collect() };
        assert_eq!(bits(&held), bits(&plain), "{algorithm:?}");
        // `c` swings on its circle about `b = (0, -3)` toward the target,
        // and `b` keeps its bits.
        let swung = solve_toward(&sk, &goal_at(c, [0.0, 5.0]), with(algorithm));
        assert!(swung.converged, "{algorithm:?}");
        for k in [Coordinate::X, Coordinate::Y] {
            let key = (b.id(), k);
            assert_eq!(
                swung.coordinates[&key].to_bits(),
                plain.coordinates[&key].to_bits()
            );
        }
        let (x, y) = (
            swung.coordinates[&(c.id(), Coordinate::X)],
            swung.coordinates[&(c.id(), Coordinate::Y)],
        );
        assert!(
            x.abs() <= COORDINATE_ERROR && (y + 2.0).abs() <= COORDINATE_ERROR,
            "{algorithm:?}: {x} {y}"
        );
    }
}

#[test]
fn kept_decompositions_change_no_bit_of_a_solve_or_its_analysis() {
    let (sk, _, c) = two_stages(false);
    let kept = crate::stage::Decompositions::of(&System::new(&sk));
    for algorithm in ALGORITHMS {
        let bits =
            |o: &Outcome| -> Vec<u64> { o.coordinates.values().map(|v| v.to_bits()).collect() };
        let goals = goal_at(c, [0.0, 5.0]);
        let fresh = solve_toward(&sk, &goals, with(algorithm));
        let reused = solve_kept(&sk, &goals, with(algorithm), Some(&kept));
        assert_eq!(bits(&fresh), bits(&reused), "{algorithm:?}");
        assert_eq!(
            (fresh.iterations, fresh.evaluations, fresh.rejected),
            (reused.iterations, reused.evaluations, reused.rejected),
            "{algorithm:?}"
        );
        let options = with(algorithm);
        assert_eq!(
            crate::diagnose::analyse(&sk, &fresh, &options),
            crate::diagnose::analyse_kept(&sk, &reused, &options, Some(&kept)),
            "{algorithm:?}"
        );
    }
}

/// A stage that held before its parents moved is continued with them
/// (`continued`): two pinned parents carried 100 above a point on both their
/// circles leave it below the line through them, where a descent from where
/// it stood finds the mirror root; the continuation translates it with them,
/// the construction's answer. The parents' bits are their own stages'.
#[test]
fn a_stage_moved_by_its_parents_keeps_its_branch() {
    let rise = 100.0;
    let height = 5.0 * 3.0_f64.sqrt();
    let mut sk = Sketch::new();
    let a = sk.point(0.0, 0.0).unwrap();
    let b = sk.point(10.0, 0.0).unwrap();
    let c = sk.point(5.0, height).unwrap();
    sk.distance(a, c, 10.0).unwrap();
    sk.distance(b, c, 10.0).unwrap();
    sk.fix(a, 0.0, rise).unwrap();
    sk.fix(b, 10.0, rise).unwrap();
    for algorithm in ALGORITHMS {
        let out = solve(&sk, with(algorithm));
        assert!(out.converged, "{algorithm:?}");
        let at = |p: crate::model::Point, axis| out.coordinates[&(p.id(), axis)];
        let (x, y) = (at(c, Coordinate::X), at(c, Coordinate::Y));
        assert!(
            (x - 5.0).abs() <= COORDINATE_ERROR && (y - (rise + height)).abs() <= COORDINATE_ERROR,
            "{algorithm:?}: {x} {y}"
        );
    }
}

/// A staged part continued from where it was solved is held to the whole
/// component's acceptance at that start, not to a mixed measure (its own
/// entities at the start, everything outside it where the earlier parts have
/// moved). Here the head's far corner falls from 1000 to 1: the start's
/// residual of 1e-9 meets the acceptance of a 1000 extent and not that of a
/// unit one (plans/m8-frame-budget review).
#[test]
fn a_continued_part_is_held_to_the_acceptance_of_the_whole_at_its_start() {
    let mut sk = Sketch::new();
    let a = sk.point_fixed(0.0, 0.0).unwrap();
    let d = sk.point(1000.0, 0.0).unwrap();
    let c = sk.point(0.5, 0.0).unwrap();
    let h = sk.line(a, d).unwrap();
    sk.horizontal(h).unwrap();
    let l = sk.line(c, d).unwrap();
    sk.horizontal(l).unwrap();
    sk.distance(a, c, 0.5 + 1e-9).unwrap();
    let c_id = c.id();
    let system = System::new(&sk);
    let component = system
        .components
        .iter()
        .find(|c| c.params.contains(&(c_id, Coordinate::X)))
        .unwrap();
    let parts = parts_of(&system, component).expect("a head and a stage");
    let k = parts
        .iter()
        .position(|p| p.params.contains(&(c.id(), Coordinate::X)))
        .unwrap();
    assert!(k > 0);
    let start = sk.coordinates();
    let mut moved = start.clone();
    moved.insert((d.id(), Coordinate::X), 1.0);
    moved.insert((d.id(), Coordinate::Y), 0.1);
    moved.insert((c.id(), Coordinate::Y), 0.1);
    let fresh = || Outcome::start(moved.clone());
    let mut splitter = Splitter::new(
        &component.frame,
        &parts.iter().map(|p| &p.params[..]).collect::<Vec<_>>(),
    );
    // As `solve_staged`: each earlier part split before its own move.
    for j in 0..k {
        let _ = splitter.split(j, if j == 0 { &start } else { &moved });
        splitter.moved(j);
    }
    let accepts = Scope::measured(component, splitter.split(k, &moved));
    let split = Scope {
        part: &parts[k],
        whole: component,
        accepts: &accepts,
        solved: Some(&start),
    };
    let whole = Scope {
        accepts: component,
        ..split
    };
    for algorithm in ALGORITHMS {
        let options = with(algorithm);
        let want = continued(&system, whole, &start, &options, &mut fresh());
        let got = continued(&system, split, &start, &options, &mut fresh());
        assert!(
            want.is_some(),
            "{algorithm:?}: the whole's acceptance holds the start"
        );
        let bits = |v: &Option<Coordinates>| -> Option<Vec<u64>> {
            v.as_ref()
                .map(|v| v.values().map(|x| x.to_bits()).collect())
        };
        assert_eq!(bits(&got), bits(&want), "{algorithm:?}");
    }
}

/// The sparse satisfaction test equals the dense `evaluate(..).satisfied`
/// (plans/m8-frame-floor step 3): at the initial guess, a solution, zeros
/// (degenerate rows), a non-finite coordinate and f64's extremes.
#[test]
fn satisfaction_without_a_jacobian_equals_the_dense_one() {
    use laman_debug::corpus::{rect_chain, slot_grid, triangle_strip};
    let mut sketches: Vec<(String, Sketch)> = vec![];
    for slug in [
        "sss-triangle",
        "belt",
        "slotted-plate",
        "sss-triangle-redundant",
        "belt-redundant",
        "slotted-plate-redundant",
        "sss-triangle-conflicting",
        "belt-conflicting",
        "slotted-plate-conflicting",
    ] {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/c1")
            .join(slug);
        let fixture = laman_debug::fixture::Fixture::load(&path).unwrap();
        let sk: Sketch =
            serde_json::from_str(&serde_json::to_string(&fixture.sketch).unwrap()).unwrap();
        sketches.push((slug.to_string(), sk));
    }
    for (name, case) in [
        ("rect-chain-19", rect_chain(19)),
        ("slot-grid-3x3", slot_grid(3, 3)),
        ("triangle-strip-32", triangle_strip(32)),
    ] {
        let case = case.unwrap();
        let sk: Sketch =
            serde_json::from_str(&serde_json::to_string(&case.sketch).unwrap()).unwrap();
        sketches.push((name.to_string(), sk));
    }
    let (mut satisfied, mut unsatisfied) = (0, 0);
    for (name, sk) in &sketches {
        let system = System::new(sk);
        let guess = sk.coordinates();
        let mut variants: Vec<(&str, Coordinates)> = vec![("guess", guess.clone())];
        for (label, value) in [
            ("zero", 0.0),
            ("nan", f64::NAN),
            ("inf", f64::INFINITY),
            ("huge", 1e300),
            ("tiny", 1e-300),
        ] {
            let mut all = guess.clone();
            for v in all.values_mut() {
                *v = value;
            }
            variants.push((label, all));
            // One coordinate only: a single bad entry among good rows.
            let mut one = guess.clone();
            if let Some(v) = one.values_mut().next() {
                *v = value;
            }
            variants.push((label, one));
        }
        // The solved coordinates, where most components are satisfied.
        let solved = sk.solve(&crate::solution::SolverOptions::default());
        variants.push(("solved", solved.coordinates.clone()));
        for (label, values) in &variants {
            for component in &system.components {
                let dense = dense_satisfied(&system, component, values);
                assert_eq!(
                    sparse_satisfied(&system, component, values),
                    dense,
                    "{name} at {label}"
                );
                if dense {
                    satisfied += 1;
                } else {
                    unsatisfied += 1;
                }
            }
        }
    }
    assert!(
        satisfied > 0 && unsatisfied > 0,
        "{satisfied} {unsatisfied}"
    );
}
