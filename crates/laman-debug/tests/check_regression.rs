//! Regressions in the independent checker, under `tests/fixtures/regression/`.
#![allow(clippy::unwrap_used, clippy::expect_used)] // tests: a failing unwrap is the test failing (clippy.toml)

use laman::model::{Coordinate, EntityId, Sketch};
use laman_check::{Source, check};
use laman_debug::fixture::Fixture;
use std::path::PathBuf;

fn load(slug: &str) -> Fixture {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/regression")
        .join(slug);
    Fixture::load(&dir).unwrap()
}

/// The sketch with entity `id`'s stored point moved to (`x`, `y`).
fn reseed(sketch: &Sketch, id: u32, x: f64, y: f64) -> Sketch {
    let mut form = serde_json::to_value(sketch).unwrap();
    for entity in form["entities"].as_array_mut().unwrap() {
        if entity["id"] == id {
            entity["x"] = serde_json::json!(x);
            entity["y"] = serde_json::json!(y);
        }
    }
    serde_json::from_value(form).unwrap()
}

fn flagged(report: &laman_check::Report) -> Vec<Source> {
    report.violations.iter().map(|v| v.id).collect()
}

#[test]
#[ignore = "check takes its length tolerance from the sketch's guesses, so a far guess (1e6) accepts a 1e-6 error on a length-1 distance"]
fn check_guess_scale() {
    let f = load("check-guess-scale");
    let far = EntityId(1);
    // Expected lists free parameters only; fixed ones keep the sketch's values.
    let mut solution = f.sketch.coordinates();
    solution.extend(f.expected.coordinates.iter().map(|(k, v)| (*k, *v)));
    assert!(check(&f.sketch, &solution).is_clean());

    let mut off = solution;
    *off.get_mut(&(far, Coordinate::X)).unwrap() += 1e-6;
    let at_solution = reseed(&f.sketch, far.0, 1.0, 0.0);
    let reference = check(&at_solution, &off);
    assert_eq!(
        reference.violations.len(),
        1,
        "reseeded control must flag `unit`"
    );

    // The verdict at given coordinates must not depend on the guesses.
    assert_eq!(flagged(&check(&f.sketch, &off)), flagged(&reference));
}
