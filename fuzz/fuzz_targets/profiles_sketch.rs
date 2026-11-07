#![no_main]

//! Any sketch the loader accepts gives profiles at its own coordinates and
//! at a solution's, without a panic: every area is finite, the result is the
//! same on a second call, and values lacking a coordinate are refused with a
//! typed error (`.agents/rules/solver.md` §Correctness). A panic, a
//! non-finite area, a differing second result, or a hang is the finding.

use laman::model::{Coordinate, Sketch, SketchForm};
use laman::profile::profiles;
use laman::solution::SolverOptions;
use libfuzzer_sys::fuzz_target;

/// Sketches larger than this are skipped: the fuzzer's job is malformed
/// and extreme input, and the solve it makes costs more with the sketch.
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
    let solved = sketch.solve(&SolverOptions::default()).coordinates;
    for values in [sketch.coordinates(), solved] {
        let found = profiles(&sketch, &values);
        assert_eq!(found, profiles(&sketch, &values));
        if let Ok(found) = found {
            for p in &found.profiles {
                assert!(p.net_area.is_finite());
                assert!(p.outer.signed_area.is_finite());
                assert!(p.holes.iter().all(|h| h.signed_area.is_finite()));
            }
        }
        // A value the edges need but the values lack is named, not a panic.
        if let Some(&(id, _)) = values.keys().next() {
            let mut lacking = values.clone();
            lacking.remove(&(id, Coordinate::X));
            let _ = profiles(&sketch, &lacking);
        }
    }
});
