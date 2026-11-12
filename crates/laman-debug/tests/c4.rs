//! The C4 intersection fixtures, under `tests/fixtures/c4/`: each is solved
//! from its guesses, held to the expected coordinates and the independent
//! checker, and its intersections are measured at the solution and at the
//! expected coordinates and held to the closed forms of `expected.json`
//! (ADR-0016).
#![allow(clippy::unwrap_used, clippy::expect_used)] // tests: a failing unwrap is the test failing (clippy.toml)

use laman::intersect::intersections;
use laman::model::intersection::MeetingKind;
use laman::model::{Coordinate, Coordinates, Sketch};
use laman::solution::{SolverOptions, Status};
use laman_debug::fixture::{Fixture, SKETCH_FILE};
use laman_debug::json::{sketch_from_str, sketch_to_string};
use laman_debug::runner::solve_run;
use std::path::PathBuf;

const FIXTURES: [&str; 8] = [
    "crossing-lines",
    "line-through-circle",
    "two-circles",
    "tangent-touch",
    "collinear-overlap",
    "arc-outside-sweep",
    "t-end-on-line",
    "arc-crosses-line",
];

fn dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/c4")
        .join(name)
}

fn load(name: &str) -> Fixture {
    Fixture::load(&dir(name)).unwrap()
}

fn expected_values(fixture: &Fixture) -> Coordinates {
    let mut values = fixture.sketch.coordinates();
    for (key, value) in &fixture.expected.coordinates {
        values.insert(*key, *value);
    }
    values
}

fn measure(name: &str, sketch: &Sketch, values: &Coordinates, fixture: &Fixture) {
    let found = intersections(sketch, values).unwrap();
    let want = fixture.expected.intersections.as_ref().unwrap();
    let off = want.mismatches(&found);
    assert!(off.is_empty(), "{name}: {off:#?}");
}

#[test]
fn every_directory_is_listed() {
    let mut on_disk: Vec<String> = std::fs::read_dir(dir(""))
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    on_disk.sort();
    let mut listed: Vec<&str> = FIXTURES.to_vec();
    listed.sort_unstable();
    assert_eq!(on_disk, listed);
}

#[test]
fn every_sketch_file_round_trips_byte_for_byte() {
    for name in FIXTURES {
        let text = std::fs::read_to_string(dir(name).join(SKETCH_FILE)).unwrap();
        let back = sketch_to_string(&sketch_from_str(&text).unwrap());
        assert_eq!(back, text, "{name}");
    }
}

/// Solved, checked, then intersected: the order of the plan's acceptance.
#[test]
fn each_fixture_solves_checks_and_measures_its_intersections() {
    for name in FIXTURES {
        let fixture = load(name);
        let solved = solve_run(&fixture, &SolverOptions::default());
        assert_eq!(solved.solution.status, Status::FullyConstrained, "{name}");
        assert_eq!(
            solved.solution.diagnostics.dof, fixture.expected.dof as usize,
            "{name}"
        );
        assert!(solved.report.is_clean(), "{name}: {:?}", solved.report);
        for (key, want) in &fixture.expected.coordinates {
            let got = solved.solution.coordinates[key];
            assert!(
                (got - want).abs() <= fixture.expected.tolerance,
                "{name}: {key:?} {got} vs {want}"
            );
        }
        measure(
            name,
            &fixture.sketch,
            &solved.solution.coordinates,
            &fixture,
        );
        // The closed-form coordinates give the same meetings.
        measure(name, &fixture.sketch, &expected_values(&fixture), &fixture);
    }
}

#[test]
fn the_acceptance_numbers() {
    let found = |name: &str| {
        let f = load(name);
        intersections(&f.sketch, &expected_values(&f)).unwrap()
    };
    let kinds = |name: &str| -> Vec<Vec<MeetingKind>> {
        found(name)
            .pairs
            .iter()
            .map(|p| p.meetings.iter().map(|m| m.kind).collect())
            .collect()
    };
    assert_eq!(kinds("crossing-lines"), [[MeetingKind::Crossing]]);
    assert_eq!(
        kinds("line-through-circle"),
        [[MeetingKind::Crossing, MeetingKind::Crossing]]
    );
    assert_eq!(
        kinds("two-circles"),
        [[MeetingKind::Crossing, MeetingKind::Crossing]]
    );
    assert_eq!(kinds("tangent-touch"), [[MeetingKind::Touch]]);
    assert!(matches!(
        kinds("collinear-overlap")[0][..],
        [MeetingKind::Overlap { .. }]
    ));
    assert!(found("arc-outside-sweep").pairs.is_empty());
    assert_eq!(kinds("t-end-on-line"), [[MeetingKind::Crossing]]);
    assert_eq!(kinds("arc-crosses-line"), [[MeetingKind::Crossing]]);
}

/// A solved `tangent` is a join, not a meeting (ADR-0016 D5): the slot's
/// lines meet its arcs only at the shared or coincident-joined ends, so a
/// plate whose tangents are all solved reports no pair at all. Both ways of
/// joining are held, from the C3 plates.
#[test]
fn a_solved_slot_plate_reports_no_intersection() {
    for name in ["slotted-plate", "slotted-plate-coincident"] {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/c3")
            .join(name);
        let fixture = Fixture::load(&path).unwrap();
        let solved = solve_run(&fixture, &SolverOptions::default());
        assert_eq!(solved.solution.status, Status::FullyConstrained, "{name}");
        let found = intersections(&fixture.sketch, &solved.solution.coordinates).unwrap();
        assert!(found.pairs.is_empty(), "{name}: {:#?}", found.pairs);
    }
}

/// A solved `tangent` with no join is exactly one touch, never two
/// crossings or none; the same line 1e-3 inside the circle is the control
/// that the measurement can tell the two apart.
#[test]
fn a_solved_free_tangent_is_one_touch_and_a_nudge_makes_two_crossings() {
    let mut sk = Sketch::new();
    let a = sk.point_fixed(-3.0, 0.0).unwrap();
    let b = sk.point_fixed(3.0, 0.0).unwrap();
    let c = sk.point(0.4, 2.3).unwrap();
    let line = sk.line(a, b).unwrap();
    let circle = sk.circle(c, 2.0).unwrap();
    sk.tangent_line_circle(line, circle).unwrap();
    let solved = sk.solve(&SolverOptions::default());
    assert_ne!(solved.status, Status::Unsatisfiable);
    let found = intersections(&sk, &solved.coordinates).unwrap();
    assert_eq!(found.pairs.len(), 1);
    assert_eq!(found.pairs[0].meetings.len(), 1);
    let m = found.pairs[0].meetings[0];
    assert_eq!(m.kind, MeetingKind::Touch);
    assert!(
        (m.at[1]).abs() < 1e-6 && (m.at[0] - 0.4).abs() < 1e-6,
        "{m:?}"
    );
    // The centre 1e-3 nearer the line: the line now cuts the circle twice.
    let mut moved = solved.coordinates.clone();
    let cy = (c.id(), Coordinate::Y);
    moved.insert(cy, moved[&cy] - 1e-3);
    let found = intersections(&sk, &moved).unwrap();
    let kinds: Vec<_> = found.pairs[0].meetings.iter().map(|m| m.kind).collect();
    assert_eq!(kinds, [MeetingKind::Crossing, MeetingKind::Crossing]);
}
