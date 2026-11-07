//! Cases the review of plans/m9-profiles found, through the public API
//! only: every loop is accounted for, and joins and free ends are named by
//! the end points of the edges.
#![allow(clippy::unwrap_used, clippy::expect_used)] // tests: a failing unwrap is the test failing (clippy.toml)

use laman::model::{Point, Sketch};
use laman::profile::profiles;

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
