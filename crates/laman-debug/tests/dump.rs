//! The dumps: the text dump is snapshot-tested on a small sketch that has
//! one of everything a line can say; the SVG dump is checked for its view
//! box, its arc, its dashes and its reds, never as a picture.
#![allow(clippy::unwrap_used, clippy::expect_used)] // tests: a failing unwrap is the test failing (clippy.toml)

use laman::model::{Coordinate, Sketch};
use laman_check::check;
use laman_debug::dump::{svg, text};

/// A ground origin, a point `q` at (3, 4) with a distance of 5 that holds
/// and one of 6 that does not, a tagged line, a construction circle, and
/// the quarter arc from (1, 0) to (0, 1) about the origin.
fn sketch() -> Sketch {
    let mut sk = Sketch::new();
    let o = sk.point_fixed(0.0, 0.0).unwrap();
    let q = sk.point(3.0, 4.0).unwrap();
    let line = sk.line(o, q).unwrap();
    sk.tag(line.id(), "ray").unwrap();
    let circle = sk.circle(o, 2.5).unwrap();
    sk.set_construction(circle.id(), true).unwrap();
    let s = sk.point(1.0, 0.0).unwrap();
    let e = sk.point(0.0, 1.0).unwrap();
    sk.arc(o, s, e).unwrap();
    let ok = sk.distance(o, q, 5.0).unwrap();
    sk.tag_constraint(ok, "reach").unwrap();
    sk.distance(o, q, 6.0).unwrap();
    sk
}

#[test]
fn text_dump_is_a_snapshot() {
    let sk = sketch();
    let values = sk.coordinates();
    let expected = "\
entities: 7
  #0 point (0, 0) fixed=x,y
  #1 point (3, 4)
  #2 line #0 -> #1 tag=\"ray\"
  #3 circle c=#0 r=2.5 construction
  #4 point (1, 0)
  #5 point (0, 1)
  #6 arc c=#0 s=#4 e=#5
constraints: 2
  c0 distance #0 #1 value=5 tag=\"reach\"
  c1 distance #0 #1 value=6
report:
  violated c1 Constraint(Distance) measured=5 wanted=6
";
    assert_eq!(text(&sk, &values, &check(&sk, &values)), expected);
}

#[test]
fn text_dump_shows_a_missing_parameter() {
    let sk = sketch();
    let mut values = sk.coordinates();
    let q = sk.entities().nth(1).unwrap().0;
    values.remove(&(q, Coordinate::X));
    let out = text(&sk, &values, &check(&sk, &values));
    assert!(out.contains("#1 point (?, 4)"), "{out}");
    assert!(out.contains("missing #1 X"), "{out}");
}

#[test]
fn svg_view_box_is_the_bounds_plus_margin() {
    let sk = sketch();
    let values = sk.coordinates();
    let out = svg(&sk, &values, &check(&sk, &values));
    // x spans -2.5 (the circle) to 3, y spans -2.5 to 4: 5.5 by 6.5, and a
    // tenth of the larger side (0.65) on every side.
    assert!(out.contains("viewBox=\"0 0 6.8000 7.8000\""), "{out}");
}

#[test]
fn svg_draws_the_arc_counter_clockwise_and_dashes_construction() {
    let sk = sketch();
    let values = sk.coordinates();
    let out = svg(&sk, &values, &check(&sk, &values));
    // Origin maps to (3.15, 4.65); the arc starts at (1, 0) and ends at
    // (0, 1), a quarter turn: small arc, sweep flag 0 (y is flipped).
    assert!(
        out.contains("M 4.1500 4.6500 A 1.0000 1.0000 0 0 0 3.1500 3.6500"),
        "{out}"
    );
    let circle = out
        .lines()
        .find(|l| l.contains("r=\"2.5000\""))
        .expect("the circle");
    assert!(circle.contains("stroke-dasharray"), "{circle}");
}

#[test]
fn svg_marks_only_the_violation_red() {
    let sk = sketch();
    let values = sk.coordinates();
    let out = svg(&sk, &values, &check(&sk, &values));
    assert!(out.contains("fill=\"red\">c1</text>"), "{out}");
    assert!(out.contains("fill=\"blue\">reach</text>"), "{out}");
    let clean = Sketch::new();
    let none = clean.coordinates();
    let empty = svg(&clean, &none, &check(&clean, &none));
    assert!(!empty.contains("red"), "{empty}");
}

#[test]
fn svg_escapes_tags() {
    let mut sk = Sketch::new();
    let p = sk.point(0.0, 0.0).unwrap();
    sk.tag(p.id(), "a<b&c").unwrap();
    let values = sk.coordinates();
    let out = svg(&sk, &values, &check(&sk, &values));
    assert!(out.contains("a&lt;b&amp;c"), "{out}");
}

#[test]
fn svg_arc_past_a_half_turn_sets_the_large_flag() {
    let mut sk = Sketch::new();
    let o = sk.point_fixed(0.0, 0.0).unwrap();
    // From (0, 1) counter-clockwise to (1, 0) is three quarters of a turn.
    let s = sk.point(0.0, 1.0).unwrap();
    let e = sk.point(1.0, 0.0).unwrap();
    sk.arc(o, s, e).unwrap();
    let values = sk.coordinates();
    let out = svg(&sk, &values, &check(&sk, &values));
    assert!(out.contains("A 1.0000 1.0000 0 1 0"), "{out}");
}
