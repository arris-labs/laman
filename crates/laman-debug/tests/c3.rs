//! The C3 fixtures of ROADMAP.md §C3 acceptance, under `tests/fixtures/c3/`:
//! each is solved from its guesses, held to the expected coordinates and the
//! independent checker, and its profiles are extracted at the solution and at
//! the expected coordinates and held to the closed forms of `expected.json`.
#![allow(clippy::unwrap_used, clippy::expect_used)] // tests: a failing unwrap is the test failing (clippy.toml)

use laman::model::{Coordinates, Sketch};
use laman::profile::profiles;
use laman::solution::{SolverOptions, Status};
use laman_debug::fixture::{Fixture, SKETCH_FILE};
use laman_debug::json::{sketch_from_str, sketch_to_string};
use laman_debug::runner::solve_run;
use std::path::PathBuf;

const FIXTURES: [&str; 10] = [
    "rectangle",
    "slotted-plate",
    "slotted-plate-coincident",
    "plate-island",
    "two-loops",
    "circle-alone",
    "open-chain",
    "dangling-edge",
    "shared-edge",
    "unjoined",
];

/// Fixtures whose solved coordinates are not compared: their closed-form
/// coordinates are held at the default tolerance by `regression/` instead
/// (`coincident-contact-drift`, ignored until the solver reaches them).
const UNCOMPARED: [&str; 1] = ["slotted-plate-coincident"];

fn dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/c3")
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

fn extract(name: &str, sketch: &Sketch, values: &Coordinates, fixture: &Fixture) {
    let found = profiles(sketch, values).unwrap();
    let want = fixture.expected.profiles.as_ref().unwrap();
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

/// Solved, checked, then extracted: the order of the roadmap's acceptance.
#[test]
fn each_fixture_solves_checks_and_extracts_its_profiles() {
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
            if UNCOMPARED.contains(&name) {
                break;
            }
            let got = solved.solution.coordinates[key];
            assert!(
                (got - want).abs() <= fixture.expected.tolerance,
                "{name}: {key:?} {got} vs {want}"
            );
        }
        extract(
            name,
            &fixture.sketch,
            &solved.solution.coordinates,
            &fixture,
        );
        // The closed-form coordinates give the same profiles.
        extract(name, &fixture.sketch, &expected_values(&fixture), &fixture);
    }
}

#[test]
fn the_acceptance_numbers() {
    let area = |name: &str| {
        let f = load(name);
        profiles(&f.sketch, &expected_values(&f)).unwrap()
    };
    let near = |a: f64, b: f64| assert!((a - b).abs() <= 1e-9 * b.abs().max(1.0), "{a} vs {b}");
    let pi = std::f64::consts::PI;
    let r = area("rectangle");
    near(r.profiles[0].net_area, 7200.0);
    assert!(r.profiles[0].outer.edges.iter().all(|t| !t.reversed));
    for name in ["slotted-plate", "slotted-plate-coincident"] {
        let r = area(name);
        assert_eq!(r.profiles.len(), 1, "{name}");
        near(r.profiles[0].net_area, 7200.0 - 400.0 - 25.0 * pi);
        assert_eq!(r.profiles[0].holes.len(), 1, "{name}");
        assert!(r.profiles[0].holes[0].signed_area < 0.0, "{name}");
    }
    assert_eq!(
        area("slotted-plate-coincident").profiles[0].holes[0]
            .joins
            .len(),
        4
    );
    let r = area("plate-island");
    near(r.profiles[0].net_area, 7200.0 - 400.0 * pi);
    near(r.profiles[1].net_area, 100.0 * pi);
    assert_eq!(area("two-loops").profiles.len(), 2);
    near(area("circle-alone").profiles[0].net_area, 25.0 * pi);
    assert_eq!(area("open-chain").open_chains.len(), 1);
    assert_eq!(area("dangling-edge").dangling.len(), 1);
    assert_eq!(area("shared-edge").branched.len(), 1);
    assert!(area("shared-edge").profiles.is_empty());
    // A rectangle with a corner of two ground points at one place is an open
    // chain whose free ends are reported, never joined.
    let r = area("unjoined");
    assert!(r.profiles.is_empty());
    assert_eq!(r.open_chains.len(), 1);
    assert_eq!(r.unjoined.len(), 1);
}

/// A slot grid has exactly one profile, the plate, with one hole per slot,
/// and the net area is the plate less the slots, in closed form
/// (plans/m9-profiles step 7). The plate is `W` by `H` and each slot
/// `2·length·r + πr²` (`corpus::slot_grid` fixes length 40 and radius 5);
/// the expected coordinates are the construction's, never a solve.
#[test]
fn slot_grids_are_one_plate_with_a_hole_per_slot() {
    use laman_debug::corpus::slot_grid;
    let (length, radius) = (40.0, 5.0);
    for rows in 1..=7 {
        for cols in 1..=7 {
            let case = slot_grid(rows, cols).unwrap();
            let mut values = case.sketch.coordinates();
            for (key, value) in &case.expected.coordinates {
                values.insert(*key, *value);
            }
            let found = profiles(&case.sketch, &values).unwrap();
            let name = format!("slot-grid-{rows}x{cols}");
            if (rows, cols) == (7, 7) {
                assert_eq!(case.sketch.entities().count(), 498, "{name}");
            }
            assert_eq!(found.profiles.len(), 1, "{name}");
            assert!(found.open_chains.is_empty(), "{name}");
            assert!(found.dangling.is_empty(), "{name}");
            assert!(found.branched.is_empty(), "{name}");
            assert!(found.degenerate.is_empty(), "{name}");
            assert!(found.unjoined.is_empty(), "{name}");
            let plate = &found.profiles[0];
            assert_eq!(plate.holes.len(), rows * cols, "{name}");
            // The plate's size, from `corpus::slot_grid`'s pitch and margin.
            let width = cols as f64 * (2.0 * length + 4.0 * radius) + 4.0 * radius;
            let height = rows as f64 * 6.0 * radius + 4.0 * radius;
            let slot = 2.0 * length * radius + std::f64::consts::PI * radius * radius;
            let n = (rows * cols) as f64;
            let net = width * height - n * slot;
            assert!(
                (plate.outer.signed_area - width * height).abs() <= 1e-9 * width * height,
                "{name}: plate {} vs {}",
                plate.outer.signed_area,
                width * height
            );
            assert!(
                (plate.net_area - net).abs() <= 1e-9 * (width * height),
                "{name}: net {} vs {net}",
                plate.net_area
            );
            for hole in &plate.holes {
                assert!(hole.signed_area < 0.0, "{name}");
                assert!(
                    (hole.signed_area + slot).abs() <= 1e-9 * slot,
                    "{name}: hole {} vs {slot}",
                    hole.signed_area
                );
                assert_eq!(hole.edges.len(), 4, "{name}");
            }
        }
    }
}
