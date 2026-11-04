//! `Sketch::solve` and `Sketch::apply`: stale and foreign solutions, atomic
//! refusals, fixed coordinates, warm starts and the serde form.
#![allow(clippy::unwrap_used, clippy::panic)]

use laman::model::{Coordinate, EntityId, ModelError, Sketch};
use laman::solution::{Algorithm, Solution, SolverOptions, Status};
use laman_check::check;

fn rod() -> (Sketch, EntityId, EntityId) {
    let mut sk = Sketch::new();
    let a = sk.point_fixed(0.0, 0.0).unwrap();
    let b = sk.point(3.0, 1.0).unwrap();
    sk.distance(a, b, 5.0).unwrap();
    let l = sk.line(a, b).unwrap();
    sk.horizontal(l).unwrap();
    (sk, a.id(), b.id())
}

fn options(algorithm: Algorithm) -> SolverOptions {
    let mut o = SolverOptions::default();
    o.algorithm = algorithm;
    o
}

#[test]
fn both_algorithms_solve_and_the_checker_agrees() {
    for algorithm in [Algorithm::LevenbergMarquardt, Algorithm::DogLeg] {
        let (mut sk, _, b) = rod();
        let s = sk.solve(&options(algorithm));
        assert_eq!(s.status, Status::FullyConstrained);
        assert_eq!(s.diagnostics.dof, 0);
        assert!((s.coordinates[&(b, Coordinate::X)] - 5.0).abs() < 1e-9);
        sk.apply(&s).unwrap();
        assert!(check(&sk, &sk.coordinates()).is_clean());
    }
}

#[test]
fn a_stale_solution_is_refused_and_changes_nothing() {
    let (mut sk, a, _) = rod();
    let s = sk.solve(&SolverOptions::default());
    sk.tag(a, "edit").unwrap();
    let before = sk.coordinates();
    let revision = sk.revision();
    assert_eq!(
        sk.apply(&s),
        Err(ModelError::Stale {
            solved: revision - 1,
            current: revision
        })
    );
    assert_eq!(sk.coordinates(), before);
    assert_eq!(sk.revision(), revision);
}

#[test]
fn equal_revision_from_other_contents_is_foreign() {
    let (sk, _, _) = rod();
    let mut other = Sketch::new();
    let a = other.point_fixed(0.0, 0.0).unwrap();
    let b = other.point(9.0, 9.0).unwrap();
    other.distance(a, b, 5.0).unwrap();
    let l = other.line(a, b).unwrap();
    other.horizontal(l).unwrap();
    assert_eq!(sk.revision(), other.revision());
    let s = other.solve(&SolverOptions::default());
    let mut sk = sk;
    assert_eq!(sk.apply(&s), Err(ModelError::ForeignSolution));
}

fn refused(edit: impl FnOnce(&mut Solution, EntityId, EntityId)) -> (ModelError, Sketch, Sketch) {
    let (mut sk, a, b) = rod();
    let mut s = sk.solve(&SolverOptions::default());
    edit(&mut s, a, b);
    let before = sk.clone();
    let err = sk.apply(&s).unwrap_err();
    (err, before, sk)
}

#[test]
fn malformed_solutions_are_refused_atomically() {
    let x = |id| (id, Coordinate::X);
    let (e, before, after) = refused(|s, _, b| {
        s.coordinates.remove(&x(b));
    });
    assert!(matches!(e, ModelError::IncompleteSolution { .. }));
    assert_eq!(
        (before.coordinates(), before.revision()),
        (after.coordinates(), after.revision())
    );

    let (e, ..) = refused(|s, _, b| {
        s.coordinates.insert(x(b), f64::NAN);
    });
    assert!(matches!(e, ModelError::NonFinite { .. }));
    let (e, ..) = refused(|s, a, _| {
        s.coordinates.insert(x(a), 1.0);
    });
    assert!(matches!(e, ModelError::FixedMoved { .. }));
    let (e, ..) = refused(|s, _, _| {
        s.coordinates.insert((EntityId(99), Coordinate::X), 1.0);
    });
    assert_eq!(e, ModelError::UnknownId { id: EntityId(99) });
    let (e, ..) = refused(|s, _, b| {
        s.coordinates.insert((b, Coordinate::R), 1.0);
    });
    assert!(matches!(e, ModelError::NoSuchParameter { .. }));
}

#[test]
fn a_negative_radius_is_refused() {
    let mut sk = Sketch::new();
    let c = sk.point(0.0, 0.0).unwrap();
    let k = sk.circle(c, 1.0).unwrap();
    let mut s = sk.solve(&SolverOptions::default());
    s.coordinates.insert((k.id(), Coordinate::R), -1.0);
    assert!(matches!(
        sk.apply(&s),
        Err(ModelError::DimensionOutOfRange { .. })
    ));
}

/// Circles externally tangent, centres held 10 apart, the second's radius
/// 20: only `r = −10` holds the tangency, so with `r ≥ 0` (ADR-0010) the
/// three constraints conflict. The free radius ends held at 0, exactly, and
/// the solution applies; without the bound the solve converged to `r = −10`.
#[test]
fn a_radius_held_at_zero_is_a_conflict_not_a_negative_radius() {
    for algorithm in [Algorithm::LevenbergMarquardt, Algorithm::DogLeg] {
        let mut sk = Sketch::new();
        let a = sk.point_fixed(0.0, 0.0).unwrap();
        let b = sk.point(0.0, 25.0).unwrap();
        let small = sk.circle(a, 1.0).unwrap();
        let large = sk.circle(b, 20.0).unwrap();
        let fix = sk.fix(b, 0.0, 10.0).unwrap();
        let radius = sk.radius(large, 20.0).unwrap();
        let tangent = sk.tangent_circles(small, large).unwrap();
        let s = sk.solve(&options(algorithm));
        assert_eq!(s.status, Status::Unsatisfiable, "{algorithm:?}");
        let sets: Vec<Vec<_>> = s
            .diagnostics
            .conflicts
            .iter()
            .map(|set| set.iter().map(|c| c.id).collect())
            .collect();
        assert_eq!(sets, [[fix, radius, tangent]], "{algorithm:?}");
        assert_eq!(
            s.coordinates[&(small.id(), Coordinate::R)],
            0.0,
            "{algorithm:?}"
        );
        sk.apply(&s).unwrap();
    }
}

#[test]
fn apply_then_solve_keeps_every_bit() {
    let (mut sk, _, _) = rod();
    let s = sk.solve(&SolverOptions::default());
    sk.apply(&s).unwrap();
    assert_eq!(sk.coordinates(), s.coordinates);
    let again = sk.solve(&SolverOptions::default());
    for (k, v) in &s.coordinates {
        assert_eq!(again.coordinates[k].to_bits(), v.to_bits());
    }
    assert_eq!(again.status, Status::FullyConstrained);
    // The first solution is stale; the second applies.
    assert!(sk.apply(&s).is_err());
    sk.apply(&again).unwrap();
}

#[test]
fn empty_and_fixed_only_sketches_solve() {
    let mut sk = Sketch::new();
    let s = sk.solve(&SolverOptions::default());
    assert_eq!(s.status, Status::FullyConstrained);
    sk.apply(&s).unwrap();
    let p = sk.point_fixed(1.0, 2.0).unwrap();
    let s = sk.solve(&SolverOptions::default());
    assert_eq!(s.status, Status::FullyConstrained);
    assert_eq!(s.diagnostics.entities[0].entity.id, p.id());
}

#[test]
fn conflicts_name_ids_kinds_and_tags() {
    let mut sk = Sketch::new();
    let a = sk.point_fixed(0.0, 0.0).unwrap();
    let b = sk.point_fixed(1.0, 0.0).unwrap();
    let d = sk.distance(a, b, 2.0).unwrap();
    sk.tag_constraint(d, "gap").unwrap();
    let s = sk.solve(&SolverOptions::default());
    assert_eq!(s.status, Status::Unsatisfiable);
    let set = &s.diagnostics.conflicts[0];
    assert_eq!((set[0].id, set[0].tag.as_deref()), (d, Some("gap")));
}

#[test]
fn the_serde_form_round_trips_and_is_deterministic() {
    let (sk, _, _) = rod();
    let s = sk.solve(&SolverOptions::default());
    let text = serde_json::to_string(&s).unwrap();
    assert_eq!(
        text,
        serde_json::to_string(&sk.solve(&SolverOptions::default())).unwrap()
    );
    let back: Solution = serde_json::from_str(&text).unwrap();
    assert_eq!(back, s);
    let mut sk = sk;
    sk.apply(&back).unwrap();
}

#[test]
fn a_read_solution_cannot_bypass_apply() {
    let (mut sk, _, _) = rod();
    let s = sk.solve(&SolverOptions::default());
    let mut v: serde_json::Value = serde_json::to_value(&s).unwrap();
    // A forged digest is foreign; a forged revision is stale.
    v["source"]["digest"] = "0000000000000000".into();
    let forged: Solution = serde_json::from_value(v.clone()).unwrap();
    assert_eq!(sk.apply(&forged), Err(ModelError::ForeignSolution));
    v["source"]["revision"] = 999.into();
    let forged: Solution = serde_json::from_value(v.clone()).unwrap();
    assert!(matches!(sk.apply(&forged), Err(ModelError::Stale { .. })));
}

#[test]
fn malformed_serialized_solutions_are_refused() {
    let (sk, _, _) = rod();
    let good = serde_json::to_value(sk.solve(&SolverOptions::default())).unwrap();
    let bad = |edit: &dyn Fn(&mut serde_json::Value)| {
        let mut v = good.clone();
        edit(&mut v);
        serde_json::from_value::<Solution>(v).is_err()
    };
    assert!(bad(&|v| v["format"] = "laman.sketch".into()));
    assert!(bad(&|v| v["version"] = 2.into()));
    assert!(bad(&|v| v["source"]["digest"] = "xyz".into()));
    assert!(bad(&|v| v["source"]["digest"] = "gggggggggggggggg".into()));
    assert!(bad(&|v| v["status"] = "Fine".into()));
    assert!(bad(&|v| {
        let first = v["coordinates"][0].clone();
        v["coordinates"].as_array_mut().unwrap().push(first);
    }));
    assert!(bad(
        &|v| v["coordinates"][0]["value"] = serde_json::Value::Null
    ));
    assert!(bad(&|v| {
        v.as_object_mut().unwrap().remove("diagnostics");
    }));
}

/// A line `b` drawn antiparallel to the fixed line `a`, its free end pulled
/// to the one place that is parallel the other way: by construction the
/// solve crosses from `dot < 0` to `dot > 0`, and names it.
fn antiparallel_then_pulled() -> Sketch {
    let mut sk = Sketch::new();
    let o = sk.point_fixed(0.0, 0.0).unwrap();
    let e = sk.point_fixed(1.0, 0.0).unwrap();
    let a = sk.line(o, e).unwrap();
    let s = sk.point_fixed(0.0, 1.0).unwrap();
    let p = sk.point(-1.0, 0.3).unwrap();
    let b = sk.line(s, p).unwrap();
    let q = sk.point_fixed(1.0, 1.0).unwrap();
    let parallel = sk.parallel(a, b).unwrap();
    sk.tag_constraint(parallel, "same-way").unwrap();
    sk.coincident(p, q).unwrap();
    sk
}

#[test]
fn a_solve_that_crosses_to_the_other_root_names_the_flip() {
    use laman::solution::FlipSubject;
    for algorithm in [Algorithm::LevenbergMarquardt, Algorithm::DogLeg] {
        let sk = antiparallel_then_pulled();
        let s = sk.solve(&options(algorithm));
        assert!(check(&sk, &s.coordinates).is_clean(), "{algorithm:?}");
        let flips = &s.diagnostics.flips;
        assert_eq!(flips.len(), 1, "{algorithm:?}: {flips:?}");
        let FlipSubject::Constraint(c) = &flips[0].subject else {
            panic!("a constraint flips");
        };
        assert_eq!(c.tag.as_deref(), Some("same-way"));
        assert!(flips[0].was < 0.0 && flips[0].now > 0.0, "{flips:?}");
        // The serde form carries the flips and reads them back.
        let text = serde_json::to_string(&s).unwrap();
        assert!(text.contains("\"flips\""));
        let back: Solution = serde_json::from_str(&text).unwrap();
        assert_eq!(back.diagnostics.flips, s.diagnostics.flips);
    }
}

#[test]
fn a_solve_that_keeps_every_branch_omits_flips_from_the_form() {
    let (sk, _, _) = rod();
    let s = sk.solve(&SolverOptions::default());
    assert!(s.diagnostics.flips.is_empty());
    assert!(!serde_json::to_string(&s).unwrap().contains("flips"));
}
