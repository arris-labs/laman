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

#[test]
#[ignore = "a drag step returns a radius of -2.55e-19, which Sketch::apply refuses"]
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
        let mut applied = f.sketch.clone();
        let result = applied.apply(&session.finish());
        assert!(result.is_ok(), "{algorithm:?}: {result:?}");
    }
}

#[test]
#[ignore = "a solve returns a radius of -7.6e90, which Sketch::apply refuses"]
fn negative_radius_solve() {
    let f = load("negative-radius-solve");
    for algorithm in [Algorithm::LevenbergMarquardt, Algorithm::DogLeg] {
        let mut options = SolverOptions::default();
        options.algorithm = algorithm;
        let solution = f.sketch.solve(&options);
        let mut applied = f.sketch.clone();
        let result = applied.apply(&solution);
        assert!(result.is_ok(), "{algorithm:?}: {result:?}");
    }
}
