//! Regressions found by solving, under `tests/fixtures/regression/`. Each
//! asserts the desired result and is ignored until its fix lands.
#![allow(clippy::unwrap_used, clippy::expect_used)] // tests: a failing unwrap is the test failing (clippy.toml)

use laman::solution::{Algorithm, SolverOptions, Status};
use laman_check::check;
use laman_debug::fixture::Fixture;
use std::path::PathBuf;

fn load(slug: &str) -> Fixture {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/regression")
        .join(slug);
    Fixture::load(&dir).unwrap()
}

#[test]
#[ignore = "the checker returns NaN for a satisfied point_on_line at 1e303 magnitudes"]
fn extreme_point_on_line() {
    let f = load("extreme-point-on-line");
    let solution = f.sketch.solve(&SolverOptions::default());
    assert_eq!(solution.status, Status::UnderConstrained);
    assert_eq!(
        u32::try_from(solution.diagnostics.dof).unwrap(),
        f.expected.dof
    );
    let report = check(&f.sketch, &solution.coordinates);
    assert!(report.is_clean(), "{report:?}");
}

#[test]
#[ignore = "DogLeg ends NotConverged on a conflict Levenberg-Marquardt proves"]
fn dogleg_conflict_inconclusive() {
    let f = load("dogleg-conflict-inconclusive");
    for algorithm in [Algorithm::LevenbergMarquardt, Algorithm::DogLeg] {
        let mut options = SolverOptions::default();
        options.algorithm = algorithm;
        let solution = f.sketch.solve(&options);
        assert_eq!(
            solution.status.to_string(),
            f.expected.status,
            "{algorithm:?}"
        );
        assert!(!solution.diagnostics.conflicts.is_empty(), "{algorithm:?}");
    }
}

#[test]
#[ignore = "both algorithms end NotConverged on a stretched rigid framework"]
fn framework_conflict_inconclusive() {
    let f = load("framework-conflict-inconclusive");
    for algorithm in [Algorithm::LevenbergMarquardt, Algorithm::DogLeg] {
        let mut options = SolverOptions::default();
        options.algorithm = algorithm;
        let solution = f.sketch.solve(&options);
        assert_eq!(
            solution.status.to_string(),
            f.expected.status,
            "{algorithm:?}"
        );
        assert!(!solution.diagnostics.conflicts.is_empty(), "{algorithm:?}");
    }
}

/// The only conflict is {`stated`, `stretched`}; the eight true distances
/// hold together at the guesses and are never named.
fn assert_rigid_framework(algorithm: Algorithm) {
    let f = load("false-conflict-rigid-framework");
    let mut options = SolverOptions::default();
    options.algorithm = algorithm;
    let solution = f.sketch.solve(&options);
    assert_eq!(
        solution.status.to_string(),
        f.expected.status,
        "{algorithm:?}"
    );
    let found: Vec<Vec<&str>> = solution
        .diagnostics
        .conflicts
        .iter()
        .map(|set| {
            set.iter()
                .map(|c| c.tag.as_deref().unwrap_or("?"))
                .collect()
        })
        .collect();
    assert_eq!(found, [["stated", "stretched"]], "{algorithm:?}");
}

#[test]
fn false_conflict_rigid_framework() {
    assert_rigid_framework(Algorithm::LevenbergMarquardt);
}

#[test]
#[ignore = "DogLeg ends NotConverged on a conflict Levenberg-Marquardt proves"]
fn false_conflict_rigid_framework_dogleg() {
    assert_rigid_framework(Algorithm::DogLeg);
}

/// Every conflict reported contains `stretched`, the one contradiction.
fn assert_excludes_contradiction(algorithm: Algorithm) {
    let f = load("false-conflict-excludes-contradiction");
    let mut options = SolverOptions::default();
    options.algorithm = algorithm;
    let solution = f.sketch.solve(&options);
    assert_eq!(
        solution.status.to_string(),
        f.expected.status,
        "{algorithm:?}"
    );
    assert!(!solution.diagnostics.conflicts.is_empty(), "{algorithm:?}");
    for set in &solution.diagnostics.conflicts {
        assert!(
            set.iter().any(|c| c.tag.as_deref() == Some("stretched")),
            "{algorithm:?}: a conflict without `stretched`: {set:?}"
        );
    }
}

#[test]
fn false_conflict_excludes_contradiction() {
    assert_excludes_contradiction(Algorithm::LevenbergMarquardt);
}

#[test]
#[ignore = "DogLeg ends NotConverged on a conflict Levenberg-Marquardt proves"]
fn false_conflict_excludes_contradiction_dogleg() {
    assert_excludes_contradiction(Algorithm::DogLeg);
}

/// The status, DoF and checker of a solution whose radius the bound holds
/// at or near 0: violations none, the circle named degenerate.
fn assert_held_radius(f: &Fixture, solution: &laman::solution::Solution, algorithm: Algorithm) {
    assert_eq!(
        solution.status.to_string(),
        f.expected.status,
        "{algorithm:?}"
    );
    assert_eq!(
        u32::try_from(solution.diagnostics.dof).unwrap(),
        f.expected.dof,
        "{algorithm:?}"
    );
    assert!(
        solution
            .coordinates
            .iter()
            .all(|((_, c), v)| *c != laman::model::Coordinate::R || *v >= 0.0),
        "{algorithm:?}: {:?}",
        solution.coordinates
    );
    let report = check(&f.sketch, &solution.coordinates);
    assert!(report.violations.is_empty(), "{algorithm:?}: {report:?}");
}

#[test]
fn drag_negative_radius() {
    let f = load("drag-negative-radius");
    let drag = f.drag.as_ref().unwrap();
    for algorithm in [Algorithm::LevenbergMarquardt, Algorithm::DogLeg] {
        let mut options = SolverOptions::default();
        options.algorithm = algorithm;
        let mut session = f.sketch.drag(drag.dragged, &options).unwrap();
        for target in &drag.targets {
            session.to(*target);
        }
        let finish = session.finish();
        assert_held_radius(&f, &finish, algorithm);
        let mut applied = f.sketch.clone();
        let result = applied.apply(&finish);
        assert!(result.is_ok(), "{algorithm:?}: {result:?}");
    }
}

#[test]
fn negative_radius_solve() {
    let f = load("negative-radius-solve");
    for algorithm in [Algorithm::LevenbergMarquardt, Algorithm::DogLeg] {
        let mut options = SolverOptions::default();
        options.algorithm = algorithm;
        let solution = f.sketch.solve(&options);
        assert_held_radius(&f, &solution, algorithm);
        let mut applied = f.sketch.clone();
        let result = applied.apply(&solution);
        assert!(result.is_ok(), "{algorithm:?}: {result:?}");
    }
}

/// Editing the second distance moves only point 2 by construction. Point 1's
/// coordinates must keep their solved bits, even when those bits approximate 20.
fn assert_upstream_edit_keeps_bits(algorithm: Algorithm) {
    use laman::model::{Constraint, Coordinate, EntityId};

    let fixture = load("upstream-edit-drift");
    let mut options = SolverOptions::default();
    options.algorithm = algorithm;
    let before = fixture.sketch.solve(&options);
    let mut edited = fixture.sketch.clone();
    edited.apply(&before).unwrap();
    let (id, constraint) = edited
        .constraints()
        .find(|(id, _)| edited.constraint_tag_of(*id) == Some("edited"))
        .map(|(id, constraint)| (id, *constraint))
        .unwrap();
    let Constraint::Distance { p, q, .. } = constraint else {
        unreachable!("fixture edit is a distance")
    };
    edited.remove_constraint(id).unwrap();
    edited
        .add_constraint_with_id(id, Constraint::Distance { p, q, value: 22.0 })
        .unwrap();
    edited.tag_constraint(id, "edited").unwrap();
    let after = edited.solve(&options);
    for (sketch, solution, moved) in [(&fixture.sketch, &before, false), (&edited, &after, true)] {
        assert!(check(sketch, &solution.coordinates).is_clean());
        assert_eq!(solution.status.to_string(), fixture.expected.status);
        assert_eq!(solution.diagnostics.dof as u32, fixture.expected.dof);
        assert!(solution.diagnostics.redundant.is_empty());
        assert!(solution.diagnostics.conflicts.is_empty());
        assert!(solution.diagnostics.flips.is_empty());
        for (key, expected) in &fixture.expected.coordinates {
            // Grounded collinear distances give x = 20, then x = 40 or 42.
            let expected = if moved && *key == (EntityId(2), Coordinate::X) {
                42.0
            } else {
                *expected
            };
            assert!((solution.coordinates[key] - expected).abs() <= fixture.expected.tolerance);
        }
    }
    for (key, value) in &before.coordinates {
        if key.0 != EntityId(2) {
            assert_eq!(
                after.coordinates[key].to_bits(),
                value.to_bits(),
                "{algorithm:?}: upstream {key:?}: {} vs {value}",
                after.coordinates[key]
            );
        }
    }
}

#[test]
fn upstream_edit_drift_lm() {
    assert_upstream_edit_keeps_bits(Algorithm::LevenbergMarquardt);
}

#[test]
fn upstream_edit_drift_dogleg() {
    assert_upstream_edit_keeps_bits(Algorithm::DogLeg);
}

#[test]
#[ignore = "a satisfied status at a tangent whose line has collapsed, which the checker cannot measure"]
fn satisfied_status_degenerate_tangent() {
    let f = load("satisfied-status-degenerate-tangent");
    for algorithm in [Algorithm::LevenbergMarquardt, Algorithm::DogLeg] {
        let mut options = SolverOptions::default();
        options.algorithm = algorithm;
        let solution = f.sketch.solve(&options);
        let report = check(&f.sketch, &solution.coordinates);
        if matches!(
            solution.status,
            Status::FullyConstrained | Status::UnderConstrained
        ) {
            assert!(
                report.violations.is_empty(),
                "{algorithm:?}: {:?} but {report:?}",
                solution.status
            );
        }
    }
}

/// `whole-false-certificate`: the conflict sets of `path` each contain
/// `spared` and `stretched`, and the witness, where everything but `spared`
/// holds by the checker, proves no conflict can leave `spared` out.
fn assert_spared_in_every_conflict(
    path: fn(&laman::model::Sketch, &SolverOptions) -> laman::solution::Solution,
) {
    let f = load("whole-false-certificate");
    let tagged = |tag: &str| {
        f.sketch
            .constraints()
            .find(|(id, _)| f.sketch.constraint_tag_of(*id) == Some(tag))
            .map(|(id, _)| id)
            .unwrap()
    };
    let mut without = f.sketch.clone();
    without.remove_constraint(tagged("spared")).unwrap();
    let mut witness = without.coordinates();
    witness.extend(f.expected.coordinates.iter().map(|(k, v)| (*k, *v)));
    assert!(check(&without, &witness).is_clean());
    for algorithm in [Algorithm::LevenbergMarquardt, Algorithm::DogLeg] {
        let mut options = SolverOptions::default();
        options.algorithm = algorithm;
        let solution = path(&f.sketch, &options);
        for set in &solution.diagnostics.conflicts {
            for tag in ["spared", "stretched"] {
                assert!(
                    set.iter().any(|c| c.tag.as_deref() == Some(tag)),
                    "{algorithm:?}: {set:?} leaves out {tag}"
                );
            }
        }
    }
}

#[test]
fn whole_false_certificate_staged() {
    assert_spared_in_every_conflict(|sketch, options| sketch.solve(options));
}

#[test]
#[ignore = "the whole path certifies a stationary point of a satisfiable subset and names a conflict that holds together"]
fn whole_false_certificate() {
    assert_spared_in_every_conflict(laman::reference::solve);
}
