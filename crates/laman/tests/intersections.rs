#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! `intersections` on sketches built by construction: the join exclusion,
//! the pairs looked at, order, refusals and determinism (ADR-0016).
use laman::intersect::{IntersectError, intersections};
use laman::model::intersection::MeetingKind;
use laman::model::{Coordinate, Sketch};

#[test]
fn a_shared_point_is_a_join_not_an_intersection() {
    let mut sk = Sketch::new();
    let p = [(0.0, 0.0), (4.0, 0.0), (4.0, 3.0)].map(|(x, y)| sk.point(x, y).unwrap());
    sk.line(p[0], p[1]).unwrap();
    sk.line(p[1], p[2]).unwrap();
    assert!(
        intersections(&sk, &sk.coordinates())
            .unwrap()
            .pairs
            .is_empty()
    );
}

#[test]
fn a_coincident_join_is_a_join_but_an_unjoined_corner_is_a_crossing() {
    for joined in [true, false] {
        let mut sk = Sketch::new();
        let a = sk.point(0.0, 0.0).unwrap();
        let b = sk.point(4.0, 0.0).unwrap();
        let c = sk.point(4.0, 0.0).unwrap();
        let d = sk.point(4.0, 3.0).unwrap();
        sk.line(a, b).unwrap();
        sk.line(c, d).unwrap();
        if joined {
            sk.coincident(b, c).unwrap();
        }
        let found = intersections(&sk, &sk.coordinates()).unwrap();
        if joined {
            assert!(found.pairs.is_empty());
        } else {
            let m = found.pairs[0].meetings[0];
            assert_eq!(m.kind, MeetingKind::Crossing);
            assert_eq!((m.at, m.on_first, m.on_second), ([4.0, 0.0], 1.0, 0.0));
        }
    }
}

#[test]
fn a_tangent_line_joined_to_an_arc_end_does_not_meet_it() {
    // A slot's join: the line ends where the arc starts, tangent there.
    let mut sk = Sketch::new();
    let c = sk.point(0.0, 0.0).unwrap();
    let s = sk.point(5.0, 0.0).unwrap();
    let e = sk.point(0.0, 5.0).unwrap();
    let a = sk.point(5.0, -4.0).unwrap();
    let b = sk.point(5.0, 0.0).unwrap();
    sk.arc(c, s, e).unwrap();
    sk.line(a, b).unwrap();
    sk.coincident(b, s).unwrap();
    assert!(
        intersections(&sk, &sk.coordinates())
            .unwrap()
            .pairs
            .is_empty()
    );
}

#[test]
fn a_joined_line_that_crosses_the_arc_again_is_reported_there_only() {
    // The chord of the circle through the arc's start, ending beyond it.
    let mut sk = Sketch::new();
    let c = sk.point(0.0, 0.0).unwrap();
    let s = sk.point(5.0, 0.0).unwrap();
    let e = sk.point(-5.0, 0.0).unwrap();
    let a = sk.point(5.0, 0.0).unwrap();
    let b = sk.point(-3.0, 4.0).unwrap();
    let arc = sk.arc(c, s, e).unwrap();
    let line = sk.line(a, b).unwrap();
    sk.coincident(a, s).unwrap();
    let found = intersections(&sk, &sk.coordinates()).unwrap();
    assert_eq!(found.pairs.len(), 1);
    let p = &found.pairs[0];
    assert_eq!((p.first, p.second), (arc.id(), line.id()));
    assert_eq!(p.meetings.len(), 1);
    // Their one other meeting is the line's end, on the arc's circle.
    assert_eq!(p.meetings[0].at, [-3.0, 4.0]);
    assert_eq!(p.meetings[0].on_second, 1.0);
}

#[test]
fn construction_curves_meet_nothing() {
    let mut sk = Sketch::new();
    let p = [(0.0, 0.0), (4.0, 4.0), (0.0, 4.0), (4.0, 0.0)].map(|(x, y)| sk.point(x, y).unwrap());
    sk.line(p[0], p[1]).unwrap();
    let l2 = sk.line(p[2], p[3]).unwrap();
    sk.set_construction(l2.id(), true).unwrap();
    assert!(
        intersections(&sk, &sk.coordinates())
            .unwrap()
            .pairs
            .is_empty()
    );
}

#[test]
fn along_orders_by_parameter_from_either_side() {
    // A line through two circles' centres' height: crossings in order.
    let mut sk = Sketch::new();
    let a = sk.point(-10.0, 0.0).unwrap();
    let b = sk.point(10.0, 0.0).unwrap();
    let line = sk.line(a, b).unwrap();
    let far = sk.point(6.0, 0.0).unwrap();
    let near = sk.point(-6.0, 0.0).unwrap();
    // Added far first, so the order is the parameter's, not the id's.
    let c_far = sk.circle(far, 2.0).unwrap();
    let c_near = sk.circle(near, 2.0).unwrap();
    let found = intersections(&sk, &sk.coordinates()).unwrap();
    let on_line = found.along(line.id());
    let seen: Vec<_> = on_line.iter().map(|m| (m.other, m.meeting.at[0])).collect();
    assert_eq!(
        seen,
        [
            (c_near.id(), -8.0),
            (c_near.id(), -4.0),
            (c_far.id(), 4.0),
            (c_far.id(), 8.0)
        ]
    );
    // From a circle's side the line is the other curve, parameters turned.
    let from_far = found.along(c_far.id());
    assert_eq!(from_far.len(), 2);
    assert!(from_far.iter().all(|m| m.other == line.id()));
    assert!(from_far[0].meeting.on_first <= from_far[1].meeting.on_first);
}

#[test]
fn values_the_curves_need_are_named_when_missing_or_not_finite() {
    let mut sk = Sketch::new();
    let a = sk.point(0.0, 0.0).unwrap();
    let b = sk.point(1.0, 0.0).unwrap();
    sk.line(a, b).unwrap();
    let mut values = sk.coordinates();
    values.remove(&(b.id(), Coordinate::Y));
    assert_eq!(
        intersections(&sk, &values),
        Err(IntersectError::Missing { entity: b.id() })
    );
    let mut values = sk.coordinates();
    values.insert((a.id(), Coordinate::X), f64::NAN);
    assert_eq!(
        intersections(&sk, &values),
        Err(IntersectError::NonFinite { entity: a.id() })
    );
}

#[test]
fn the_result_is_the_same_on_a_second_call() {
    let mut sk = Sketch::new();
    let p = [(0.0, 0.0), (4.0, 4.0), (0.0, 4.0), (4.0, 0.0)].map(|(x, y)| sk.point(x, y).unwrap());
    sk.line(p[0], p[1]).unwrap();
    sk.line(p[2], p[3]).unwrap();
    let c = sk.point(2.0, 2.0).unwrap();
    sk.circle(c, 1.0).unwrap();
    let values = sk.coordinates();
    assert_eq!(intersections(&sk, &values), intersections(&sk, &values));
}
