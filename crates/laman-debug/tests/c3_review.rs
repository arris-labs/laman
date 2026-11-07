//! Cases the review of plans/m9-profiles found, through the public API
//! only: every loop is accounted for, and joins and free ends are named by
//! the end points of the edges.
#![allow(clippy::unwrap_used, clippy::expect_used)] // tests: a failing unwrap is the test failing (clippy.toml)

use laman::model::tolerance::Tolerances;
use laman::model::{Coordinate, Point, Sketch};
use laman::profile::{ProfileError, profiles};

/// A closed rectangle over `[x0, x1] x [y0, y1]`, counter-clockwise, and its
/// corner points.
fn rectangle(sk: &mut Sketch, x0: f64, y0: f64, x1: f64, y1: f64) -> [Point; 4] {
    let p = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)].map(|(x, y)| sk.point(x, y).unwrap());
    for i in 0..4 {
        sk.line(p[i], p[(i + 1) % 4]).unwrap();
    }
    p
}

/// Two bars crossing inside a plate and a speck at their crossing:
/// the bars' curves cross without a shared point (unspecified nesting), and
/// the speck lies in all three, so its container count is odd while its
/// innermost container is itself a hole. No loop may vanish.
#[test]
fn a_loop_whose_innermost_container_is_a_hole_is_still_reported() {
    let mut sk = Sketch::new();
    rectangle(&mut sk, 0.0, 0.0, 100.0, 100.0); // A
    rectangle(&mut sk, 10.0, 40.0, 60.0, 60.0); // B, a wide bar
    rectangle(&mut sk, 40.0, 10.0, 65.0, 70.0); // C, a tall bar crossing B
    rectangle(&mut sk, 45.0, 45.0, 55.0, 55.0); // the speck, in A, B and C
    let found = profiles(&sk, &sk.coordinates()).unwrap();
    let loops: usize = found.profiles.iter().map(|p| 1 + p.holes.len()).sum();
    assert_eq!(loops, 4, "{found:#?}");
    // Every outer loop turns counter-clockwise, every hole clockwise.
    for p in &found.profiles {
        assert!(p.outer.signed_area > 0.0);
        assert!(p.holes.iter().all(|h| h.signed_area < 0.0));
    }
    assert_eq!(found, profiles(&sk, &sk.coordinates()).unwrap());
}

/// A four-sided open chain a-b-c-d-e around a square of side 10 whose free
/// ends `a` and `e` are both at the origin, `e` made of its own point.
fn open_square(sk: &mut Sketch) -> (Point, Point) {
    let a = sk.point(0.0, 0.0).unwrap();
    let b = sk.point(10.0, 0.0).unwrap();
    let c = sk.point(10.0, 10.0).unwrap();
    let d = sk.point(0.0, 10.0).unwrap();
    let e = sk.point(0.0, 0.0).unwrap();
    sk.line(a, b).unwrap();
    sk.line(b, c).unwrap();
    sk.line(c, d).unwrap();
    sk.line(d, e).unwrap();
    (a, e)
}

/// D4 names both end points of the edges, not the joins' smallest ids: a
/// construction point with a smaller id made coincident with an end must not
/// be reported in its place.
#[test]
fn unjoined_names_the_edges_end_points() {
    let mut sk = Sketch::new();
    let k = sk.point(0.0, 0.0).unwrap(); // smaller than every end
    let (a, e) = open_square(&mut sk);
    sk.coincident(k, e).unwrap();
    let found = profiles(&sk, &sk.coordinates()).unwrap();
    assert!(found.profiles.is_empty());
    assert_eq!(found.unjoined.len(), 1, "{found:#?}");
    assert_eq!(
        (found.unjoined[0].first, found.unjoined[0].second),
        (a.id(), e.id())
    );
}

/// `Loop::joins` names the `coincident`s that join two or more distinct end
/// points of the loop, not one that only ties a corner to a point no edge of
/// the loop uses.
#[test]
fn loop_joins_exclude_coincidents_to_points_off_the_loop() {
    let mut sk = Sketch::new();
    let (a, e) = open_square(&mut sk);
    let q = sk.point(0.0, 0.0).unwrap(); // on no edge
    let join = sk.coincident(a, e).unwrap();
    let off = sk.coincident(a, q).unwrap();
    let found = profiles(&sk, &sk.coordinates()).unwrap();
    assert_eq!(found.profiles.len(), 1);
    let joins = &found.profiles[0].outer.joins;
    assert!(joins.contains(&join), "{joins:?}");
    assert!(!joins.contains(&off), "{joins:?}");
}

/// A chain through a point no edge uses is a join: both links are named.
#[test]
fn loop_joins_name_a_chain_through_an_off_loop_point() {
    let mut sk = Sketch::new();
    let (a, e) = open_square(&mut sk);
    let q = sk.point(0.0, 0.0).unwrap();
    let first = sk.coincident(a, q).unwrap();
    let second = sk.coincident(q, e).unwrap();
    let found = profiles(&sk, &sk.coordinates()).unwrap();
    assert_eq!(found.profiles.len(), 1);
    assert_eq!(found.profiles[0].outer.joins, vec![first, second]);
}

/// D1/Q3 A: a `coincident` the values violate still joins its ends, so the
/// loop closes and names it; the free ends it joins are not also reported.
#[test]
fn a_violated_joining_coincident_still_closes_its_loop_and_is_named() {
    let mut sk = Sketch::new();
    let (a, e) = open_square(&mut sk);
    let join = sk.coincident(a, e).unwrap();
    let mut values = sk.coordinates();
    values.insert((e.id(), Coordinate::X), 3.0);
    values.insert((e.id(), Coordinate::Y), 4.0);
    let found = profiles(&sk, &values).unwrap();
    assert_eq!(found.profiles.len(), 1, "{found:#?}");
    assert_eq!(found.profiles[0].outer.joins, vec![join]);
    assert!(found.open_chains.is_empty() && found.unjoined.is_empty());
}

/// An edge whose two ends are one join is reported as `degenerate` and left
/// out of every other list.
#[test]
fn a_self_joined_edge_is_reported_degenerate() {
    let mut sk = Sketch::new();
    let a = sk.point(0.0, 0.0).unwrap();
    let b = sk.point(5.0, 0.0).unwrap();
    let line = sk.line(a, b).unwrap();
    sk.coincident(a, b).unwrap();
    let found = profiles(&sk, &sk.coordinates()).unwrap();
    assert_eq!(found.degenerate, vec![line.id()]);
    assert!(found.profiles.is_empty() && found.open_chains.is_empty());
    assert!(found.dangling.is_empty() && found.unjoined.is_empty());
}

/// D4: two free ends are reported when they lie within the smaller of their
/// edges' length tolerances, not the larger.
#[test]
fn unjoined_ends_are_held_to_the_smaller_tolerance() {
    let build = |gap: f64| {
        let mut sk = Sketch::new();
        let a = sk.point(-1.0, 0.0).unwrap();
        let b = sk.point(0.0, 0.0).unwrap();
        let c = sk.point(0.0, gap).unwrap();
        let d = sk.point(1e6, 0.0).unwrap();
        let (short, long) = (sk.line(a, b).unwrap(), sk.line(c, d).unwrap());
        let t = Tolerances::at(&sk, &sk.coordinates());
        // The premise: a short edge near the origin is held far tighter than
        // a long one.
        assert!(t.entity(short.id()) * 100.0 < t.entity(long.id()));
        let ends = (b.id(), c.id());
        (profiles(&sk, &sk.coordinates()).unwrap(), t, short, ends)
    };
    let (_, t, short, _) = build(1e-7);
    let tight = t.entity(short.id());
    let (inside, ..) = build(tight / 2.0);
    let (outside, ..) = build(tight * 4.0);
    let (_, _, _, ends) = build(1e-7);
    assert_eq!(inside.unjoined.len(), 1, "{inside:#?}");
    assert_eq!((inside.unjoined[0].first, inside.unjoined[0].second), ends);
    assert!(outside.unjoined.is_empty(), "{outside:#?}");
}

/// A non-finite coordinate of a point an edge uses is refused by id.
#[test]
fn a_non_finite_point_coordinate_is_refused_by_id() {
    let mut sk = Sketch::new();
    let (a, _) = open_square(&mut sk);
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut values = sk.coordinates();
        values.insert((a.id(), Coordinate::Y), bad);
        assert_eq!(
            profiles(&sk, &values),
            Err(ProfileError::NonFinite { entity: a.id() })
        );
    }
}
