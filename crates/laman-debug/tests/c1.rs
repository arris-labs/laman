//! The C1 fixtures of ROADMAP.md §C1 acceptance, under `tests/fixtures/c1/`:
//! each base and redundant twin checks clean at its expected coordinates,
//! each conflicting twin flags exactly the constraint it added, and every
//! `sketch.json` round-trips byte for byte. The solve-and-compare tests are
//! written against the runner with the solve stubbed to return the guesses,
//! and wait for the solver (M2).
#![allow(clippy::unwrap_used, clippy::expect_used)] // tests: a failing unwrap is the test failing (clippy.toml)

use laman::model::{Coordinates, Sketch};
use laman_debug::fixture::{Fixture, SKETCH_FILE};
use laman_debug::json::{sketch_from_str, sketch_to_string};
use laman_debug::runner::run;
use std::path::PathBuf;

fn dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/c1")
        .join(name)
}

fn load(name: &str) -> Fixture {
    Fixture::load(&dir(name)).unwrap()
}

/// The tags of the violations a fixture shows at its expected coordinates.
fn flagged(name: &str) -> Vec<String> {
    let result = run(&load(name));
    assert!(result.report.degenerate.is_empty(), "{name}: degenerate");
    assert!(result.report.missing.is_empty(), "{name}: missing");
    result
        .report
        .violations
        .iter()
        .map(|v| v.tag.clone().unwrap_or_default())
        .collect()
}

/// The solve, stubbed until M2: it returns the guesses.
fn solve(sketch: &Sketch) -> Coordinates {
    sketch.coordinates()
}

const FIXTURES: [&str; 6] = [
    "sss-triangle",
    "sss-triangle-redundant",
    "sss-triangle-conflicting",
    "belt",
    "belt-redundant",
    "belt-conflicting",
];

#[test]
fn every_sketch_file_round_trips_byte_for_byte() {
    for name in FIXTURES {
        let text = std::fs::read_to_string(dir(name).join(SKETCH_FILE)).unwrap();
        let back = sketch_to_string(&sketch_from_str(&text).unwrap());
        assert_eq!(back, text, "{name}");
    }
}

#[test]
fn the_bases_and_redundant_twins_check_clean_at_their_expected_coordinates() {
    for name in [
        "sss-triangle",
        "sss-triangle-redundant",
        "belt",
        "belt-redundant",
    ] {
        let result = run(&load(name));
        assert!(result.report.is_clean(), "{name}: {:?}", result.report);
    }
}

#[test]
fn the_redundant_twins_name_what_they_added() {
    assert_eq!(
        load("sss-triangle").expected.redundant,
        Vec::<String>::new()
    );
    assert_eq!(
        load("sss-triangle-redundant").expected.redundant,
        ["right-angle"]
    );
    assert_eq!(load("belt-redundant").expected.redundant, ["same-belt"]);
}

#[test]
fn the_sss_conflicting_twin_flags_only_apex() {
    assert_eq!(flagged("sss-triangle-conflicting"), ["apex"]);
}

#[test]
fn the_belt_conflicting_twin_flags_only_top_length() {
    assert_eq!(flagged("belt-conflicting"), ["top-length"]);
}

#[test]
fn the_conflict_sets_are_the_ones_the_roadmap_names() {
    assert_eq!(
        load("sss-triangle-conflicting").expected.conflicts,
        [["base", "left", "right", "apex"]]
    );
    let belt = &load("belt-conflicting").expected.conflicts;
    assert_eq!(belt.len(), 1);
    assert_eq!(
        belt[0],
        [
            "r1",
            "r2",
            "span",
            "top-on-c1",
            "top-on-c2",
            "top-tangent-c1",
            "top-tangent-c2",
            "top-length"
        ]
    );
}

/// The solve from the guesses lands on the expected coordinates (checked
/// there: the answer is clean). Runs when `solve` is the real one.
#[test]
#[ignore = "solver: M2"]
fn the_solve_reaches_the_expected_coordinates() {
    for name in [
        "sss-triangle",
        "sss-triangle-redundant",
        "belt",
        "belt-redundant",
    ] {
        let fixture = load(name);
        let solved = solve(&fixture.sketch);
        for (key, want) in &fixture.expected.coordinates {
            let got = solved.get(key).copied().unwrap();
            assert!(
                (got - want).abs() <= fixture.expected.tolerance,
                "{name}: {key:?}: {got} vs {want}"
            );
        }
    }
}
