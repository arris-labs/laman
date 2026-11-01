//! Every constraint, the arc's intrinsic equation and the degenerate
//! entities, each on a configuration that satisfies it and one that does
//! not. Configurations are hand-built: the sketch is drawn where the
//! constraint holds (so the builder takes its sense from the drawing), then
//! one parameter is moved.
#![allow(clippy::unwrap_used, clippy::panic)] // tests: a failing unwrap or panic is the test failing (clippy.toml)

use std::f64::consts::PI;

use laman::model::{
    Arc, Circle, ConstraintId, ConstraintKind, Coordinate, Coordinates, EntityId, EntityKind, Line,
    Point, Sketch,
};
use laman_check::{Degeneracy, Missing, Report, Source, Subject, check};

fn pt(sk: &mut Sketch, x: f64, y: f64) -> Point {
    sk.point(x, y).unwrap()
}

fn seg(sk: &mut Sketch, a: (f64, f64), b: (f64, f64)) -> Line {
    let (a, b) = (pt(sk, a.0, a.1), pt(sk, b.0, b.1));
    sk.line(a, b).unwrap()
}

fn circle(sk: &mut Sketch, c: (f64, f64), r: f64) -> Circle {
    let c = pt(sk, c.0, c.1);
    sk.circle(c, r).unwrap()
}

fn arc(sk: &mut Sketch, c: (f64, f64), s: (f64, f64), e: (f64, f64)) -> Arc {
    let (c, s, e) = (pt(sk, c.0, c.1), pt(sk, s.0, s.1), pt(sk, e.0, e.1));
    sk.arc(c, s, e).unwrap()
}

/// The line's start and end points.
fn ends(sk: &Sketch, l: Line) -> (Point, Point) {
    match sk.entity(l.id()) {
        Some(laman::model::Entity::Line { a, b }) => (*a, *b),
        other => panic!("not a line: {other:?}"),
    }
}

fn moved(sk: &Sketch, id: EntityId, coordinate: Coordinate, value: f64) -> Coordinates {
    let mut c = sk.coordinates();
    assert!(c.insert((id, coordinate), value).is_some());
    c
}

fn clean(sk: &Sketch) {
    assert_eq!(check(sk, &sk.coordinates()), Report::default());
}

/// The only violation is `id`'s, of `kind`.
fn only(report: &Report, id: ConstraintId, kind: ConstraintKind) -> (f64, f64) {
    assert_eq!(report.violations.len(), 1, "{report:?}");
    let v = &report.violations[0];
    assert_eq!(v.id, Source::Constraint(id));
    assert_eq!(v.kind, Subject::Constraint(kind));
    (v.measured, v.wanted)
}

#[test]
fn coincident() {
    let mut sk = Sketch::new();
    let (p, q) = (pt(&mut sk, 1.0, 2.0), pt(&mut sk, 1.0, 2.0));
    let id = sk.coincident(p, q).unwrap();
    clean(&sk);
    let r = check(&sk, &moved(&sk, q.id(), Coordinate::X, 1.5));
    assert_eq!(only(&r, id, ConstraintKind::Coincident), (0.5, 0.0));
}

#[test]
fn point_on_line_is_distance_to_the_infinite_line() {
    let mut sk = Sketch::new();
    let l = seg(&mut sk, (0.0, 0.0), (4.0, 0.0));
    let p = pt(&mut sk, 9.0, 0.0); // beyond the segment, still on its line
    let id = sk.point_on_line(p, l).unwrap();
    clean(&sk);
    let r = check(&sk, &moved(&sk, p.id(), Coordinate::Y, 0.5));
    assert_eq!(only(&r, id, ConstraintKind::PointOnLine), (0.5, 0.0));
}

#[test]
fn point_on_circle_and_on_an_arcs_whole_circle() {
    let mut sk = Sketch::new();
    let c = circle(&mut sk, (0.0, 0.0), 5.0);
    let p = pt(&mut sk, 3.0, 4.0);
    let id = sk.point_on_circle(p, c).unwrap();
    let a = arc(&mut sk, (0.0, 0.0), (5.0, 0.0), (0.0, 5.0));
    let q = pt(&mut sk, 0.0, -5.0); // outside the sweep, on the circle
    let on_arc = sk.point_on_circle(q, a).unwrap();
    clean(&sk);
    let r = check(&sk, &moved(&sk, p.id(), Coordinate::X, 3.5));
    let (measured, wanted) = only(&r, id, ConstraintKind::PointOnCircle);
    assert_eq!(wanted, 5.0);
    assert!((measured - 28.25f64.sqrt()).abs() < 1e-15, "{measured}");
    let r = check(&sk, &moved(&sk, q.id(), Coordinate::Y, -6.0));
    assert_eq!(only(&r, on_arc, ConstraintKind::PointOnCircle), (6.0, 5.0));
}

#[test]
fn horizontal_and_vertical_by_line_and_by_points() {
    let mut sk = Sketch::new();
    let h = seg(&mut sk, (0.0, 1.0), (4.0, 1.0));
    let hid = sk.horizontal(h).unwrap();
    let v = seg(&mut sk, (7.0, 0.0), (7.0, 3.0));
    let vid = sk.vertical(v).unwrap();
    let (p, q) = (pt(&mut sk, 0.0, 5.0), pt(&mut sk, 2.0, 5.0));
    let hp = sk.horizontal((p, q)).unwrap();
    let (r, s) = (pt(&mut sk, 5.0, 5.0), pt(&mut sk, 5.0, 8.0));
    let vp = sk.vertical((r, s)).unwrap();
    clean(&sk);

    let (_, hb) = ends(&sk, h);
    let rep = check(&sk, &moved(&sk, hb.id(), Coordinate::Y, 1.25));
    assert_eq!(only(&rep, hid, ConstraintKind::Horizontal), (0.25, 0.0));
    let (_, vb) = ends(&sk, v);
    let rep = check(&sk, &moved(&sk, vb.id(), Coordinate::X, 7.125));
    assert_eq!(only(&rep, vid, ConstraintKind::Vertical), (0.125, 0.0));
    let rep = check(&sk, &moved(&sk, q.id(), Coordinate::Y, 5.5));
    assert_eq!(only(&rep, hp, ConstraintKind::Horizontal), (0.5, 0.0));
    let rep = check(&sk, &moved(&sk, s.id(), Coordinate::X, 5.5));
    assert_eq!(only(&rep, vp, ConstraintKind::Vertical), (0.5, 0.0));
}

#[test]
fn parallel_accepts_both_orientations() {
    let mut sk = Sketch::new();
    let a = seg(&mut sk, (0.0, 0.0), (1.0, 1.0));
    let same = seg(&mut sk, (0.0, 1.0), (2.0, 3.0));
    let opposite = seg(&mut sk, (3.0, 3.0), (1.0, 1.0));
    let id = sk.parallel(a, same).unwrap();
    sk.parallel(a, opposite).unwrap();
    clean(&sk);
    let (_, end) = ends(&sk, same);
    let rep = check(&sk, &moved(&sk, end.id(), Coordinate::Y, 4.0));
    let (measured, wanted) = only(&rep, id, ConstraintKind::Parallel);
    assert_eq!(wanted, 0.0);
    // directions (1,1) and (2,3): atan2(1, 5)
    assert!(
        (measured - libm::atan2(1.0, 5.0)).abs() < 1e-15,
        "{measured}"
    );
}

#[test]
fn perpendicular() {
    let mut sk = Sketch::new();
    let a = seg(&mut sk, (0.0, 0.0), (1.0, 1.0));
    let b = seg(&mut sk, (5.0, 0.0), (4.0, 1.0));
    let id = sk.perpendicular(a, b).unwrap();
    clean(&sk);
    let (_, end) = ends(&sk, b);
    let rep = check(&sk, &moved(&sk, end.id(), Coordinate::X, 5.0));
    // directions (1,1) and (0,1): 45 degrees, not 90
    let (measured, wanted) = only(&rep, id, ConstraintKind::Perpendicular);
    assert_eq!(wanted, PI / 2.0);
    assert!((measured - PI / 4.0).abs() < 1e-15, "{measured}");
}

#[test]
fn equal_length_and_equal_radius() {
    let mut sk = Sketch::new();
    let a = seg(&mut sk, (0.0, 0.0), (3.0, 4.0));
    let b = seg(&mut sk, (1.0, 1.0), (6.0, 1.0));
    let len = sk.equal_length(a, b).unwrap();
    let c = circle(&mut sk, (0.0, 0.0), 2.0);
    let d = circle(&mut sk, (9.0, 0.0), 2.0);
    let arc = arc(&mut sk, (0.0, 9.0), (2.0, 9.0), (0.0, 11.0));
    let rad = sk.equal_radius(c, d).unwrap();
    sk.equal_radius(c, arc).unwrap();
    clean(&sk);
    let (_, end) = ends(&sk, b);
    let rep = check(&sk, &moved(&sk, end.id(), Coordinate::X, 7.0));
    assert_eq!(only(&rep, len, ConstraintKind::EqualLength), (5.0, 6.0));
    let rep = check(&sk, &moved(&sk, d.id(), Coordinate::R, 2.5));
    assert_eq!(only(&rep, rad, ConstraintKind::EqualRadius), (2.0, 2.5));
}

#[test]
fn tangent_line_circle_on_either_side() {
    let mut sk = Sketch::new();
    let l = seg(&mut sk, (0.0, 0.0), (10.0, 0.0));
    let above = circle(&mut sk, (5.0, 3.0), 3.0);
    let below = circle(&mut sk, (5.0, -2.0), 2.0);
    let up = sk.tangent_line_circle(l, above).unwrap();
    let down = sk.tangent_line_circle(l, below).unwrap();
    clean(&sk);
    // too big: touches no more
    let rep = check(&sk, &moved(&sk, above.id(), Coordinate::R, 3.5));
    assert_eq!(
        only(&rep, up, ConstraintKind::TangentLineCircle),
        (3.0, 3.5)
    );
    // the centre on the wrong side is not tangent, though the distance is
    let (_, centre) = match sk.entity(below.id()) {
        Some(laman::model::Entity::Circle { c, .. }) => ((), *c),
        other => panic!("{other:?}"),
    };
    let rep = check(&sk, &moved(&sk, centre.id(), Coordinate::Y, 2.0));
    assert_eq!(
        only(&rep, down, ConstraintKind::TangentLineCircle),
        (2.0, -2.0)
    );
}

#[test]
fn tangent_circles_external_and_internal() {
    let mut sk = Sketch::new();
    let a = circle(&mut sk, (0.0, 0.0), 2.0);
    let b = circle(&mut sk, (5.0, 0.0), 3.0);
    let ext = sk.tangent_circles(a, b).unwrap();
    let big = circle(&mut sk, (0.0, 20.0), 5.0);
    let small = circle(&mut sk, (2.0, 20.0), 3.0);
    let inner = sk.tangent_circles(big, small).unwrap();
    let swapped = sk.tangent_circles(small, big).unwrap();
    clean(&sk);
    let rep = check(&sk, &moved(&sk, b.id(), Coordinate::R, 3.5));
    assert_eq!(only(&rep, ext, ConstraintKind::TangentCircles), (5.0, 5.5));
    let rep = check(&sk, &moved(&sk, small.id(), Coordinate::R, 2.0));
    let ids: Vec<_> = rep.violations.iter().map(|v| v.id).collect();
    assert_eq!(
        ids,
        [Source::Constraint(inner), Source::Constraint(swapped)]
    );
    assert_eq!(
        (rep.violations[0].measured, rep.violations[0].wanted),
        (2.0, 3.0)
    );
    assert_eq!(
        (rep.violations[1].measured, rep.violations[1].wanted),
        (2.0, 3.0)
    );
}

#[test]
fn midpoint_and_symmetric() {
    let mut sk = Sketch::new();
    let l = seg(&mut sk, (0.0, 1.0), (4.0, 1.0));
    let m = pt(&mut sk, 2.0, 1.0);
    let mid = sk.midpoint(m, l).unwrap();
    let (p, q) = (pt(&mut sk, 1.0, 3.0), pt(&mut sk, 1.0, -1.0));
    let sym = sk.symmetric(p, q, l).unwrap();
    // both on the axis: a point is its own mirror image
    let (r, s) = (pt(&mut sk, 8.0, 1.0), pt(&mut sk, 8.0, 1.0));
    sk.symmetric(r, s, l).unwrap();
    clean(&sk);
    let rep = check(&sk, &moved(&sk, m.id(), Coordinate::X, 2.5));
    assert_eq!(only(&rep, mid, ConstraintKind::Midpoint), (0.5, 0.0));
    // q slides along the perpendicular bisector's parallel: image off by 1
    let rep = check(&sk, &moved(&sk, q.id(), Coordinate::X, 2.0));
    assert_eq!(only(&rep, sym, ConstraintKind::Symmetric), (1.0, 0.0));
}

#[test]
fn fix_distance_radius() {
    let mut sk = Sketch::new();
    let p = pt(&mut sk, 3.0, 4.0);
    let fix = sk.fix(p, 3.0, 4.0).unwrap();
    let q = pt(&mut sk, 0.0, 0.0);
    let dist = sk.distance(p, q, 5.0).unwrap();
    let c = circle(&mut sk, (9.0, 9.0), 2.0);
    let rad = sk.radius(c, 2.0).unwrap();
    let a = arc(&mut sk, (0.0, 9.0), (2.0, 9.0), (0.0, 11.0));
    let rad_arc = sk.radius(a, 2.0).unwrap();
    clean(&sk);
    let rep = check(&sk, &moved(&sk, p.id(), Coordinate::X, 6.0));
    let ids: Vec<_> = rep.violations.iter().map(|v| v.id).collect();
    assert_eq!(ids, [Source::Constraint(fix), Source::Constraint(dist)]);
    assert_eq!(
        (rep.violations[0].measured, rep.violations[0].wanted),
        (3.0, 0.0)
    );
    let rep = check(&sk, &moved(&sk, c.id(), Coordinate::R, 2.5));
    assert_eq!(only(&rep, rad, ConstraintKind::Radius), (2.5, 2.0));
    // an arc's radius is centre-to-start; moving the centre changes it
    let centre = match sk.entity(a.id()) {
        Some(laman::model::Entity::Arc { c, .. }) => *c,
        other => panic!("{other:?}"),
    };
    let rep = check(&sk, &moved(&sk, centre.id(), Coordinate::X, 1.0));
    assert!(
        rep.violations
            .iter()
            .any(|v| v.id == Source::Constraint(rad_arc) && v.measured == 1.0 && v.wanted == 2.0),
        "{rep:?}"
    );
}

#[test]
fn point_line_distance_keeps_its_side() {
    let mut sk = Sketch::new();
    let l = seg(&mut sk, (0.0, 0.0), (4.0, 0.0));
    let above = pt(&mut sk, 1.0, 2.0);
    let below = pt(&mut sk, 3.0, -1.5);
    let up = sk.point_line_distance(above, l, 2.0).unwrap();
    let down = sk.point_line_distance(below, l, 1.5).unwrap();
    clean(&sk);
    let rep = check(&sk, &moved(&sk, above.id(), Coordinate::Y, 2.5));
    assert_eq!(
        only(&rep, up, ConstraintKind::PointLineDistance),
        (2.5, 2.0)
    );
    // crossing the line: the distance is 1.5 again, on the wrong side
    let rep = check(&sk, &moved(&sk, below.id(), Coordinate::Y, 1.5));
    assert_eq!(
        only(&rep, down, ConstraintKind::PointLineDistance),
        (1.5, -1.5)
    );
}

#[test]
fn angle_is_taken_mod_a_half_turn() {
    let mut sk = Sketch::new();
    let a = seg(&mut sk, (0.0, 0.0), (1.0, 0.0));
    let b = seg(&mut sk, (5.0, 5.0), (6.0, 6.0));
    let flipped = seg(&mut sk, (9.0, 9.0), (8.0, 8.0));
    let id = sk.angle(a, b, PI / 4.0).unwrap();
    sk.angle(a, flipped, PI / 4.0).unwrap();
    clean(&sk);
    let (_, end) = ends(&sk, b);
    let rep = check(&sk, &moved(&sk, end.id(), Coordinate::Y, 7.0));
    let (measured, wanted) = only(&rep, id, ConstraintKind::Angle);
    assert_eq!(wanted, PI / 4.0);
    assert!(
        (measured - libm::atan2(2.0, 1.0)).abs() < 1e-15,
        "{measured}"
    );
}

#[test]
fn an_arcs_end_is_as_far_from_the_centre_as_its_start() {
    let mut sk = Sketch::new();
    let a = arc(&mut sk, (0.0, 0.0), (5.0, 0.0), (3.0, 4.0));
    clean(&sk);
    let end = match sk.entity(a.id()) {
        Some(laman::model::Entity::Arc { e, .. }) => *e,
        other => panic!("{other:?}"),
    };
    let rep = check(&sk, &moved(&sk, end.id(), Coordinate::Y, 5.0));
    assert_eq!(rep.violations.len(), 1);
    let v = &rep.violations[0];
    assert_eq!(v.id, Source::Arc(a.id()));
    assert_eq!(v.kind, Subject::ArcIntrinsic);
    assert_eq!(v.wanted, 5.0);
    assert!((v.measured - 34.0f64.sqrt()).abs() < 1e-15);
}

#[test]
fn a_violation_names_the_callers_tag() {
    let mut sk = Sketch::new();
    let (p, q) = (pt(&mut sk, 0.0, 0.0), pt(&mut sk, 3.0, 4.0));
    let d = sk.distance(p, q, 5.0).unwrap();
    sk.tag_constraint(d, "span").unwrap();
    let arc = arc(&mut sk, (0.0, 9.0), (2.0, 9.0), (0.0, 11.0));
    sk.tag(arc.id(), "fillet").unwrap();
    let mut c = moved(&sk, q.id(), Coordinate::X, 0.0);
    c.insert((arc_end(&sk, arc).id(), Coordinate::X), 1.0);
    let rep = check(&sk, &c);
    let tags: Vec<_> = rep.violations.iter().map(|v| v.tag.as_deref()).collect();
    assert_eq!(tags, [Some("span"), Some("fillet")]);
}

fn arc_end(sk: &Sketch, a: Arc) -> Point {
    match sk.entity(a.id()) {
        Some(laman::model::Entity::Arc { e, .. }) => *e,
        other => panic!("{other:?}"),
    }
}

#[test]
fn degenerate_entities_are_listed_though_no_constraint_is_off() {
    let mut sk = Sketch::new();
    let l = seg(&mut sk, (0.0, 0.0), (1.0, 0.0));
    let c = circle(&mut sk, (5.0, 5.0), 1.0);
    let a = arc(&mut sk, (9.0, 9.0), (10.0, 9.0), (9.0, 10.0));
    let b = arc(&mut sk, (20.0, 20.0), (21.0, 20.0), (20.0, 21.0));
    sk.tag(l.id(), "edge").unwrap();
    let (_, lb) = ends(&sk, l);
    let lx = lb.id();
    let mut v = sk.coordinates();
    v.insert((lx, Coordinate::X), 0.0); // line collapses
    v.insert((c.id(), Coordinate::R), 0.0);
    let (ae, be) = (arc_end(&sk, a), arc_end(&sk, b));
    // a: end lands on start (radius still right); b: start and end land on the centre
    v.insert((ae.id(), Coordinate::X), 10.0);
    v.insert((ae.id(), Coordinate::Y), 9.0);
    let bs = match sk.entity(b.id()) {
        Some(laman::model::Entity::Arc { s, c, .. }) => (*s, *c),
        other => panic!("{other:?}"),
    };
    v.insert((bs.0.id(), Coordinate::X), 20.0);
    v.insert((bs.0.id(), Coordinate::Y), 20.0);
    v.insert((be.id(), Coordinate::X), 20.0);
    v.insert((be.id(), Coordinate::Y), 20.0);
    let rep = check(&sk, &v);
    assert!(
        rep.violations.is_empty() && rep.missing.is_empty(),
        "{rep:?}"
    );
    let got: Vec<_> = rep
        .degenerate
        .iter()
        .map(|d| (d.id, d.kind, d.what, d.tag.as_deref()))
        .collect();
    assert_eq!(
        got,
        [
            (
                l.id(),
                EntityKind::Line,
                Degeneracy::ZeroLengthLine,
                Some("edge")
            ),
            (c.id(), EntityKind::Circle, Degeneracy::ZeroRadius, None),
            (a.id(), EntityKind::Arc, Degeneracy::EmptyArc, None),
            (b.id(), EntityKind::Arc, Degeneracy::ZeroRadiusArc, None),
            (b.id(), EntityKind::Arc, Degeneracy::EmptyArc, None),
        ]
    );
}

#[test]
fn a_missing_parameter_is_reported_not_guessed() {
    let mut sk = Sketch::new();
    let (p, q) = (pt(&mut sk, 0.0, 0.0), pt(&mut sk, 3.0, 4.0));
    sk.distance(p, q, 5.0).unwrap();
    let mut c = sk.coordinates();
    c.remove(&(q.id(), Coordinate::Y));
    let rep = check(&sk, &c);
    assert!(rep.violations.is_empty());
    assert_eq!(
        rep.missing,
        [Missing {
            id: q.id(),
            coordinate: Some(Coordinate::Y)
        }]
    );
    assert!(!rep.is_clean());
    assert!(!check(&sk, &Coordinates::new()).is_clean());
}

#[test]
fn a_non_finite_coordinate_is_a_violation_not_a_pass() {
    let mut sk = Sketch::new();
    let (p, q) = (pt(&mut sk, 0.0, 0.0), pt(&mut sk, 3.0, 4.0));
    let id = sk.distance(p, q, 5.0).unwrap();
    let rep = check(&sk, &moved(&sk, q.id(), Coordinate::X, f64::NAN));
    assert_eq!(rep.violations.len(), 1);
    assert_eq!(rep.violations[0].id, Source::Constraint(id));
    let rep = check(&sk, &moved(&sk, q.id(), Coordinate::X, f64::INFINITY));
    assert_eq!(rep.violations.len(), 1);
}

#[test]
fn a_zero_length_line_does_not_make_a_constraint_pass() {
    let mut sk = Sketch::new();
    let l = seg(&mut sk, (0.0, 0.0), (1.0, 0.0));
    let p = pt(&mut sk, 0.5, 0.0);
    let id = sk.point_on_line(p, l).unwrap();
    let (_, b) = ends(&sk, l);
    let rep = check(&sk, &moved(&sk, b.id(), Coordinate::X, 0.0));
    assert_eq!(rep.violations.len(), 1);
    assert_eq!(rep.violations[0].id, Source::Constraint(id));
    assert_eq!(rep.degenerate.len(), 1);
}
