//! Broken twins of the C4 corpus: each fixture's closed-form result is
//! clean, and the same result with one thing broken names its variant
//! (ADR-0018). A twin moves a meeting or a loop by far more than the
//! undecided band, so none is a matter of rounding. The intersection corpus
//! also runs scaled by 1e-6 and 1e6 and shifted far from the origin, where
//! the checker stays clean and the kernel's kinds and counts do not change
//! (the m12 review's backlog line).
#![allow(clippy::unwrap_used, clippy::expect_used)] // tests: a failing unwrap is the test failing (clippy.toml)

use laman::intersect::intersections;
use laman::model::intersection::{Intersections, MeetingKind};
use laman::model::profile::{Loop, Profiles, Traversal};
use laman::model::{Coordinate, Coordinates, EntityId, Sketch};
use laman::profile::profiles;
use laman_check::{
    IntersectionViolation as V, ProfileViolation as P, check_intersections, check_profiles,
};
use laman_debug::fixture::Fixture;
use std::path::PathBuf;

const INTERSECTION_FIXTURES: [&str; 8] = [
    "crossing-lines",
    "line-through-circle",
    "two-circles",
    "tangent-touch",
    "collinear-overlap",
    "arc-outside-sweep",
    "t-end-on-line",
    "arc-crosses-line",
];

fn load(group: &str, name: &str) -> (Sketch, Coordinates) {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(group)
        .join(name);
    let fixture = Fixture::load(&dir).unwrap();
    let mut values = fixture.sketch.coordinates();
    for (key, value) in &fixture.expected.coordinates {
        values.insert(*key, *value);
    }
    (fixture.sketch, values)
}

fn pairs_kinds(found: &Intersections) -> Vec<Vec<u8>> {
    let kind = |k: MeetingKind| match k {
        MeetingKind::Crossing => 0,
        MeetingKind::Touch => 1,
        MeetingKind::Overlap { .. } => 2,
    };
    found
        .pairs
        .iter()
        .map(|p| p.meetings.iter().map(|m| kind(m.kind)).collect())
        .collect()
}

/// The corpus scaled by `scale` and shifted by `shift` sizes: radii scale,
/// points scale and shift.
fn moved(values: &Coordinates, scale: f64, shift: f64) -> Coordinates {
    values
        .iter()
        .map(|(&(id, c), &v)| {
            let moved = match c {
                Coordinate::X => (v + shift * 1.0) * scale,
                Coordinate::Y => (v - shift * 0.7) * scale,
                Coordinate::R => v * scale,
            };
            ((id, c), moved)
        })
        .collect()
}

#[test]
fn the_intersection_corpus_is_clean_scaled_and_shifted() {
    for name in INTERSECTION_FIXTURES {
        let (sk, values) = load("c4", name);
        let base = intersections(&sk, &values).unwrap();
        for (scale, shift) in [(1.0, 0.0), (1e-6, 0.0), (1e6, 0.0), (1e-6, 1e3), (1e6, 1e3)] {
            let at = moved(&values, scale, shift);
            let found = intersections(&sk, &at).unwrap();
            assert_eq!(
                pairs_kinds(&found),
                pairs_kinds(&base),
                "{name} at {scale:e} shifted {shift:e}"
            );
            let report = check_intersections(&sk, &at, &found);
            assert!(
                report.is_clean(),
                "{name} at {scale:e} shifted {shift:e}: {report:#?}"
            );
        }
    }
}

/// Each fixture's result with one thing broken.
#[test]
fn a_broken_intersection_names_its_variant() {
    let mut broken = 0;
    for name in INTERSECTION_FIXTURES {
        let (sk, values) = load("c4", name);
        let found = intersections(&sk, &values).unwrap();
        let violations = |f: &Intersections| check_intersections(&sk, &values, f).violations;
        assert!(violations(&found).is_empty(), "{name}");
        for (p, pair) in found.pairs.iter().enumerate() {
            let (first, second) = (pair.first, pair.second);
            // The pair dropped.
            let mut twin = found.clone();
            twin.pairs.remove(p);
            assert!(
                violations(&twin).contains(&V::Missed { first, second }),
                "{name}: dropped pair"
            );
            for (m, meeting) in pair.meetings.iter().enumerate() {
                let at_m = |v: &V| match v {
                    V::OffCurve { meeting, .. }
                    | V::Parameter { meeting, .. }
                    | V::Kind { meeting, .. }
                    | V::OutsideSweep { meeting, .. } => *meeting == m,
                    V::Order { .. }
                    | V::NotACurve { .. }
                    | V::Repeated { .. }
                    | V::Missed { .. }
                    | V::AtJoin { .. } => false,
                };
                // One meeting of several dropped; the lone one drops the pair.
                if pair.meetings.len() > 1 {
                    let mut twin = found.clone();
                    twin.pairs[p].meetings.remove(m);
                    assert!(
                        violations(&twin).contains(&V::Missed { first, second }),
                        "{name}: dropped meeting {m}"
                    );
                }
                // Its point moved by a tenth of a unit of the sketch: off a curve, or
                // (along a collinear one) off its parameter.
                let size = values
                    .values()
                    .fold(0.0_f64, |s, v| s.max(v.abs()))
                    .max(1.0);
                let mut twin = found.clone();
                twin.pairs[p].meetings[m].at[0] += 0.1 * size;
                assert!(
                    violations(&twin)
                        .iter()
                        .any(|v| matches!(v, V::OffCurve { .. } | V::Parameter { .. })),
                    "{name}: moved point"
                );
                // Its parameter on the first curve changed.
                let mut twin = found.clone();
                let t = &mut twin.pairs[p].meetings[m].on_first;
                *t = if *t > 0.5 { *t - 0.2 } else { *t + 0.2 };
                assert!(
                    violations(&twin)
                        .iter()
                        .any(|v| matches!(v, V::Parameter { .. }) && at_m(v)),
                    "{name}: changed parameter"
                );
                // Its kind changed.
                let mut twin = found.clone();
                let k = &mut twin.pairs[p].meetings[m].kind;
                *k = match *k {
                    MeetingKind::Crossing => MeetingKind::Touch,
                    MeetingKind::Touch | MeetingKind::Overlap { .. } => MeetingKind::Crossing,
                };
                assert!(
                    violations(&twin)
                        .iter()
                        .any(|v| matches!(v, V::Kind { .. }) && at_m(v)),
                    "{name}: changed kind {meeting:?}"
                );
                broken += 1;
            }
        }
    }
    assert!(broken >= 7, "{broken}");
}

#[test]
fn a_broken_profile_split_names_its_variant() {
    // Two bounded areas merged into one loop: the wall between them is
    // inside it and left out.
    let (sk, values) = load("c4", "shared-wall");
    let found = profiles(&sk, &values).unwrap();
    assert!(check_profiles(&sk, &values, &found).is_clean());
    assert_eq!(found.profiles.len(), 2);
    let wall = found.profiles[0]
        .outer
        .edges
        .iter()
        .find(|t| {
            found.profiles[1]
                .outer
                .edges
                .iter()
                .any(|u| u.entity == t.entity)
        })
        .unwrap()
        .entity;
    let rotated = |l: &Loop, wall_at: usize| {
        let i = l.edges.iter().position(|t| t.entity == wall).unwrap();
        let mut edges = l.edges.clone();
        // Rotate so the wall lands at `wall_at`.
        let n = edges.len();
        edges.rotate_left((i + n - wall_at) % n);
        edges
    };
    let a = rotated(&found.profiles[0].outer, 3);
    let b = rotated(&found.profiles[1].outer, 0);
    let area = found.profiles[0].outer.signed_area + found.profiles[1].outer.signed_area;
    let merged: Vec<Traversal> = a[..a.len() - 1].iter().chain(&b[1..]).copied().collect();
    let mut twin = Profiles {
        profiles: vec![],
        ..found.clone()
    };
    twin.profiles.push(laman::model::profile::Profile {
        outer: Loop {
            edges: merged,
            signed_area: area,
            joins: vec![],
        },
        holes: vec![],
        net_area: area,
    });
    let first = twin.profiles[0].outer.edges.first().map(|t| t.entity);
    let report = check_profiles(&sk, &values, &twin);
    assert!(
        report.violations.contains(&P::NotOneArea { first }),
        "{report:#?}"
    );

    // A bridge put in a loop: listed in the outer loop, not as a bridge.
    let (sk, values) = load("c4", "bridged-plate");
    let mut twin = profiles(&sk, &values).unwrap();
    let bridge = twin.bridges.pop().unwrap();
    twin.profiles[0].outer.edges.push(Traversal {
        entity: bridge,
        reversed: false,
    });
    let report = check_profiles(&sk, &values, &twin);
    assert!(
        report
            .violations
            .contains(&P::Misclassified { entity: bridge }),
        "{report:#?}"
    );

    // An edge dropped from the bridges and listed dangling instead.
    let (sk, values) = load("c4", "bridged-plate");
    let mut twin = profiles(&sk, &values).unwrap();
    let bridge = twin.bridges.pop().unwrap();
    twin.dangling.push(bridge);
    let report = check_profiles(&sk, &values, &twin);
    assert_eq!(report.violations, [P::Misclassified { entity: bridge }]);

    // A hole inside a sibling hole of another piece: the plate's island,
    // which lies in the plate's own hole, listed as a second hole.
    let (sk, values) = load("c3", "plate-island");
    let mut twin = profiles(&sk, &values).unwrap();
    let mut island = twin.profiles.pop().unwrap().outer;
    island.edges.reverse();
    for t in &mut island.edges {
        t.reversed = !t.reversed;
    }
    island.signed_area = -island.signed_area;
    let named: Option<EntityId> = island.edges.first().map(|t| t.entity);
    twin.profiles[0].holes.push(island);
    let report = check_profiles(&sk, &values, &twin);
    assert!(
        report.violations.contains(&P::Containment { first: named }),
        "{report:#?}"
    );
}
