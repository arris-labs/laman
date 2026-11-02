#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use super::*;
use serde_json::Value;
use std::path::PathBuf;

/// Agreement of an observed quantity with its closed form: a few ulps of a
/// unit cosine or of `atan(4/3)`, independent of any solver threshold.
const QUANTITY_ERROR: f64 = 1e-14;

const SLUGS: [&str; 5] = [
    "arc-through-start",
    "antiparallel-lines",
    "angle-supplement",
    "circle-through-zero",
    "semicircle-at-rounding",
];

struct Case {
    sketch: Sketch,
    before: Coordinates,
    after: Coordinates,
    flips: Vec<Flip>,
}

fn load(slug: &str) -> Case {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/branch")
        .join(slug);
    let sketch: Sketch =
        serde_json::from_str(&std::fs::read_to_string(dir.join("sketch.json")).unwrap()).unwrap();
    let expected: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("expected.json")).unwrap()).unwrap();
    let before = sketch.coordinates();
    let mut after = before.clone();
    for c in expected["coordinates"].as_array().unwrap() {
        let id = EntityId(c["id"].as_u64().unwrap() as u32);
        let coordinate = match c["coordinate"].as_str().unwrap() {
            "x" => Coordinate::X,
            "y" => Coordinate::Y,
            "r" => Coordinate::R,
            other => panic!("{slug}: coordinate {other}"),
        };
        *after.get_mut(&(id, coordinate)).unwrap() = c["value"].as_f64().unwrap();
    }
    let flips = expected["flips"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            let subject = match (f.get("entity"), f.get("constraint")) {
                (Some(id), None) => Subject::Entity(EntityId(id.as_u64().unwrap() as u32)),
                (None, Some(id)) => Subject::Constraint(ConstraintId(id.as_u64().unwrap() as u32)),
                _ => panic!("{slug}: flip subject"),
            };
            Flip {
                subject,
                was: f["was"].as_f64().unwrap(),
                now: f["now"].as_f64().unwrap(),
            }
        })
        .collect();
    Case {
        sketch,
        before,
        after,
        flips,
    }
}

fn scaled(values: &Coordinates, k: f64) -> Coordinates {
    values.iter().map(|(key, v)| (*key, v * k)).collect()
}

fn assert_flips(slug: &str, got: &[Flip], want: &[Flip], k: f64) {
    assert_eq!(got.len(), want.len(), "{slug} ×{k:e}: {got:?}");
    for (g, w) in got.iter().zip(want) {
        assert_eq!(g.subject, w.subject, "{slug} ×{k:e}");
        // A radius scales with the sketch; a sweep and a cosine do not.
        let unit = if matches!(slug, "circle-through-zero") {
            k
        } else {
            1.0
        };
        for (a, b) in [(g.was, w.was), (g.now, w.now)] {
            assert!(
                (a / unit - b).abs() <= QUANTITY_ERROR * b.abs().max(1.0),
                "{slug} ×{k:e}: {g:?} against {w:?}"
            );
        }
    }
}

#[test]
fn fixtures_flip_by_construction() {
    for slug in SLUGS {
        let case = load(slug);
        let before = Observations::at(&case.sketch, &case.before);
        let after = before.next(&case.sketch, &case.after);
        assert_flips(slug, &before.flips(&after), &case.flips, 1.0);
        // Back again is the mirror flip.
        let back = after.next(&case.sketch, &case.before);
        let mirrored: Vec<_> = case
            .flips
            .iter()
            .map(|f| Flip {
                was: f.now,
                now: f.was,
                ..*f
            })
            .collect();
        assert_flips(slug, &after.flips(&back), &mirrored, 1.0);
        // Observing the same coordinates again never flips.
        assert!(
            after
                .flips(&after.next(&case.sketch, &case.after))
                .is_empty()
        );
    }
}

#[test]
fn fixtures_at_f64_extremes() {
    for slug in SLUGS {
        let case = load(slug);
        let k = 1e300;
        let before = Observations::at(&case.sketch, &scaled(&case.before, k));
        let after = before.next(&case.sketch, &scaled(&case.after, k));
        assert_flips(slug, &before.flips(&after), &case.flips, k);
        // Below the length floor (ADR-0003) no entity is resolved, so no
        // branch is known and nothing is compared.
        let k = 1e-300;
        let before = Observations::at(&case.sketch, &scaled(&case.before, k));
        let after = before.next(&case.sketch, &scaled(&case.after, k));
        assert!(before.flips(&after).is_empty(), "{slug} ×{k:e}");
        assert!(after.0.values().all(|o| o.known.is_none()), "{slug}");
    }
}

#[test]
fn semicircle_cross_sign_changes_but_sweep_does_not_flip() {
    let case = load("semicircle-at-rounding");
    let cross = |v: &Coordinates| {
        let at = |id: u32, c| v[&(EntityId(id), c)];
        let (cx, cy) = (at(0, Coordinate::X), at(0, Coordinate::Y));
        (at(1, Coordinate::X) - cx) * (at(2, Coordinate::Y) - cy)
            - (at(1, Coordinate::Y) - cy) * (at(2, Coordinate::X) - cx)
    };
    // The fixture is what it claims: the cross product's sign is rounding.
    assert!(cross(&case.before) > 0.0 && cross(&case.after) < 0.0);
    let before = Observations::at(&case.sketch, &case.before);
    let after = before.next(&case.sketch, &case.after);
    let sweep = after.get(Subject::Entity(EntityId(3))).unwrap();
    assert!((sweep.value - PI).abs() < QUANTITY_ERROR);
    assert_eq!(sweep.known.map(|k| k.branch), Some(0));
}

#[test]
fn arc_past_a_full_turn_flips() {
    // Nearly a full turn, counter-clockwise from (10, 0) to (8, −6); `e`
    // carried on past `s` to (8, 6): the sweep tracks past 2π.
    let mut sk = Sketch::new();
    let c = sk.point_fixed(0.0, 0.0).unwrap();
    let s = sk.point(10.0, 0.0).unwrap();
    let e = sk.point(8.0, -6.0).unwrap();
    let arc = sk.arc(c, s, e).unwrap();
    let before = Observations::at(&sk, &sk.coordinates());
    let mut moved = sk.coordinates();
    moved.insert((e.id(), Coordinate::Y), 6.0);
    let after = before.next(&sk, &moved);
    let flips = before.flips(&after);
    let short = libm::atan2(6.0, 8.0);
    assert_eq!(flips.len(), 1, "{flips:?}");
    assert_eq!(flips[0].subject, Subject::Entity(arc.id()));
    assert!((flips[0].was - (TAU - short)).abs() < QUANTITY_ERROR);
    assert!((flips[0].now - (TAU + short)).abs() < QUANTITY_ERROR);
}

/// Two parallel lines whose second end sits at `x`, the first line along +x.
fn parallel_pair(x: f64) -> (Sketch, EntityId, ConstraintId) {
    let mut sk = Sketch::new();
    let a = sk.point(0.0, 0.0).unwrap();
    let b = sk.point(10.0, 0.0).unwrap();
    let l1 = sk.line(a, b).unwrap();
    let c = sk.point(0.0, 5.0).unwrap();
    let d = sk.point(x, 5.0).unwrap();
    let l2 = sk.line(c, d).unwrap();
    let id = sk.parallel(l1, l2).unwrap();
    (sk, d.id(), id)
}

#[test]
fn a_crossing_through_the_margin_is_still_a_flip() {
    // +x, then a line shorter than its degeneracy length (unknown), then −x:
    // the middle frame carries the first branch, so the third flips.
    let (sk, d, id) = parallel_pair(10.0);
    let first = Observations::at(&sk, &sk.coordinates());
    let mut middle = sk.coordinates();
    middle.insert((d, Coordinate::X), 1e-13);
    let second = first.next(&sk, &middle);
    assert!(first.flips(&second).is_empty());
    let carried = second.get(Subject::Constraint(id)).unwrap();
    assert_eq!(carried.value, 1.0);
    assert_eq!(carried.known.map(|k| k.branch), Some(1));
    let mut last = sk.coordinates();
    last.insert((d, Coordinate::X), -10.0);
    let third = second.next(&sk, &last);
    assert_eq!(
        second.flips(&third),
        [Flip {
            subject: Subject::Constraint(id),
            was: 1.0,
            now: -1.0
        }]
    );
}

#[test]
fn a_branch_first_known_is_adopted_without_a_flip() {
    // Drawn with a line inside the margin: unknown, so the first known
    // orientation after it is the sketch's branch, not a flip.
    let (sk, d, id) = parallel_pair(1e-13);
    let first = Observations::at(&sk, &sk.coordinates());
    assert_eq!(first.get(Subject::Constraint(id)).unwrap().known, None);
    let mut moved = sk.coordinates();
    moved.insert((d, Coordinate::X), -10.0);
    let second = first.next(&sk, &moved);
    assert!(first.flips(&second).is_empty());
    assert_eq!(
        second
            .get(Subject::Constraint(id))
            .unwrap()
            .known
            .map(|k| k.branch),
        Some(-1)
    );
}

#[test]
fn perpendicular_orientation_is_observed() {
    let mut sk = Sketch::new();
    let a = sk.point(0.0, 0.0).unwrap();
    let b = sk.point(10.0, 0.0).unwrap();
    let c = sk.point(0.0, 10.0).unwrap();
    let l1 = sk.line(a, b).unwrap();
    let l2 = sk.line(a, c).unwrap();
    let id = sk.perpendicular(l1, l2).unwrap();
    let before = Observations::at(&sk, &sk.coordinates());
    let mut moved = sk.coordinates();
    moved.insert((c.id(), Coordinate::Y), -10.0);
    let flips = before.flips(&before.next(&sk, &moved));
    assert_eq!(
        flips,
        [Flip {
            subject: Subject::Constraint(id),
            was: 1.0,
            now: -1.0
        }]
    );
}

#[test]
fn non_finite_coordinates_are_unknown_never_a_flip() {
    for case in SLUGS.map(load) {
        let before = Observations::at(&case.sketch, &case.before);
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let broken: Coordinates = case.after.keys().map(|k| (*k, bad)).collect();
            let after = before.next(&case.sketch, &broken);
            assert!(before.flips(&after).is_empty());
            // Every subject carries the branch it had.
            for (subject, o) in &after.0 {
                assert_eq!(o.known, before.0[subject].known);
            }
        }
    }
}
