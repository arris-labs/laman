#![no_main]

//! Any sketch the loader accepts solves, under both algorithms, to a
//! complete answer without a panic, and the answer applies back to the sketch
//! it was solved from (`.agents/rules/solver.md` §Correctness). A panic, a
//! hang past the libFuzzer timeout, or a refused `apply` is the finding.

use laman::model::{Sketch, SketchForm};
use laman::solution::{Algorithm, SolverOptions};
use libfuzzer_sys::fuzz_target;

/// Sketches larger than this are skipped: the fuzzer's job is malformed
/// and extreme input, and a solve's cost grows with the sketch.
const MAX_ENTITIES: usize = 24;

fuzz_target!(|data: &[u8]| {
    let Ok(form) = serde_json::from_slice::<SketchForm>(data) else {
        return;
    };
    let Ok(sketch) = Sketch::from_form(form) else {
        return;
    };
    if sketch.entities().count() > MAX_ENTITIES {
        return;
    }
    for algorithm in [Algorithm::LevenbergMarquardt, Algorithm::DogLeg] {
        let mut options = SolverOptions::default();
        options.algorithm = algorithm;
        let solution = sketch.solve(&options);
        assert_eq!(solution.coordinates.len(), sketch.coordinates().len());
        let mut applied = sketch.clone();
        assert!(applied.apply(&solution).is_ok());
    }
});
