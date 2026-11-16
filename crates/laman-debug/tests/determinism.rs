//! The determinism corpus: the digest of every C1 fixture and twin, blessed
//! in `tests/fixtures/c1/digests.txt`, and of every C3 fixture's profiles, in
//! `tests/fixtures/c3/digests.txt`. `tools/digest-wasm.sh` runs this same test natively and on
//! wasm32-wasip1 and diffs the `digest <name> <hex>` lines it prints, so the
//! fixtures are embedded, not read (a WASI run preopens no directory).
//! A change that moves a digest re-blesses it in the same commit, saying why.
//! A run digest covers the loaded sketch and the checker's report at the
//! expected coordinates; a solved digest covers the sketch, the `Solution`
//! from the guesses (both algorithms) and the checker's report there. A
//! solved digest is blessed only for a run the checker passes: clean, or for
//! a conflicting twin exactly the violations `c1.rs` derives. The test below
//! holds that itself before it compares a digest. A drag-step digest covers
//! the step's index, its target's bits, the sketch, the step's solution and
//! the checker's report; the drag's ten steps are blessed per algorithm.
//! The digests and the conflicting twins' violations are read from
//! `digests.txt` and `violations.json` (`include_str!`, as wasip1 has no file
//! system); pytest reads the same files.
//!
//! The C3 corpus (`tests/fixtures/c3/`, ADR-0013) is digested the same way,
//! blessed in `tests/fixtures/c3/digests.txt` under `c3-` names: a fixture's
//! profiles extracted and checked at its expected coordinates
//! (`c3-<name>`), and at each algorithm's solution
//! (`c3-solved-<lm|dogleg>-<name>`), plus the 3 by 3 slot grid at its
//! expected coordinates. A profile digest covers the coordinates, the
//! extracted profiles and the profile check's report, and a solved one the
//! solution and its report first; both are blessed only for runs both
//! checkers pass (`c3.rs` holds the same runs).
//!
//! The C4 corpus (`tests/fixtures/c4/`, ADR-0016 to ADR-0018) likewise, in
//! `tests/fixtures/c4/digests.txt` under `c4-` names: each intersection
//! fixture's intersections and checks at the expected coordinates and at
//! each algorithm's solution, each branched-core fixture's profiles the
//! same way, and the 2 by 2 cell grid. An intersection digest covers the
//! coordinates, every pair and meeting bit for bit, and the intersection
//! check's report; blessed only for runs the checkers pass.
#![allow(clippy::unwrap_used, clippy::expect_used)] // tests: a failing unwrap is the test failing (clippy.toml)

use laman::intersect::intersections;
use laman::profile::profiles;
use laman::solution::{Algorithm, SolverOptions};
use laman_check::{check_intersections, check_profiles};
use laman_debug::corpus::{cell_grid, slot_grid};
use laman_debug::digest::{
    of_intersections, of_profiles, of_solved_intersections, of_solved_profiles,
};
use laman_debug::fixture::Fixture;
use laman_debug::runner::{drag_run, run, solve_run};

/// Name, sketch.json, expected.json.
const CORPUS: [(&str, &str, &str); 10] = [
    (
        "sss-triangle",
        include_str!("../../../tests/fixtures/c1/sss-triangle/sketch.json"),
        include_str!("../../../tests/fixtures/c1/sss-triangle/expected.json"),
    ),
    (
        "sss-triangle-redundant",
        include_str!("../../../tests/fixtures/c1/sss-triangle-redundant/sketch.json"),
        include_str!("../../../tests/fixtures/c1/sss-triangle-redundant/expected.json"),
    ),
    (
        "sss-triangle-conflicting",
        include_str!("../../../tests/fixtures/c1/sss-triangle-conflicting/sketch.json"),
        include_str!("../../../tests/fixtures/c1/sss-triangle-conflicting/expected.json"),
    ),
    (
        "belt",
        include_str!("../../../tests/fixtures/c1/belt/sketch.json"),
        include_str!("../../../tests/fixtures/c1/belt/expected.json"),
    ),
    (
        "belt-redundant",
        include_str!("../../../tests/fixtures/c1/belt-redundant/sketch.json"),
        include_str!("../../../tests/fixtures/c1/belt-redundant/expected.json"),
    ),
    (
        "belt-conflicting",
        include_str!("../../../tests/fixtures/c1/belt-conflicting/sketch.json"),
        include_str!("../../../tests/fixtures/c1/belt-conflicting/expected.json"),
    ),
    (
        "slotted-plate",
        include_str!("../../../tests/fixtures/c1/slotted-plate/sketch.json"),
        include_str!("../../../tests/fixtures/c1/slotted-plate/expected.json"),
    ),
    (
        "slotted-plate-redundant",
        include_str!("../../../tests/fixtures/c1/slotted-plate-redundant/sketch.json"),
        include_str!("../../../tests/fixtures/c1/slotted-plate-redundant/expected.json"),
    ),
    (
        "slotted-plate-conflicting",
        include_str!("../../../tests/fixtures/c1/slotted-plate-conflicting/sketch.json"),
        include_str!("../../../tests/fixtures/c1/slotted-plate-conflicting/expected.json"),
    ),
    (
        "slotted-plate-drag",
        include_str!("../../../tests/fixtures/c1/slotted-plate-drag/sketch.json"),
        include_str!("../../../tests/fixtures/c1/slotted-plate-drag/expected.json"),
    ),
];

/// The blessed digests, one `<name> <hex>` per line: a run digest under the
/// fixture's name, `solved-<lm|dogleg>-<name>`, `drag-<lm|dogleg>-step-NN`.
const DIGESTS: &str = include_str!("../../../tests/fixtures/c1/digests.txt");

/// The violations the checker must report at a conflicting twin's best
/// effort (derived by analysis in `c1.rs`); every other run is clean.
const VIOLATIONS: &str = include_str!("../../../tests/fixtures/c1/violations.json");

fn blessed(name: &str) -> &'static str {
    DIGESTS
        .lines()
        .find_map(|line| line.strip_prefix(name)?.strip_prefix(' '))
        .expect("a blessed digest for the name")
}

#[test]
fn corpus_digests_match_the_blessed_ones() {
    let mut wrong = Vec::new();
    for (name, sketch, expected) in CORPUS {
        let fixture = Fixture::parse(sketch, expected, None).unwrap();
        let digest = run(&fixture).digest.to_string();
        println!("\ndigest {name} {digest}");
        if digest != blessed(name) {
            wrong.push(name);
        }
    }
    assert!(
        wrong.is_empty(),
        "digests differ from the blessed ones: {wrong:?}"
    );
}

#[test]
fn solved_digests_match_the_blessed_ones() {
    let violations: std::collections::BTreeMap<String, Vec<String>> =
        serde_json::from_str(VIOLATIONS).unwrap();
    let mut wrong = Vec::new();
    for (name, sketch, expected) in CORPUS {
        let fixture = Fixture::parse(sketch, expected, None).unwrap();
        for (algorithm, label) in [
            (Algorithm::LevenbergMarquardt, "lm"),
            (Algorithm::DogLeg, "dogleg"),
        ] {
            let mut options = SolverOptions::default();
            options.algorithm = algorithm;
            let solved = solve_run(&fixture, &options);
            // Bless only checker-validated runs: clean, or for a conflicting
            // twin the violations derived by analysis in `c1.rs`.
            let mut flagged: Vec<String> = solved
                .report
                .violations
                .iter()
                .map(|v| v.tag.clone().unwrap_or_default())
                .collect();
            flagged.sort();
            let want = violations.get(name).cloned().unwrap_or_default();
            assert_eq!(flagged, want, "{label} {name}: the checker's verdict");
            assert!(solved.report.degenerate.is_empty(), "{label} {name}");
            assert!(solved.report.missing.is_empty(), "{label} {name}");
            let digest = solved.digest.to_string();
            println!("\ndigest solved-{label}-{name} {digest}");
            if digest != blessed(&format!("solved-{label}-{name}")) {
                wrong.push(format!("{label} {name}"));
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "solved digests differ from the blessed ones: {wrong:?}"
    );
}

const DRAG_SKETCH: &str = include_str!("../../../tests/fixtures/c1/slotted-plate-drag/sketch.json");
const DRAG_EXPECTED: &str =
    include_str!("../../../tests/fixtures/c1/slotted-plate-drag/expected.json");
const DRAG_FILE: &str = include_str!("../../../tests/fixtures/c1/slotted-plate-drag/drag.json");

#[test]
fn embedded_fixtures_use_lf_line_endings() {
    for (name, sketch, expected) in CORPUS {
        assert!(!sketch.contains('\r'), "{name}/sketch.json contains CR");
        assert!(!expected.contains('\r'), "{name}/expected.json contains CR");
    }
    for (name, text) in [
        ("digests.txt", DIGESTS),
        ("violations.json", VIOLATIONS),
        ("slotted-plate-drag/drag.json", DRAG_FILE),
    ] {
        assert!(!text.contains('\r'), "{name} contains CR");
    }
}

#[test]
fn drag_step_digests_match_the_blessed_ones() {
    let fixture = Fixture::parse(DRAG_SKETCH, DRAG_EXPECTED, Some(DRAG_FILE)).unwrap();
    let mut wrong = Vec::new();
    for (algorithm, label) in [
        (Algorithm::LevenbergMarquardt, "lm"),
        (Algorithm::DogLeg, "dogleg"),
    ] {
        let mut options = SolverOptions::default();
        options.algorithm = algorithm;
        let steps = drag_run(&fixture, &options).unwrap();
        assert_eq!(steps.len(), 10);
        for (k, step) in steps.iter().enumerate() {
            // Bless only checker-validated steps.
            assert!(step.report.is_clean(), "{label} step {}", k + 1);
            let digest = step.digest.to_string();
            let name = format!("drag-{label}-step-{:02}", k + 1);
            println!("\ndigest {name} {digest}");
            if digest != blessed(&name) {
                wrong.push(format!("{label} step {}", k + 1));
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "drag digests differ from the blessed ones: {wrong:?}"
    );
}

/// The C3 fixtures: name, sketch.json, expected.json.
const C3_CORPUS: [(&str, &str, &str); 10] = [
    (
        "rectangle",
        include_str!("../../../tests/fixtures/c3/rectangle/sketch.json"),
        include_str!("../../../tests/fixtures/c3/rectangle/expected.json"),
    ),
    (
        "slotted-plate",
        include_str!("../../../tests/fixtures/c3/slotted-plate/sketch.json"),
        include_str!("../../../tests/fixtures/c3/slotted-plate/expected.json"),
    ),
    (
        "slotted-plate-coincident",
        include_str!("../../../tests/fixtures/c3/slotted-plate-coincident/sketch.json"),
        include_str!("../../../tests/fixtures/c3/slotted-plate-coincident/expected.json"),
    ),
    (
        "plate-island",
        include_str!("../../../tests/fixtures/c3/plate-island/sketch.json"),
        include_str!("../../../tests/fixtures/c3/plate-island/expected.json"),
    ),
    (
        "two-loops",
        include_str!("../../../tests/fixtures/c3/two-loops/sketch.json"),
        include_str!("../../../tests/fixtures/c3/two-loops/expected.json"),
    ),
    (
        "circle-alone",
        include_str!("../../../tests/fixtures/c3/circle-alone/sketch.json"),
        include_str!("../../../tests/fixtures/c3/circle-alone/expected.json"),
    ),
    (
        "open-chain",
        include_str!("../../../tests/fixtures/c3/open-chain/sketch.json"),
        include_str!("../../../tests/fixtures/c3/open-chain/expected.json"),
    ),
    (
        "dangling-edge",
        include_str!("../../../tests/fixtures/c3/dangling-edge/sketch.json"),
        include_str!("../../../tests/fixtures/c3/dangling-edge/expected.json"),
    ),
    (
        "shared-edge",
        include_str!("../../../tests/fixtures/c3/shared-edge/sketch.json"),
        include_str!("../../../tests/fixtures/c3/shared-edge/expected.json"),
    ),
    (
        "unjoined",
        include_str!("../../../tests/fixtures/c3/unjoined/sketch.json"),
        include_str!("../../../tests/fixtures/c3/unjoined/expected.json"),
    ),
];

/// The C3 digests, one `<name> <hex>` per line, under `c3-` names.
const C3_DIGESTS: &str = include_str!("../../../tests/fixtures/c3/digests.txt");

fn c3_blessed(name: &str) -> &'static str {
    C3_DIGESTS
        .lines()
        .find_map(|line| line.strip_prefix(name)?.strip_prefix(' '))
        .expect("a blessed C3 digest for the name")
}

#[test]
fn c3_digests_match_the_blessed_ones() {
    let mut wrong = Vec::new();
    let mut note = |name: String, digest: String| {
        println!("\ndigest {name} {digest}");
        if digest != c3_blessed(&name) {
            wrong.push(name);
        }
    };
    for (name, sketch, expected) in C3_CORPUS {
        assert!(!sketch.contains('\r') && !expected.contains('\r'), "{name}");
        let fixture = Fixture::parse(sketch, expected, None).unwrap();
        let mut values = fixture.sketch.coordinates();
        for (key, value) in &fixture.expected.coordinates {
            values.insert(*key, *value);
        }
        let found = profiles(&fixture.sketch, &values).unwrap();
        let report = check_profiles(&fixture.sketch, &values, &found);
        assert!(report.is_clean(), "{name}: {report:#?}");
        note(
            format!("c3-{name}"),
            of_profiles(&fixture.sketch, &values, &found, &report).to_string(),
        );
        for (algorithm, label) in [
            (Algorithm::LevenbergMarquardt, "lm"),
            (Algorithm::DogLeg, "dogleg"),
        ] {
            let mut options = SolverOptions::default();
            options.algorithm = algorithm;
            let solved = solve_run(&fixture, &options);
            assert!(solved.report.is_clean(), "{label} {name}");
            let solution = &solved.solution;
            let found = profiles(&fixture.sketch, &solution.coordinates).unwrap();
            let profile_report = check_profiles(&fixture.sketch, &solution.coordinates, &found);
            assert!(
                profile_report.is_clean(),
                "{label} {name}: {profile_report:#?}"
            );
            note(
                format!("c3-solved-{label}-{name}"),
                of_solved_profiles(
                    &fixture.sketch,
                    solution,
                    &solved.report,
                    &found,
                    &profile_report,
                )
                .to_string(),
            );
        }
    }
    let case = slot_grid(3, 3).unwrap();
    let mut values = case.sketch.coordinates();
    for (key, value) in &case.expected.coordinates {
        values.insert(*key, *value);
    }
    let found = profiles(&case.sketch, &values).unwrap();
    let report = check_profiles(&case.sketch, &values, &found);
    assert!(report.is_clean(), "slot-grid-3x3: {report:#?}");
    note(
        "c3-slot-grid-3x3".to_owned(),
        of_profiles(&case.sketch, &values, &found, &report).to_string(),
    );
    assert!(
        wrong.is_empty(),
        "C3 digests differ from the blessed ones: {wrong:?}"
    );
}

/// The C4 intersection fixtures: name, sketch.json, expected.json.
const C4_INTERSECTIONS: [(&str, &str, &str); 8] = [
    (
        "crossing-lines",
        include_str!("../../../tests/fixtures/c4/crossing-lines/sketch.json"),
        include_str!("../../../tests/fixtures/c4/crossing-lines/expected.json"),
    ),
    (
        "line-through-circle",
        include_str!("../../../tests/fixtures/c4/line-through-circle/sketch.json"),
        include_str!("../../../tests/fixtures/c4/line-through-circle/expected.json"),
    ),
    (
        "two-circles",
        include_str!("../../../tests/fixtures/c4/two-circles/sketch.json"),
        include_str!("../../../tests/fixtures/c4/two-circles/expected.json"),
    ),
    (
        "tangent-touch",
        include_str!("../../../tests/fixtures/c4/tangent-touch/sketch.json"),
        include_str!("../../../tests/fixtures/c4/tangent-touch/expected.json"),
    ),
    (
        "collinear-overlap",
        include_str!("../../../tests/fixtures/c4/collinear-overlap/sketch.json"),
        include_str!("../../../tests/fixtures/c4/collinear-overlap/expected.json"),
    ),
    (
        "arc-outside-sweep",
        include_str!("../../../tests/fixtures/c4/arc-outside-sweep/sketch.json"),
        include_str!("../../../tests/fixtures/c4/arc-outside-sweep/expected.json"),
    ),
    (
        "t-end-on-line",
        include_str!("../../../tests/fixtures/c4/t-end-on-line/sketch.json"),
        include_str!("../../../tests/fixtures/c4/t-end-on-line/expected.json"),
    ),
    (
        "arc-crosses-line",
        include_str!("../../../tests/fixtures/c4/arc-crosses-line/sketch.json"),
        include_str!("../../../tests/fixtures/c4/arc-crosses-line/expected.json"),
    ),
];

/// The C4 profile fixtures (branched cores): name, sketch.json, expected.json.
const C4_PROFILES: [(&str, &str, &str); 6] = [
    (
        "shared-wall",
        include_str!("../../../tests/fixtures/c4/shared-wall/sketch.json"),
        include_str!("../../../tests/fixtures/c4/shared-wall/expected.json"),
    ),
    (
        "window",
        include_str!("../../../tests/fixtures/c4/window/sketch.json"),
        include_str!("../../../tests/fixtures/c4/window/expected.json"),
    ),
    (
        "tangent-branch",
        include_str!("../../../tests/fixtures/c4/tangent-branch/sketch.json"),
        include_str!("../../../tests/fixtures/c4/tangent-branch/expected.json"),
    ),
    (
        "figure-eight",
        include_str!("../../../tests/fixtures/c4/figure-eight/sketch.json"),
        include_str!("../../../tests/fixtures/c4/figure-eight/expected.json"),
    ),
    (
        "bridged-plate",
        include_str!("../../../tests/fixtures/c4/bridged-plate/sketch.json"),
        include_str!("../../../tests/fixtures/c4/bridged-plate/expected.json"),
    ),
    (
        "branched-in-plate",
        include_str!("../../../tests/fixtures/c4/branched-in-plate/sketch.json"),
        include_str!("../../../tests/fixtures/c4/branched-in-plate/expected.json"),
    ),
];

/// The C4 digests, one `<name> <hex>` per line, under `c4-` names: a
/// fixture's intersections or profiles at its expected coordinates
/// (`c4-<name>`) and at each algorithm's solution
/// (`c4-solved-<lm|dogleg>-<name>`), and the 2 by 2 cell grid's profiles
/// and intersections at its expected coordinates.
const C4_DIGESTS: &str = include_str!("../../../tests/fixtures/c4/digests.txt");

fn c4_blessed(name: &str) -> &'static str {
    C4_DIGESTS
        .lines()
        .find_map(|line| line.strip_prefix(name)?.strip_prefix(' '))
        .unwrap_or("not blessed")
}

fn expected_values(fixture: &Fixture) -> laman::model::Coordinates {
    let mut values = fixture.sketch.coordinates();
    for (key, value) in &fixture.expected.coordinates {
        values.insert(*key, *value);
    }
    values
}

#[test]
fn c4_digests_match_the_blessed_ones() {
    let mut wrong = Vec::new();
    let mut note = |name: String, digest: String| {
        println!("\ndigest {name} {digest}");
        if digest != c4_blessed(&name) {
            wrong.push(name);
        }
    };
    let algorithms = [
        (Algorithm::LevenbergMarquardt, "lm"),
        (Algorithm::DogLeg, "dogleg"),
    ];
    for (name, sketch, expected) in C4_INTERSECTIONS {
        assert!(!sketch.contains('\r') && !expected.contains('\r'), "{name}");
        let fixture = Fixture::parse(sketch, expected, None).unwrap();
        let values = expected_values(&fixture);
        let found = intersections(&fixture.sketch, &values).unwrap();
        let report = check_intersections(&fixture.sketch, &values, &found);
        assert!(report.is_clean(), "{name}: {report:#?}");
        note(
            format!("c4-{name}"),
            of_intersections(&fixture.sketch, &values, &found, &report).to_string(),
        );
        for (algorithm, label) in algorithms {
            let mut options = SolverOptions::default();
            options.algorithm = algorithm;
            let solved = solve_run(&fixture, &options);
            assert!(solved.report.is_clean(), "{label} {name}");
            let solution = &solved.solution;
            let found = intersections(&fixture.sketch, &solution.coordinates).unwrap();
            let checked = check_intersections(&fixture.sketch, &solution.coordinates, &found);
            assert!(checked.is_clean(), "{label} {name}: {checked:#?}");
            note(
                format!("c4-solved-{label}-{name}"),
                of_solved_intersections(
                    &fixture.sketch,
                    solution,
                    &solved.report,
                    &found,
                    &checked,
                )
                .to_string(),
            );
        }
    }
    for (name, sketch, expected) in C4_PROFILES {
        assert!(!sketch.contains('\r') && !expected.contains('\r'), "{name}");
        let fixture = Fixture::parse(sketch, expected, None).unwrap();
        let values = expected_values(&fixture);
        let found = profiles(&fixture.sketch, &values).unwrap();
        let report = check_profiles(&fixture.sketch, &values, &found);
        assert!(report.is_clean(), "{name}: {report:#?}");
        note(
            format!("c4-{name}"),
            of_profiles(&fixture.sketch, &values, &found, &report).to_string(),
        );
        for (algorithm, label) in algorithms {
            let mut options = SolverOptions::default();
            options.algorithm = algorithm;
            let solved = solve_run(&fixture, &options);
            assert!(solved.report.is_clean(), "{label} {name}");
            let solution = &solved.solution;
            let found = profiles(&fixture.sketch, &solution.coordinates).unwrap();
            let checked = check_profiles(&fixture.sketch, &solution.coordinates, &found);
            assert!(checked.is_clean(), "{label} {name}: {checked:#?}");
            note(
                format!("c4-solved-{label}-{name}"),
                of_solved_profiles(&fixture.sketch, solution, &solved.report, &found, &checked)
                    .to_string(),
            );
        }
    }
    let case = cell_grid(2, 2).unwrap();
    let mut values = case.sketch.coordinates();
    for (key, value) in &case.expected.coordinates {
        values.insert(*key, *value);
    }
    let found = profiles(&case.sketch, &values).unwrap();
    let report = check_profiles(&case.sketch, &values, &found);
    assert!(report.is_clean(), "cell-grid-2x2: {report:#?}");
    note(
        "c4-cell-grid-2x2".to_owned(),
        of_profiles(&case.sketch, &values, &found, &report).to_string(),
    );
    let found = intersections(&case.sketch, &values).unwrap();
    let report = check_intersections(&case.sketch, &values, &found);
    assert!(
        report.is_clean(),
        "cell-grid-2x2 intersections: {report:#?}"
    );
    note(
        "c4-cell-grid-2x2-intersections".to_owned(),
        of_intersections(&case.sketch, &values, &found, &report).to_string(),
    );
    assert!(
        wrong.is_empty(),
        "C4 digests differ from the blessed ones: {wrong:?}"
    );
}
