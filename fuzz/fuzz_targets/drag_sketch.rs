#![no_main]

//! Any sketch the loader accepts, any point of it, any targets: a drag
//! session opens or refuses with a typed error, every `to` returns a
//! complete answer without a panic (a target that is not finite comes back
//! `NotConverged`), and `finish` returns a solution of the sketch's
//! parameters. A panic or a hang past the libFuzzer timeout is the finding.
//!
//! Input layout: one byte picking the point, one byte for the number of
//! targets (1 to 4), then 16 bytes per target (two `f64`s by their raw
//! bits, so NaN, infinities and extremes are all reached), then the sketch's
//! serde JSON.

use laman::model::{Sketch, SketchForm};
use laman::solution::{Algorithm, SolverOptions, Status};
use libfuzzer_sys::fuzz_target;

/// Sketches larger than this are skipped, as in `solve_sketch`.
const MAX_ENTITIES: usize = 24;

fn float(bytes: &[u8]) -> f64 {
    let mut raw = [0_u8; 8];
    raw.copy_from_slice(bytes);
    f64::from_bits(u64::from_le_bytes(raw))
}

fuzz_target!(|data: &[u8]| {
    let [pick, count, rest @ ..] = data else {
        return;
    };
    let count = usize::from(*count % 4) + 1;
    if rest.len() < 16 * count {
        return;
    }
    let (raw, json) = rest.split_at(16 * count);
    let targets: Vec<[f64; 2]> = raw
        .chunks_exact(16)
        .map(|c| [float(&c[..8]), float(&c[8..])])
        .collect();
    let Ok(form) = serde_json::from_slice::<SketchForm>(json) else {
        return;
    };
    let Ok(sketch) = Sketch::from_form(form) else {
        return;
    };
    if sketch.entities().count() > MAX_ENTITIES {
        return;
    }
    let ids: Vec<_> = sketch.entities().map(|(id, _)| id).collect();
    let Some(&id) = ids.get(usize::from(*pick) % ids.len().max(1)) else {
        return;
    };
    for algorithm in [Algorithm::LevenbergMarquardt, Algorithm::DogLeg] {
        let mut options = SolverOptions::default();
        options.algorithm = algorithm;
        let Ok(mut session) = sketch.drag(id, &options) else {
            continue;
        };
        for target in &targets {
            let step = session.to(*target);
            assert_eq!(step.coordinates.len(), sketch.coordinates().len());
            if !(target[0].is_finite() && target[1].is_finite()) {
                assert_eq!(step.status, Status::NotConverged);
            }
        }
        assert_eq!(session.finish().coordinates.len(), sketch.coordinates().len());
    }
});
