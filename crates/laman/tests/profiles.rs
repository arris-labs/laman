#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! `profiles` on sketches built by construction: orientation, nesting,
//! closed-form areas, reports and refusals.
use laman::model::profile::{Profiles, Unjoined};
use laman::model::{Coordinate, EntityId, Point, Sketch};
use laman::profile::{ProfileError, profiles};
use std::f64::consts::PI;

/// A rectangle `w` by `h` at `(x, y)`; `clockwise` draws it the other way.
/// Returns its edge ids.
fn rect(sk: &mut Sketch, x: f64, y: f64, w: f64, h: f64, clockwise: bool) -> Vec<EntityId> {
    let mut c = [(x, y), (x + w, y), (x + w, y + h), (x, y + h)];
    if clockwise {
        c.reverse();
    }
    let p: Vec<Point> = c.iter().map(|&(a, b)| sk.point(a, b).unwrap()).collect();
    (0..4)
        .map(|i| sk.line(p[i], p[(i + 1) % 4]).unwrap().id())
        .collect()
}

fn run(sk: &Sketch) -> Profiles {
    profiles(sk, &sk.coordinates()).unwrap()
}

fn near(a: f64, b: f64) {
    assert!((a - b).abs() <= 1e-9 * b.abs().max(1.0), "{a} vs {b}");
}

#[test]
fn rectangle_is_counter_clockwise_whichever_way_it_is_drawn() {
    for clockwise in [false, true] {
        let mut sk = Sketch::new();
        let e = rect(&mut sk, 0.0, 0.0, 80.0, 90.0, clockwise);
        let r = run(&sk);
        assert_eq!(r.profiles.len(), 1);
        let p = &r.profiles[0];
        near(p.outer.signed_area, 7200.0);
        near(p.net_area, 7200.0);
        assert!(p.holes.is_empty());
        assert_eq!(p.outer.edges[0].entity, e[0]);
        // Drawn clockwise, the loop runs against its first edge.
        assert_eq!(p.outer.edges[0].reversed, clockwise);
        assert_eq!(p.outer.edges.len(), 4);
    }
}

#[test]
fn a_hole_is_clockwise_and_nets_out() {
    for clockwise in [false, true] {
        let mut sk = Sketch::new();
        rect(&mut sk, 0.0, 0.0, 80.0, 90.0, false);
        rect(&mut sk, 10.0, 10.0, 20.0, 20.0, clockwise);
        let r = run(&sk);
        assert_eq!(r.profiles.len(), 1);
        let p = &r.profiles[0];
        assert_eq!(p.holes.len(), 1);
        near(p.holes[0].signed_area, -400.0);
        near(p.net_area, 6800.0);
    }
}

#[test]
fn an_island_in_a_hole_is_its_own_profile() {
    let mut sk = Sketch::new();
    let c = sk.point(50.0, 50.0).unwrap();
    sk.circle(c, 40.0).unwrap();
    sk.circle(c, 20.0).unwrap();
    sk.circle(c, 10.0).unwrap();
    let r = run(&sk);
    assert_eq!(r.profiles.len(), 2);
    near(r.profiles[0].net_area, PI * (1600.0 - 400.0));
    near(r.profiles[0].holes[0].signed_area, -PI * 400.0);
    near(r.profiles[1].net_area, PI * 100.0);
    assert!(r.profiles[1].holes.is_empty());
    assert!(r.profiles[0].holes[0].edges[0].reversed);
}

#[test]
fn disjoint_loops_are_two_profiles_in_id_order() {
    let mut sk = Sketch::new();
    let a = rect(&mut sk, 0.0, 0.0, 1.0, 1.0, false);
    let b = rect(&mut sk, 5.0, 0.0, 2.0, 1.0, false);
    let r = run(&sk);
    assert_eq!(r.profiles.len(), 2);
    assert_eq!(r.profiles[0].outer.edges[0].entity, a[0]);
    assert_eq!(r.profiles[1].outer.edges[0].entity, b[0]);
}

#[test]
fn a_half_disc_has_its_closed_area() {
    let mut sk = Sketch::new();
    let c = sk.point(0.0, 0.0).unwrap();
    let s = sk.point(2.0, 0.0).unwrap();
    let e = sk.point(-2.0, 0.0).unwrap();
    sk.arc(c, s, e).unwrap();
    sk.line(e, s).unwrap();
    let r = run(&sk);
    near(r.profiles[0].net_area, PI * 2.0);
}

#[test]
fn reports_come_with_the_caller_ids() {
    let mut sk = Sketch::new();
    rect(&mut sk, 0.0, 0.0, 4.0, 4.0, false);
    let a = sk.point(10.0, 0.0).unwrap();
    let b = sk.point(11.0, 0.0).unwrap();
    let open = sk.line(a, b).unwrap().id();
    let r = run(&sk);
    assert_eq!(r.profiles.len(), 1);
    assert_eq!(r.open_chains, vec![vec![open]]);
}

#[test]
fn missing_and_non_finite_values_are_named() {
    let mut sk = Sketch::new();
    rect(&mut sk, 0.0, 0.0, 4.0, 4.0, false);
    let c = sk.point(1.0, 1.0).unwrap();
    let circle = sk.circle(c, 1.0).unwrap().id();
    let mut v = sk.coordinates();
    v.remove(&(c.id(), Coordinate::Y));
    assert_eq!(
        profiles(&sk, &v),
        Err(ProfileError::Missing { entity: c.id() })
    );
    let mut v = sk.coordinates();
    v.insert((circle, Coordinate::R), f64::NAN);
    assert_eq!(
        profiles(&sk, &v),
        Err(ProfileError::NonFinite { entity: circle })
    );
}

#[test]
fn a_corner_of_two_points_at_one_place_is_open_and_named() {
    let mut sk = Sketch::new();
    let a = sk.point(0.0, 0.0).unwrap();
    let b = sk.point(4.0, 0.0).unwrap();
    let c = sk.point(4.0, 3.0).unwrap();
    let d = sk.point(0.0, 3.0).unwrap();
    let d2 = sk.point(0.0, 3.0).unwrap(); // the same place as d
    sk.line(a, b).unwrap();
    sk.line(b, c).unwrap();
    sk.line(c, d).unwrap();
    sk.line(d2, a).unwrap();
    let r = run(&sk);
    assert!(r.profiles.is_empty());
    assert_eq!(r.open_chains.len(), 1);
    assert_eq!(
        r.unjoined,
        vec![Unjoined {
            first: d.id(),
            second: d2.id()
        }]
    );

    // Joining them closes the loop and the report goes.
    sk.coincident(d, d2).unwrap();
    let r = run(&sk);
    assert_eq!(r.profiles.len(), 1);
    assert!(r.unjoined.is_empty());
}

#[test]
fn free_ends_far_apart_are_not_reported() {
    let mut sk = Sketch::new();
    let p: Vec<_> = (0..4)
        .map(|i| sk.point(f64::from(i) * 10.0, 0.0).unwrap())
        .collect();
    sk.line(p[0], p[1]).unwrap();
    sk.line(p[2], p[3]).unwrap();
    assert!(run(&sk).unjoined.is_empty());
}
