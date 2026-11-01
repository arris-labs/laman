#![no_main]

//! Any bytes are a sketch file or a refusal: `serde_json` reads them into the
//! serde form, and the loader replays the builder, which returns a sketch or a
//! `ModelError`. A panic anywhere on that path is the finding
//! (`.agents/rules/solver.md` §Correctness).

use laman::model::{Sketch, SketchForm};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(form) = serde_json::from_slice::<SketchForm>(data) else {
        return;
    };
    let _ = Sketch::from_form(form);
});
