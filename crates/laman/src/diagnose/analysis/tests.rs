#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use super::*;
use crate::solve::Algorithm;
use laman_debug::fixture::Fixture;
use std::path::PathBuf;

/// Every case runs under both engines, as the solver's own tests do.
const ALGORITHMS: [Algorithm; 2] = [Algorithm::LevenbergMarquardt, Algorithm::DogLeg];

fn with(algorithm: Algorithm) -> Options {
    Options {
        algorithm,
        ..Options::default()
    }
}

fn load(slug: &str) -> Sketch {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/c1")
        .join(slug);
    let fixture = Fixture::load(&path).unwrap();
    serde_json::from_str(&serde_json::to_string(&fixture.sketch).unwrap()).unwrap()
}

fn ids(sk: &Sketch, tags: &[&str]) -> Vec<ConstraintId> {
    let mut out: Vec<_> = tags
        .iter()
        .map(|t| {
            sk.constraints()
                .find(|(id, _)| sk.constraint_tag_of(*id) == Some(*t))
                .unwrap_or_else(|| panic!("no constraint tagged {t}"))
                .0
        })
        .collect();
    out.sort();
    out
}

/// The independent checker at `values`, across the test/library type
/// boundary through the public serde form (the checker links the normal
/// library).
fn check(sk: &Sketch, values: &Coordinates) -> laman_check::Report {
    let mut fixture = Fixture::parse(
        &serde_json::to_string(sk).unwrap(),
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
    laman_check::check(&fixture.sketch, &fixture.expected.coordinates)
}

/// What the checker finds violated, as this crate's sources.
fn violated(report: &laman_check::Report) -> Vec<Source> {
    report
        .violations
        .iter()
        .map(|v| match v.id {
            laman_check::Source::Constraint(id) => Source::Constraint(ConstraintId(id.0)),
            laman_check::Source::Arc(id) => Source::Arc(EntityId(id.0)),
        })
        .collect()
}

fn constraints(ids: &[ConstraintId]) -> Vec<Source> {
    ids.iter().copied().map(Source::Constraint).collect()
}

/// Solve, check and diagnose: every solve here is checked independently.
fn run(sk: &Sketch, options: Options) -> (Outcome, laman_check::Report, Diagnosis) {
    let out = solve(sk, options);
    assert!(out.coordinates.values().all(|x| x.is_finite()));
    let report = check(sk, &out.coordinates);
    let d = analyse(sk, &out, &options);
    assert_eq!(d.status == Status::Unsatisfiable, !d.conflicts.is_empty());
    // A satisfied status is the checker's verdict too, whether or not the
    // solve met its stricter polishing target.
    if matches!(
        d.status,
        Status::FullyConstrained | Status::UnderConstrained
    ) {
        assert!(violated(&report).is_empty(), "{report:?}");
    }
    (out, report, d)
}

/// `sk` with only the constraints `keep`.
fn only(sk: &Sketch, keep: &[ConstraintId]) -> Sketch {
    let mut out = sk.clone();
    for (id, _) in sk.constraints() {
        if !keep.contains(&id) {
            out.remove_constraint(id).unwrap();
        }
    }
    out
}

/// The set alone fails, and without any one member the rest solves
/// checker-clean: the removal witnesses that make it inclusion-minimal.
fn assert_witnessed(sk: &Sketch, set: &[ConstraintId], options: Options) {
    let alone = only(sk, set);
    let out = solve(&alone, options);
    assert!(!out.converged, "{set:?}");
    assert!(!violated(&check(&alone, &out.coordinates)).is_empty());
    for m in set {
        let rest: Vec<_> = set.iter().copied().filter(|k| k != m).collect();
        let less = only(sk, &rest);
        let out = solve(&less, options);
        assert!(out.converged, "{set:?} without {m:?}");
        assert!(check(&less, &out.coordinates).is_clean(), "without {m:?}");
    }
}

#[test]
fn c1_bases_and_redundant_twins() {
    for (slug, redundant) in [
        ("sss-triangle", vec![]),
        ("belt", vec![]),
        ("slotted-plate", vec![]),
        ("sss-triangle-redundant", vec!["right-angle"]),
        ("belt-redundant", vec!["same-belt"]),
        ("slotted-plate-redundant", vec!["level"]),
    ] {
        let sk = load(slug);
        for algorithm in ALGORITHMS {
            let (_, report, d) = run(&sk, with(algorithm));
            assert!(report.is_clean(), "{slug}");
            assert_eq!(d.status, Status::FullyConstrained, "{slug} {algorithm:?}");
            assert_eq!(d.freedom.dof, 0);
            assert!(d.freedom.entities.values().all(|n| *n == 0));
            assert!(d.freedom.directions.is_empty());
            // Every tangent, original equation dependent or not, is necessary.
            assert_eq!(d.redundant, ids(&sk, &redundant), "{slug} {algorithm:?}");
            assert!(d.conflicts.is_empty() && d.degenerate.is_empty());
            assert!(d.ill_conditioned.is_empty());
            assert_eq!(d.solves, 0, "redundancy costs no solve");
        }
    }
}

#[test]
fn c1_conflicting_twins() {
    // Sets by analysis (ROADMAP C1 acceptance, and below for the triangle);
    // the best effort violates exactly the residual's support.
    for (slug, set, support, solves) in [
        // o fixed, |oa| = 50, |ab| = 40, and b on the line through o at 60°
        // to oa: a's distance to that line is 50 sin 60° = 43.3 > 40, so
        // {base, right, apex} cannot hold whatever `left` says. Each pair
        // can: base + right + left is the triangle, base + apex + left puts
        // b at 30 along the 60° line, right + apex leaves |oa| free. The
        // roadmap's set adds `left`, which is not inclusion-minimal (plan
        // open question). At the least-squares point all four rotation
        // invariants share the residual; `horizontal` picks the rotation.
        (
            "sss-triangle-conflicting",
            vec!["base", "right", "apex"],
            vec!["base", "left", "right", "apex"],
            6,
        ),
        (
            "belt-conflicting",
            vec![
                "r1",
                "r2",
                "span",
                "top-on-c1",
                "top-on-c2",
                "top-tangent-c1",
                "top-tangent-c2",
                "top-length",
            ],
            vec![
                "r1",
                "r2",
                "span",
                "top-on-c1",
                "top-on-c2",
                "top-tangent-c1",
                "top-tangent-c2",
                "top-length",
            ],
            14,
        ),
        (
            "slotted-plate-conflicting",
            vec!["same-ends", "slot-r", "slot-r2"],
            vec!["same-ends", "slot-r", "slot-r2"],
            19,
        ),
    ] {
        let sk = load(slug);
        for algorithm in ALGORITHMS {
            let options = with(algorithm);
            let (out, report, d) = run(&sk, options);
            assert!(!out.converged);
            assert_eq!(
                violated(&report),
                constraints(&ids(&sk, &support)),
                "{slug} {algorithm:?}"
            );
            assert_eq!(d.status, Status::Unsatisfiable, "{slug} {algorithm:?}");
            assert_eq!(d.conflicts, vec![ids(&sk, &set)], "{slug} {algorithm:?}");
            assert!(d.redundant.is_empty(), "{slug}: {:?}", d.redundant);
            assert_eq!(d.freedom.dof, 0);
            // One re-solve per constraint of the failed component, one for
            // the rest after the set; no test needed its second start.
            assert_eq!(d.solves, solves, "{slug} {algorithm:?}");
            assert_witnessed(&sk, &d.conflicts[0], options);
            // Deterministic: the same input gives the same diagnosis.
            assert_eq!(analyse(&sk, &out, &options), d);
        }
    }
}

#[test]
fn separate_conflicts_and_several_in_one_component() {
    for algorithm in ALGORITHMS {
        // Two points, each fixed twice apart: two components, two sets, in
        // component order.
        let mut sk = Sketch::new();
        let p = sk.point(0.0, 0.0).unwrap();
        let q = sk.point(5.0, 5.0).unwrap();
        let f1 = sk.fix(p, 0.0, 0.0).unwrap();
        let g1 = sk.fix(q, 5.0, 5.0).unwrap();
        let f2 = sk.fix(p, 1.0, 0.0).unwrap();
        let g2 = sk.fix(q, 6.0, 5.0).unwrap();
        let (_, report, d) = run(&sk, with(algorithm));
        assert_eq!(d.status, Status::Unsatisfiable);
        assert_eq!(d.conflicts, [vec![f1, f2], vec![g1, g2]], "{algorithm:?}");
        // LM spends the shared budget on p's rejections and never reaches
        // q, which is re-solved alone first; DogLeg's region collapses
        // first and reaches both.
        let (support, solves) = match algorithm {
            Algorithm::LevenbergMarquardt => (vec![f1, f2, g2], 7),
            Algorithm::DogLeg => (vec![f1, g1, f2, g2], 6),
        };
        assert_eq!(violated(&report), constraints(&support), "{algorithm:?}");
        assert_eq!(d.solves, solves, "{algorithm:?}");
        // Three fixes of one point: every pair conflicts. ADR-0005's filter
        // drops f1 (f2, f3 still conflict), keeps f2 and f3 ({f3}, {f2}
        // solve); with f3 set aside, {f1, f2} conflicts and is the second
        // set; with f2 set aside, {f1} solves and the search ends.
        let mut sk = Sketch::new();
        let p = sk.point(0.0, 0.0).unwrap();
        let f1 = sk.fix(p, 0.0, 0.0).unwrap();
        let f2 = sk.fix(p, 1.0, 0.0).unwrap();
        let f3 = sk.fix(p, 0.0, 1.0).unwrap();
        let (_, _, d) = run(&sk, with(algorithm));
        assert_eq!(d.conflicts, [vec![f2, f3], vec![f1, f2]]);
        for set in &d.conflicts {
            assert_witnessed(&sk, set, with(algorithm));
        }
    }
}

#[test]
fn fixed_only_contradiction_and_implication() {
    for algorithm in ALGORITHMS {
        let mut sk = Sketch::new();
        let a = sk.point_fixed(0.0, 0.0).unwrap();
        let b = sk.point_fixed(3.0, 4.0).unwrap();
        let wrong = sk.distance(a, b, 6.0).unwrap();
        let (_, report, d) = run(&sk, with(algorithm));
        assert_eq!(violated(&report), constraints(&[wrong]));
        assert_eq!(d.status, Status::Unsatisfiable);
        assert_eq!(d.conflicts, [vec![wrong]]);
        // A satisfied fixed-only constraint is implied by the ground.
        let mut sk = Sketch::new();
        let a = sk.point_fixed(0.0, 0.0).unwrap();
        let b = sk.point_fixed(3.0, 4.0).unwrap();
        let right = sk.distance(a, b, 5.0).unwrap();
        let (_, _, d) = run(&sk, with(algorithm));
        assert_eq!(d.status, Status::FullyConstrained);
        assert_eq!(d.redundant, [right]);
    }
}

#[test]
fn conflict_through_an_arc_end_contact() {
    // Arc about the ground origin from s = (1, 0) to e; the line e -> q is
    // tangent to it, its membership the arc's end (ADR-0002), held by the
    // intrinsic equation. From ground q = (0, 1.8) the tangent length is
    // √(1.8² − 1) = 1.497, so `length` = 1 conflicts with the tangent. Each
    // alone holds: the unit circles about the origin and q meet. The arc's
    // definition and intrinsic equation belong to the entity, never blamed.
    for algorithm in ALGORITHMS {
        let mut sk = Sketch::new();
        let c = sk.point_fixed(0.0, 0.0).unwrap();
        let s = sk.point_fixed(1.0, 0.0).unwrap();
        let e = sk.point(-0.8, 0.6).unwrap();
        let q = sk.point_fixed(0.0, 1.8).unwrap();
        let arc = sk.arc(c, s, e).unwrap();
        let line = sk.line(e, q).unwrap();
        let tangent = sk.tangent_line_circle(line, arc).unwrap();
        let length = sk.distance(e, q, 1.0).unwrap();
        let contact = System::new(&sk)
            .equations
            .iter()
            .find_map(|e| e.contact.clone())
            .unwrap();
        assert_eq!(
            contact.membership,
            crate::system::Membership::ArcEnd(arc.id())
        );
        let (_, report, d) = run(&sk, with(algorithm));
        // The least-squares point shares the residual with the intrinsic
        // equation; violation is not membership.
        assert_eq!(
            violated(&report),
            [
                Source::Constraint(tangent),
                Source::Constraint(length),
                Source::Arc(arc.id())
            ],
            "{algorithm:?}"
        );
        assert_eq!(d.status, Status::Unsatisfiable, "{algorithm:?}");
        assert_eq!(d.conflicts, [vec![tangent, length]]);
        assert_witnessed(&sk, &d.conflicts[0], with(algorithm));
    }
}

#[test]
fn degenerating_subset_is_inconclusive() {
    // As above with q free, fixed inside by `fix`, and a length keeping the
    // line long. The whole sketch is certified, but {tangent, fix} alone
    // descends toward a zero-length line and certifies nothing from either
    // start, so no set is claimed.
    for algorithm in ALGORITHMS {
        let mut sk = Sketch::new();
        let c = sk.point_fixed(0.0, 0.0).unwrap();
        let s = sk.point_fixed(1.0, 0.0).unwrap();
        let e = sk.point(0.0, 1.0).unwrap();
        let q = sk.point(-1.0, 1.0).unwrap();
        let arc = sk.arc(c, s, e).unwrap();
        let line = sk.line(e, q).unwrap();
        sk.tangent_line_circle(line, arc).unwrap();
        sk.fix(q, 0.0, 0.5).unwrap();
        sk.distance(e, q, 1.0).unwrap();
        let (_, _, d) = run(&sk, with(algorithm));
        assert_eq!(d.status, Status::NotConverged, "{algorithm:?}");
        assert!(d.conflicts.is_empty());
        // {fix, length} and {tangent, length} solve; {tangent, fix} spends
        // both of its starts.
        assert_eq!(d.solves, 4, "{algorithm:?}");
    }
}

#[test]
fn inconclusive_is_not_converged() {
    let sk = load("belt-conflicting");
    for algorithm in ALGORITHMS {
        // No budget, or too little to finish the filter: no set, no claim.
        for budget in [0, 5] {
            let options = Options {
                max_analysis_solves: budget,
                ..with(algorithm)
            };
            let (_, _, d) = run(&sk, options);
            assert_eq!(d.status, Status::NotConverged, "{budget}");
            assert!(d.conflicts.is_empty());
            assert_eq!(d.solves, budget);
        }
        // A satisfiable sketch stopped by its iteration cap is not stationary.
        let sk = load("slotted-plate");
        let options = Options {
            max_iterations: 1,
            ..with(algorithm)
        };
        let (out, _, d) = run(&sk, options);
        assert!(!out.converged);
        assert_eq!(d.status, Status::NotConverged);
        // Re-solved alone from the best effort, then from the guesses.
        assert_eq!(d.solves, 2);
    }
}

#[test]
fn slow_singular_approach_is_never_certified() {
    // p one unit from two ground points two apart: the only solution
    // (1, 0) is a double root, approached linearly (Gauss–Newton halves y)
    // with |Jᵀf| / |(f_i |J_i|)| about √2 y. Stopped at any iteration while
    // still above tolerance, it must not read as a conflict.
    for algorithm in ALGORITHMS {
        let mut sk = Sketch::new();
        let a = sk.point_fixed(0.0, 0.0).unwrap();
        let b = sk.point_fixed(2.0, 0.0).unwrap();
        let p = sk.point(1.0, 0.5).unwrap();
        sk.distance(a, p, 1.0).unwrap();
        sk.distance(b, p, 1.0).unwrap();
        let mut stopped = 0;
        for cap in 1..=64 {
            let options = Options {
                max_iterations: cap,
                ..with(algorithm)
            };
            let (_, _, d) = run(&sk, options);
            assert_ne!(d.status, Status::Unsatisfiable, "{algorithm:?} {cap}");
            stopped += usize::from(d.status == Status::NotConverged);
        }
        assert!(stopped > 1, "{algorithm:?}");
    }
}

#[test]
fn redundancy_names_the_later_and_the_implied() {
    for algorithm in ALGORITHMS {
        // Two equal distances: the later is named.
        let mut sk = Sketch::new();
        let a = sk.point_fixed(0.0, 0.0).unwrap();
        let b = sk.point(3.0, 4.0).unwrap();
        sk.distance(a, b, 5.0).unwrap();
        let later = sk.distance(a, b, 5.0).unwrap();
        let (_, _, d) = run(&sk, with(algorithm));
        assert_eq!(d.status, Status::UnderConstrained);
        assert_eq!(d.redundant, [later]);
        // point_on_line then coincident with the line's end: removing the
        // coincidence frees the slide along the line, so it is necessary;
        // the point on the line is implied.
        let mut sk = Sketch::new();
        let a = sk.point_fixed(0.0, 0.0).unwrap();
        let b = sk.point_fixed(2.0, 1.0).unwrap();
        let p = sk.point(0.0, 0.0).unwrap();
        let line = sk.line(a, b).unwrap();
        let on = sk.point_on_line(p, line).unwrap();
        sk.coincident(p, a).unwrap();
        let (_, _, d) = run(&sk, with(algorithm));
        assert_eq!(d.status, Status::FullyConstrained);
        assert_eq!(d.redundant, [on]);
        // A duplicate beside a conflict in another component is still named;
        // nothing of the failed component is.
        let mut sk = Sketch::new();
        let p = sk.point(0.0, 0.0).unwrap();
        let f1 = sk.fix(p, 0.0, 0.0).unwrap();
        let f2 = sk.fix(p, 1.0, 0.0).unwrap();
        let a = sk.point_fixed(5.0, 0.0).unwrap();
        let b = sk.point(8.0, 4.0).unwrap();
        sk.distance(a, b, 5.0).unwrap();
        let later = sk.distance(a, b, 5.0).unwrap();
        let (_, _, d) = run(&sk, with(algorithm));
        assert_eq!(d.status, Status::Unsatisfiable);
        assert_eq!(d.conflicts, [vec![f1, f2]]);
        assert_eq!(d.redundant, [later]);
    }
}

#[test]
fn degenerate_entities_agree_with_the_checker() {
    let mut sk = Sketch::new();
    let o = sk.point_fixed(0.0, 0.0).unwrap();
    let p = sk.point(1.0, 1.0).unwrap();
    let q = sk.point(1.0, 1.0).unwrap();
    let short = sk.line(p, q).unwrap();
    let ray = sk.line(o, p).unwrap();
    let angle = sk.angle(short, ray, 0.5).unwrap();
    let zero = sk.circle(o, 0.0).unwrap();
    let on_centre = sk.point(0.0, 0.0).unwrap();
    let centre = sk.arc(o, on_centre, p).unwrap();
    let s = sk.point(3.0, 0.0).unwrap();
    let e = sk.point(3.0, 0.0).unwrap();
    let empty = sk.arc(o, s, e).unwrap();
    let values = sk.coordinates();
    let out = Outcome {
        coordinates: values.clone(),
        converged: false,
        iterations: 0,
        evaluations: 0,
        rejected: 0,
    };
    let d = analyse(&sk, &out, &Options::default());
    let report = check(&sk, &values);
    let theirs: Vec<(u32, String)> = report
        .degenerate
        .iter()
        .map(|g| (g.id.0, format!("{:?}", g.what)))
        .collect();
    let ours: Vec<(u32, String)> = d
        .degenerate
        .iter()
        .map(|g| (g.entity.0, format!("{:?}", g.collapse)))
        .collect();
    assert_eq!(ours, theirs);
    let of = |id: EntityId| d.degenerate.iter().find(|g| g.entity == id).unwrap();
    assert_eq!(of(short.id()).equations, [Source::Constraint(angle)]);
    assert_eq!(of(zero.id()).equations, []);
    assert_eq!(of(centre.id()).equations, [Source::Arc(centre.id())]);
    assert_eq!(of(empty.id()).equations, [Source::Arc(empty.id())]);
    // The angle on a zero-length line is undefined there, as is the
    // intrinsic equation of the arc whose centre is its start.
    assert!(d.ill_conditioned.contains(&Source::Constraint(angle)));
    assert!(d.ill_conditioned.contains(&Source::Arc(centre.id())));
    assert_eq!(d.status, Status::NotConverged);
    assert!(d.conflicts.is_empty(), "degeneracy certifies nothing");
}

#[test]
fn extremes_analyse_finitely() {
    for algorithm in ALGORITHMS {
        let mut sk = Sketch::new();
        let a = sk.point_fixed(0.0, 0.0).unwrap();
        let b = sk.point(3e300, 4e300).unwrap();
        sk.distance(a, b, 5e300).unwrap();
        sk.distance(a, b, 5e300).unwrap();
        let (_, _, d) = run(&sk, with(algorithm));
        assert_eq!(d.status, Status::UnderConstrained);
        assert_eq!(d.redundant.len(), 1);
        let mut sk = Sketch::new();
        let a = sk.point_fixed(-f64::MAX, 0.0).unwrap();
        let b = sk.point(f64::MAX, 0.0).unwrap();
        sk.distance(a, b, 1.0).unwrap();
        let (_, _, d) = run(&sk, with(algorithm));
        assert_eq!(d.status, Status::NotConverged);
        assert!(d.conflicts.is_empty());
        assert!(!d.ill_conditioned.is_empty());
        let mut sk = Sketch::new();
        let p = sk.point(1e-300, 0.0).unwrap();
        sk.fix(p, 0.0, 0.0).unwrap();
        sk.fix(p, 2e-300, 0.0).unwrap();
        // Two fixes 2e-300 apart: below LENGTH_FLOOR both hold (ADR-0003).
        let (_, _, d) = run(&sk, with(algorithm));
        assert_eq!(d.status, Status::FullyConstrained);
    }
}
