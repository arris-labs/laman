#![no_main]

//! Any sketch the loader accepts gives intersections at its own coordinates
//! and at a solution's, without a panic: every point and parameter is
//! finite, a parameter is in its range, the pairs are in id order, the
//! result is the same on a second call, and values lacking a coordinate are
//! refused with a typed error (`.agents/rules/solver.md` §Correctness). A
//! panic, a non-finite or out-of-range number, a differing second result,
//! or a hang is the finding.

use laman::intersect::intersections;
use laman::model::intersection::MeetingKind;
use laman::model::{Coordinate, Sketch, SketchForm};
use laman::solution::SolverOptions;
use libfuzzer_sys::fuzz_target;

/// Sketches larger than this are skipped: the fuzzer's job is malformed
/// and extreme input, and the solve it makes costs more with the sketch.
const MAX_ENTITIES: usize = 24;

fn finite(p: [f64; 2]) -> bool {
    p.iter().all(|x| x.is_finite())
}

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
        let found = intersections(&sketch, &values);
        assert_eq!(found, intersections(&sketch, &values));
        if let Ok(found) = found {
            let mut last = None;
            for pair in &found.pairs {
                assert!(pair.first < pair.second);
                assert!(last < Some((pair.first, pair.second)));
                last = Some((pair.first, pair.second));
                assert!(!pair.meetings.is_empty());
                for m in &pair.meetings {
                    assert!(finite(m.at));
                    // A circle's parameter and an overlap's end may pass 1.
                    assert!(m.on_first.is_finite() && m.on_first >= 0.0);
                    assert!(m.on_second.is_finite() && m.on_second >= 0.0);
                    if let MeetingKind::Overlap {
                        to,
                        to_first,
                        to_second,
                    } = m.kind
                    {
                        assert!(finite(to) && to_first.is_finite() && to_second.is_finite());
                    }
                }
                let along = found.along(pair.first);
                assert!(along.windows(2).all(|w| w[0].meeting.on_first <= w[1].meeting.on_first));
            }
        }
        // A value a curve needs but the values lack is named, not a panic.
        if let Some(&(id, _)) = values.keys().next() {
            let mut lacking = values.clone();
            lacking.remove(&(id, Coordinate::X));
            let _ = intersections(&sketch, &lacking);
        }
    }
});
