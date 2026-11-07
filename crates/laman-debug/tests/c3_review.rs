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
