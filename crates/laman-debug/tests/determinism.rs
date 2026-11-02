//! The determinism corpus: the digest of every C1 fixture and twin, blessed
//! in `tests/fixtures/c1/digests.txt`. `tools/digest-wasm.sh` runs this same test natively and on
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
#![allow(clippy::unwrap_used, clippy::expect_used)] // tests: a failing unwrap is the test failing (clippy.toml)

use laman::solution::{Algorithm, SolverOptions};
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
