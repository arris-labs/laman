#![no_main]

//! Any sketch the loader accepts, at its own coordinates and at a solution's:
//! `laman_check::check_intersections` reads what `intersections()` found and
//! a copy mutated by a byte hashed from the input (a meeting dropped, moved,
//! made non-finite or of another kind, a pair dropped, swapped, emptied,
//! repeated or stale) without a panic, gives the same report on a second
//! call, and is clean on the found intersections wherever `laman_check::check`
//! is clean at the same coordinates (ADR-0018). A panic, a differing second
//! report, a violation named on the kernel's own result, or a hang is the
//! finding.

use laman::intersect::intersections;
use laman::model::intersection::{Intersection, Intersections, MeetingKind};
use laman::model::{EntityId, Sketch, SketchForm};
use laman::solution::SolverOptions;
use laman_check::check_intersections;
use libfuzzer_sys::fuzz_target;

/// Sketches larger than this are skipped: the fuzzer's job is malformed
/// and extreme input, and the solve it makes costs more with the sketch.
const MAX_ENTITIES: usize = 24;

/// An `Intersections` value measuring would not give, chosen by `pick`.
fn mutated(found: &Intersections, pick: u8) -> Intersections {
    let mut m = found.clone();
    let Some(pair) = m.pairs.first_mut() else {
        // Nothing found: a pair of stale ids, which no sketch holds.
        m.pairs.push(Intersection {
            first: EntityId(u32::MAX - 1),
            second: EntityId(u32::MAX),
            meetings: vec![],
        });
        return m;
    };
    match pick % 8 {
        0 => {
            pair.meetings.pop();
        }
        1 => {
            if let Some(meeting) = pair.meetings.first_mut() {
                meeting.at = [meeting.at[0] * 1.5 + 1.0, f64::MAX];
            }
        }
        2 => {
            if let Some(meeting) = pair.meetings.first_mut() {
                meeting.on_first = f64::NAN;
                meeting.at = [f64::INFINITY, f64::NAN];
            }
        }
        3 => {
            if let Some(meeting) = pair.meetings.first_mut() {
                meeting.kind = match meeting.kind {
                    MeetingKind::Crossing => MeetingKind::Touch,
                    MeetingKind::Touch | MeetingKind::Overlap { .. } => MeetingKind::Crossing,
                };
            }
        }
        4 => std::mem::swap(&mut pair.first, &mut pair.second),
        5 => pair.meetings.clear(),
        6 => {
            let twice = pair.clone();
            m.pairs.push(twice);
        }
        _ => {
            m.pairs.remove(0);
        }
    }
    m
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
    let pick = data
        .iter()
        .fold(0_u8, |h, &b| h.wrapping_mul(31).wrapping_add(b));
    for values in [sketch.coordinates(), solved] {
        let Ok(found) = intersections(&sketch, &values) else {
            // Values lacking a coordinate: the checker names it, no panic.
            let _ = check_intersections(&sketch, &values, &Intersections::default());
            continue;
        };
        let report = check_intersections(&sketch, &values, &found);
        assert_eq!(report, check_intersections(&sketch, &values, &found));
        if laman_check::check(&sketch, &values).is_clean() {
            assert!(report.is_clean(), "{report:#?}");
        }
        let _ = check_intersections(&sketch, &values, &mutated(&found, pick));
    }
});
