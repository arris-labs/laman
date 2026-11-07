#![no_main]

//! Any sketch the loader accepts gives profiles at its own coordinates and
//! at a solution's, without a panic: every area is finite, the result is the
//! same on a second call, and values lacking a coordinate are refused with a
//! typed error (`.agents/rules/solver.md` §Correctness). A panic, a
//! non-finite area, a differing second result, or a hang is the finding.
//!
//! The checker (`laman_check::check_profiles`) then reads what was found and
//! a copy mutated by a byte hashed from the input (a loop reversed, an edge dropped,
//! repeated or stale, an empty loop, a hole moved, an area made non-finite or
//! extreme): never a panic, and clean on the found profiles wherever
//! `laman_check::check` is clean at the same coordinates (ADR-0013).

use laman::model::profile::{Loop, Profiles, Traversal};
use laman::model::{Coordinate, EntityId, Sketch, SketchForm};
use laman::profile::profiles;
use laman::solution::SolverOptions;
use libfuzzer_sys::fuzz_target;

/// Sketches larger than this are skipped: the fuzzer's job is malformed
/// and extreme input, and the solve it makes costs more with the sketch.
const MAX_ENTITIES: usize = 24;

/// A `Profiles` value extraction would not give, chosen by `pick`.
fn mutated(found: &Profiles, pick: u8) -> Profiles {
    let mut m = found.clone();
    let Some(profile) = m.profiles.first_mut() else {
        // Nothing found: a loop of a stale edge, which no sketch holds.
        let stale = Loop {
            edges: vec![Traversal {
                entity: EntityId(u32::MAX),
                reversed: pick & 1 == 1,
            }],
            signed_area: f64::NAN,
            joins: vec![],
        };
        m.profiles.push(laman::model::profile::Profile {
            outer: stale,
            holes: vec![],
            net_area: 0.0,
        });
        return m;
    };
    match pick % 9 {
        0 => profile.outer.signed_area = -profile.outer.signed_area,
        1 => profile.outer.edges.clear(),
        2 => {
            profile.outer.edges.pop();
        }
        3 => {
            if let Some(&t) = profile.outer.edges.first() {
                profile.outer.edges.push(t);
            }
        }
        4 => {
            for t in &mut profile.outer.edges {
                t.reversed = !t.reversed;
            }
        }
        5 => {
            if let Some(t) = profile.outer.edges.first_mut() {
                t.entity = EntityId(u32::MAX);
            }
        }
        6 => profile.outer.signed_area = f64::NAN,
        7 => profile.net_area = f64::MAX,
        _ => {
            if let Some(hole) = profile.holes.pop() {
                if let Some(other) = m.profiles.last_mut() {
                    other.holes.push(hole);
                }
            } else {
                profile.outer.signed_area = f64::INFINITY;
            }
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
    for values in [sketch.coordinates(), solved] {
        let found = profiles(&sketch, &values);
        assert_eq!(found, profiles(&sketch, &values));
        if let Ok(found) = found {
            let report = laman_check::check_profiles(&sketch, &values, &found);
            if laman_check::check(&sketch, &values).is_clean() {
                assert!(report.is_clean(), "{report:#?}");
            }
            let pick = data.iter().fold(0_u8, |h, &b| h.wrapping_mul(31).wrapping_add(b));
            let _ = laman_check::check_profiles(&sketch, &values, &mutated(&found, pick));
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
