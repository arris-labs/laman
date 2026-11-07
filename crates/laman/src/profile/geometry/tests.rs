#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! The loop kernel against closed forms: areas, orientation and winding
//! numbers of shapes built by construction, at scales 1e-6 to 1e6 (and
//! 1e±150), translated up to 1e4 times their size, and near 1e±300.
use super::*;
use std::f64::consts::{FRAC_PI_2, PI};

const EPS: f64 = f64::EPSILON;

/// Scales the closed forms are checked at: the plan's 1e-6 to 1e6, and
/// 1e±150, where an area still fits in `f64` but no frame-free product of
/// coordinates would at 1e±300.
const SCALES: [f64; 7] = [1e-150, 1e-6, 1e-3, 1.0, 1e3, 1e6, 1e150];

/// Translations, in units of the shape's scale.
const SHIFTS: [f64; 3] = [0.0, 1.0, 1e4];

/// A unit-construction point placed at `scale`, translated by `shift`
/// sizes along `(1, 0.7)`. Rounding here is the input's error: about `ε`
/// of the coordinate's magnitude, `(1 + shift)` sizes.
fn place(p: [f64; 2], scale: f64, shift: f64) -> [f64; 2] {
    [scale * (p[0] + shift), scale * (p[1] + 0.7 * shift)]
}

fn line(a: [f64; 2], b: [f64; 2]) -> Edge {
    Edge::Line { from: a, to: b }
}

fn arc(c: [f64; 2], s: [f64; 2], e: [f64; 2]) -> Edge {
    Edge::Arc {
        centre: c,
        start: s,
        end: e,
        reversed: false,
    }
}

/// The same loop traversed the other way.
fn reverse(edges: &[Edge]) -> Vec<Edge> {
    edges
        .iter()
        .rev()
        .map(|e| match *e {
            Edge::Line { from, to } => Edge::Line { from: to, to: from },
            Edge::Arc {
                centre,
                start,
                end,
                reversed,
            } => Edge::Arc {
                centre,
                start,
                end,
                reversed: !reversed,
            },
        })
        .collect()
}

/// A 120 × 60 rectangle from the origin, counter-clockwise: 7200.
fn rectangle(at: &dyn Fn([f64; 2]) -> [f64; 2]) -> Vec<Edge> {
    let p = [[0.0, 0.0], [120.0, 0.0], [120.0, 60.0], [0.0, 60.0]].map(at);
    (0..4).map(|i| line(p[i], p[(i + 1) % 4])).collect()
}

const SLOT_LENGTH: f64 = 20.0;
const SLOT_RADIUS: f64 = 5.0;

/// A slot: centres `(0, 0)` and `(20, 0)`, radius 5, counter-clockwise:
/// `2·length·r + πr²`.
fn slot(at: &dyn Fn([f64; 2]) -> [f64; 2]) -> Vec<Edge> {
    let (l, r) = (SLOT_LENGTH, SLOT_RADIUS);
    let [c1, c2, a, b, c, d] = [[0.0, 0.0], [l, 0.0], [0.0, -r], [l, -r], [l, r], [0.0, r]].map(at);
    vec![line(a, b), arc(c2, b, c), line(c, d), arc(c1, d, a)]
}

/// A D: the upper half of a radius-10 circle, closed by its diameter.
fn d_shape(at: &dyn Fn([f64; 2]) -> [f64; 2]) -> Vec<Edge> {
    let [c, s, e] = [[0.0, 0.0], [10.0, 0.0], [-10.0, 0.0]].map(at);
    vec![arc(c, s, e), line(e, s)]
}

const SEGMENT_RADIUS: f64 = 10.0;
/// Where the segments' arcs start, so none is axis-aligned.
const SEGMENT_PHASE: f64 = 0.3;

/// A circular segment: an arc of sweep `theta` from angle 0.3, closed by
/// its chord. Twice its area is `r²(θ − sin θ)`.
fn segment_loop(theta: f64, at: &dyn Fn([f64; 2]) -> [f64; 2]) -> Vec<Edge> {
    let r = SEGMENT_RADIUS;
    let polar = |phi: f64| [r * libm::cos(phi), r * libm::sin(phi)];
    let [c, s, e] = [
        [0.0, 0.0],
        polar(SEGMENT_PHASE),
        polar(SEGMENT_PHASE + theta),
    ]
    .map(at);
    vec![arc(c, s, e), line(e, s)]
}

fn assert_close(got: f64, want: f64, rel: f64, what: &str) {
    assert!(
        (got - want).abs() <= rel * want.abs(),
        "{what}: {got:e} against {want:e}, relative {:e} over {rel:e}",
        ((got - want) / want).abs()
    );
}

/// An area within the input's rounding of its closed form, and its sense.
fn check_area(edges: &[Edge], want: f64, rel: f64, what: &str) {
    let a = area(&Boundary::Edges(edges));
    assert_close(a.signed, want, rel, what);
    assert_eq!(a.orientation, Orientation::CounterClockwise, "{what}");
    let back = reverse(edges);
    let b = area(&Boundary::Edges(&back));
    assert_close(b.signed, -want, rel, &format!("{what}, reversed"));
    assert_eq!(b.orientation, Orientation::Clockwise, "{what}, reversed");
}

/// For loops of lines and arcs whose sweep is read from rounded points:
/// each point is off by `ε` of its magnitude, `(1 + shift)` sizes, which
/// moves an area by a few such parts of itself. 64 covers the summation.
fn polygon_error(shift: f64) -> f64 {
    64.0 * EPS * (1.0 + shift)
}

#[test]
fn rectangle_slot_d_and_circle_match_their_closed_forms() {
    let slot_area = 2.0 * SLOT_LENGTH * SLOT_RADIUS + PI * SLOT_RADIUS * SLOT_RADIUS;
    for scale in SCALES {
        for shift in SHIFTS {
            let at = |p| place(p, scale, shift);
            let s2 = scale * scale;
            let rel = polygon_error(shift);
            let tag = format!("scale {scale:e}, shift {shift:e}");
            check_area(
                &rectangle(&at),
                7200.0 * s2,
                rel,
                &format!("rectangle, {tag}"),
            );
            check_area(&slot(&at), slot_area * s2, rel, &format!("slot, {tag}"));
            check_area(&d_shape(&at), 50.0 * PI * s2, rel, &format!("D, {tag}"));

            let centre = at([0.0, 0.0]);
            for reversed in [false, true] {
                let a = area(&Boundary::Circle {
                    centre,
                    radius: 10.0 * scale,
                    reversed,
                });
                let want = if reversed { -100.0 } else { 100.0 } * PI * s2;
                assert_close(a.signed, want, 4.0 * EPS, &format!("circle, {tag}"));
            }
        }
    }
}

/// `θ − sin θ` in double-double arithmetic, from its Taylor series, to
/// about 1e-30 relative for `θ < 2π`: the reference the kernel's
/// `segment` is measured against.
fn segment_reference(theta: f64) -> Dd {
    let t2 = dd_mul(Dd(theta, 0.0), Dd(theta, 0.0));
    let mut term = dd_div(dd_mul(t2, Dd(theta, 0.0)), 6.0);
    let mut sum = Dd(0.0, 0.0);
    for k in 1..40 {
        sum = dd_add(sum, term);
        let n = f64::from(2 * k + 2) * f64::from(2 * k + 3);
        let next = dd_div(dd_mul(term, t2), n);
        term = Dd(-next.0, -next.1);
    }
    sum
}

/// `(got − want)/want` in ulps of `want`.
fn ulps(got: f64, want: Dd) -> f64 {
    let diff = dd_add(Dd(got, 0.0), Dd(-want.0, -want.1));
    (diff.0 / want.0).abs() / EPS
}

/// The small-θ threshold (plans/m9-profiles ⚠ OPEN, ADR-0012 D3), measured:
/// below `SMALL_SWEEP` the direct form loses many ulps, the series none;
/// from it up, the direct form is within the series' accuracy.
#[test]
fn small_sweep_threshold_is_measured() {
    let mut grid: Vec<f64> = (-1600..80)
        .map(|i| libm::pow(10.0, f64::from(i) / 200.0))
        .filter(|t| *t < TAU)
        .collect();
    grid.extend([
        SMALL_SWEEP,
        PI,
        TAU - 1e-3,
        TAU - 1e-9,
        TAU - 4.0 * EPS * TAU,
    ]);
    let (mut kernel, mut direct_below, mut direct_above) = (0.0_f64, 0.0_f64, 0.0_f64);
    for t in grid {
        let want = segment_reference(t);
        kernel = kernel.max(ulps(segment(t), want));
        let direct = ulps(t - libm::sin(t), want);
        if t < SMALL_SWEEP {
            direct_below = direct_below.max(direct);
        } else {
            direct_above = direct_above.max(direct);
        }
    }
    // Measured 2026-11-07: kernel 1.55 ulps, direct 1.18 from the
    // threshold up and over 1e8 below it (6ε/θ² at θ = 1e-8).
    assert!(kernel <= 2.0, "segment is {kernel} ulps off");
    assert!(
        direct_above <= 2.0,
        "direct form above the threshold: {direct_above}"
    );
    assert!(
        direct_below > 8.0,
        "direct form below the threshold: {direct_below}"
    );
}

#[test]
fn a_circular_segment_has_its_closed_area_at_every_sweep() {
    let sweeps = [
        1e-6,
        1e-3,
        0.5,
        SMALL_SWEEP,
        PI - 1e-6,
        PI,
        PI + 1e-6,
        3.0 * FRAC_PI_2,
        TAU - 1e-3,
        TAU - 1e-6,
    ];
    for theta in sweeps {
        let reference = segment_reference(theta);
        let unit = SEGMENT_RADIUS * SEGMENT_RADIUS * (reference.0 + reference.1) / 2.0;
        for scale in SCALES {
            for shift in SHIFTS {
                let at = |p| place(p, scale, shift);
                // The sweep read from rounded points is off by about the
                // input's error over the radius, `(1 + shift)` ulps; the
                // area moves by `(1 − cos θ)/(θ − sin θ)` of that, under
                // `1 + 3/θ` (3/θ as θ → 0).
                let rel = 16.0 * EPS * (1.0 + shift) * (1.0 + 3.0 / theta);
                let tag = format!("sweep {theta}, scale {scale:e}, shift {shift:e}");
                check_area(&segment_loop(theta, &at), unit * scale * scale, rel, &tag);
            }
        }
    }
}

#[test]
fn near_the_extremes_orientation_survives_and_area_saturates() {
    for scale in [1e300, 1e-300] {
        let at = |p| place(p, scale, 0.0);
        for (what, edges) in [
            ("rectangle", rectangle(&at)),
            ("slot", slot(&at)),
            ("D", d_shape(&at)),
            ("segment", segment_loop(1e-3, &at)),
        ] {
            let a = area(&Boundary::Edges(&edges));
            let b = area(&Boundary::Edges(&reverse(&edges)));
            assert_eq!(
                a.orientation,
                Orientation::CounterClockwise,
                "{what} {scale:e}"
            );
            assert_eq!(b.orientation, Orientation::Clockwise, "{what} {scale:e}");
            if scale > 1.0 {
                assert_eq!(a.signed, f64::MAX, "{what} {scale:e}");
                assert_eq!(b.signed, -f64::MAX, "{what} {scale:e}");
            } else {
                // 1e-600 and less: zero, with the sign in `orientation`.
                assert!(a.signed >= 0.0 && a.signed < 1e-290, "{what} {scale:e}");
                assert!(b.signed <= 0.0 && b.signed > -1e-290, "{what} {scale:e}");
            }
        }
        for reversed in [false, true] {
            let a = area(&Boundary::Circle {
                centre: at([0.0, 0.0]),
                radius: 10.0 * scale,
                reversed,
            });
            let sense = if reversed {
                Orientation::Clockwise
            } else {
                Orientation::CounterClockwise
            };
            assert_eq!(a.orientation, sense);
            assert!(a.signed.is_finite());
        }
    }

    // Corners at ±1.5e308: no offset or product overflows on the way.
    let m = 1.5e308;
    let square = rectangle(&|p: [f64; 2]| [(p[0] / 60.0 - 1.0) * m, (p[1] / 30.0 - 1.0) * m]);
    let a = area(&Boundary::Edges(&square));
    assert_eq!(
        (a.signed, a.orientation),
        (f64::MAX, Orientation::CounterClockwise)
    );
    assert_eq!(winding(&Boundary::Edges(&square), [0.0, 0.0]), 1);
    assert_eq!(winding(&Boundary::Edges(&square), [-f64::MAX, 0.0]), 0);
}

/// The winding number about each probe, forwards and reversed.
fn check_winding(edges: &[Edge], probes: &[([f64; 2], i64)], what: &str) {
    let back = reverse(edges);
    for (q, want) in probes {
        assert_eq!(
            winding(&Boundary::Edges(edges), *q),
            *want,
            "{what} at {q:?}"
        );
        assert_eq!(
            winding(&Boundary::Edges(&back), *q),
            -want,
            "{what} reversed at {q:?}"
        );
    }
}

#[test]
fn winding_counts_turns_inside_rectangle_and_slot() {
    for scale in SCALES.into_iter().chain([1e300, 1e-300]) {
        for shift in SHIFTS {
            let at = |p| place(p, scale, shift);
            let tag = format!("scale {scale:e}, shift {shift:e}");
            let rect = [([60.0, 30.0], 1), ([-10.0, 30.0], 0), ([60.0, 61.0], 0)];
            let rect = rect.map(|(p, w)| (at(p), w));
            check_winding(&rectangle(&at), &rect, &format!("rectangle, {tag}"));
            let (l, r) = (SLOT_LENGTH, SLOT_RADIUS);
            let caps = [
                ([l / 2.0, 0.0], 1),
                // The caps' centres, exactly on their chords: a cross
                // product of `−0` there must still read the left side.
                ([0.0, 0.0], 1),
                ([l, 0.0], 1),
                ([l + 0.9 * r * 0.8, 0.9 * r * 0.6], 1),
                ([-0.9 * r, 0.0], 1),
                ([l + 1.1 * r, 0.0], 0),
                ([l + 0.8 * r, 0.8 * r], 0),
            ];
            let caps = caps.map(|(p, w)| (at(p), w));
            check_winding(&slot(&at), &caps, &format!("slot, {tag}"));
        }
    }
}

/// A billionth of the radius either side of a segment's chord, at its
/// middle, and of its arc, at half its sweep: inside exactly on the
/// segment's side of each.
#[test]
fn winding_tells_the_sides_of_an_arcs_chord_and_arc_apart() {
    let delta = 1e-9;
    let sweeps = [1e-3, FRAC_PI_2, PI, 3.0 * FRAC_PI_2, TAU - 1e-3];
    for theta in sweeps {
        let r = SEGMENT_RADIUS;
        let (phi0, phi1) = (SEGMENT_PHASE, SEGMENT_PHASE + theta);
        let s = [r * libm::cos(phi0), r * libm::sin(phi0)];
        let e = [r * libm::cos(phi1), r * libm::sin(phi1)];
        let mid = [(s[0] + e[0]) / 2.0, (s[1] + e[1]) / 2.0];
        // The unit normal pointing right of the chord s → e: the arc's side.
        let len = libm::hypot(e[0] - s[0], e[1] - s[1]);
        let right = [(e[1] - s[1]) / len, -(e[0] - s[0]) / len];
        let half = phi0 + theta / 2.0;
        let apex = [libm::cos(half), libm::sin(half)];
        let probes = [
            (
                [mid[0] + delta * r * right[0], mid[1] + delta * r * right[1]],
                1,
            ),
            (
                [mid[0] - delta * r * right[0], mid[1] - delta * r * right[1]],
                0,
            ),
            (
                [r * (1.0 - delta) * apex[0], r * (1.0 - delta) * apex[1]],
                1,
            ),
            (
                [r * (1.0 + delta) * apex[0], r * (1.0 + delta) * apex[1]],
                0,
            ),
        ];
        for scale in [1e-6, 1.0, 1e6, 1e300, 1e-300] {
            for shift in [0.0, 1.0] {
                let at = |p| place(p, scale, shift);
                let probes = probes.map(|(p, w)| (at(p), w));
                let tag = format!("sweep {theta}, scale {scale:e}, shift {shift}");
                check_winding(&segment_loop(theta, &at), &probes, &tag);
            }
        }
    }
}

#[test]
fn winding_about_a_circle_and_far_away() {
    for scale in [1e-300, 1e-6, 1.0, 1e6, 1e300] {
        for shift in SHIFTS {
            let at = |p| place(p, scale, shift);
            for reversed in [false, true] {
                let circle = Boundary::Circle {
                    centre: at([0.0, 0.0]),
                    radius: 10.0 * scale,
                    reversed,
                };
                let sense = if reversed { -1 } else { 1 };
                assert_eq!(winding(&circle, at([9.9, 0.0])), sense);
                assert_eq!(winding(&circle, at([7.0, 7.2])), 0);
            }
        }
    }
    // A loop a millionth across, probed at the far end of `f64`.
    let tiny = rectangle(&|p| place(p, 1e-8, 0.0));
    for q in [[f64::MAX, 0.0], [-f64::MAX, f64::MAX], [1e-300, 1e300]] {
        assert_eq!(winding(&Boundary::Edges(&tiny), q), 0);
    }
}

#[test]
fn a_probe_lies_mid_edge() {
    for scale in [1e-300, 1.0, 1e300] {
        let at = |p| place(p, scale, 0.0);
        let rect = rectangle(&at);
        let p = probe(&Boundary::Edges(&rect)).unwrap();
        assert_close(p[0], 60.0 * scale, EPS, "line midpoint");
        assert_eq!(p[1], 0.0);
        // The D's arc from angle 0 to π: its probe is at the top.
        let d = d_shape(&at);
        let p = probe(&Boundary::Edges(&d)).unwrap();
        assert!(p[0].abs() <= 4.0 * EPS * 10.0 * scale, "{p:?}");
        assert_close(p[1], 10.0 * scale, 4.0 * EPS, "arc midpoint");
        let p = probe(&Boundary::Circle {
            centre: at([1.0, 2.0]),
            radius: 10.0 * scale,
            reversed: false,
        })
        .unwrap();
        assert_close(p[0], 11.0 * scale, 2.0 * EPS, "circle probe x");
        assert_close(p[1], 2.0 * scale, 2.0 * EPS, "circle probe y");
    }
    assert_eq!(probe(&Boundary::Edges(&[])), None);
}

// Double-double arithmetic (Dekker; Hida, Li and Bailey's QD library), for
// the reference only.

#[derive(Clone, Copy, Debug)]
struct Dd(f64, f64);

fn two_sum(a: f64, b: f64) -> Dd {
    let s = a + b;
    let v = s - a;
    Dd(s, (a - (s - v)) + (b - v))
}

fn quick_two_sum(a: f64, b: f64) -> Dd {
    let s = a + b;
    Dd(s, b - (s - a))
}

fn dd_add(x: Dd, y: Dd) -> Dd {
    let s = two_sum(x.0, y.0);
    let t = two_sum(x.1, y.1);
    let u = quick_two_sum(s.0, s.1 + t.0);
    quick_two_sum(u.0, u.1 + t.1)
}

fn dd_mul(x: Dd, y: Dd) -> Dd {
    let p = x.0 * y.0;
    let e = x.0.mul_add(y.0, -p) + (x.0 * y.1 + x.1 * y.0);
    quick_two_sum(p, e)
}

fn dd_div(x: Dd, d: f64) -> Dd {
    let q1 = x.0 / d;
    let p = dd_mul(Dd(q1, 0.0), Dd(d, 0.0));
    let r = dd_add(x, Dd(-p.0, -p.1));
    quick_two_sum(q1, r.0 / d)
}
