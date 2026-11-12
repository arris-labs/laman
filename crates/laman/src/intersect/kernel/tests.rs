#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! The pair kernel against closed forms: lines and circles built through
//! chosen meeting points, at scales 1e-6 to 1e6 and 1e±300, translated up
//! to 1e4 times their size, turned, with the tolerance an input at the
//! scale (ADR-0016 D2), and pairs either side of tangency by half and twice
//! the tolerance.
use super::*;
use crate::frame::{cross, dot, sub};

/// Scales the closed forms are checked at: the plan's 1e-6 to 1e6, and
/// 1e±300, where only the frame keeps products of coordinates finite.
const SCALES: [f64; 7] = [1e-300, 1e-6, 1e-3, 1.0, 1e3, 1e6, 1e300];

/// Translations, in units of the construction's scale.
const SHIFTS: [f64; 3] = [0.0, 1.0, 1e4];

/// Turns: none, and the 3-4-5 rotation (its cosine and sine rounded, the
/// same rounding in the input and the expectation).
const TURNS: [[f64; 2]; 2] = [[1.0, 0.0], [0.6, 0.8]];

/// The length tolerance passed to the kernel, relative to the scale: the
/// size `Tolerances::at` gives a sketch of this extent, order of magnitude.
const REL: f64 = 1e-9;

/// Agreement with the closed form, relative to the scale: the input's own
/// rounding is `ε` of a coordinate, `1e4` sizes at the largest shift, about
/// `2e-12`; the kernel may lose two digits more on these well-conditioned
/// pairs, never more.
const ACC: f64 = 1e-10;

/// A construction at one scale, shift and turn.
#[derive(Clone, Copy, Debug)]
struct Case {
    s: f64,
    shift: f64,
    turn: [f64; 2],
}

fn cases() -> Vec<Case> {
    let mut out = Vec::new();
    for s in SCALES {
        for shift in SHIFTS {
            for turn in TURNS {
                out.push(Case { s, shift, turn });
            }
        }
    }
    out
}

impl Case {
    fn rot(&self, v: [f64; 2]) -> [f64; 2] {
        let [c, n] = self.turn;
        [c * v[0] - n * v[1], n * v[0] + c * v[1]]
    }

    /// A unit-construction point in the caller's coordinates.
    fn p(&self, v: [f64; 2]) -> [f64; 2] {
        let r = self.rot(v);
        [
            self.s * (r[0] + self.shift),
            self.s * (r[1] + 0.7 * self.shift),
        ]
    }

    fn line(&self, a: [f64; 2], b: [f64; 2]) -> Curve {
        Curve::Line {
            a: self.p(a),
            b: self.p(b),
        }
    }

    fn circle(&self, c: [f64; 2], r: f64) -> Curve {
        Curve::Circle {
            centre: self.p(c),
            radius: self.s * r,
        }
    }

    fn tol(&self) -> f64 {
        REL * self.s
    }

    /// The circle parameter of the unit-construction direction `v`, turned.
    fn angle(&self, v: [f64; 2]) -> f64 {
        let r = self.rot(v);
        let t = libm::atan2(r[1], r[0]) / TAU;
        if t < 0.0 { t + 1.0 } else { t }
    }
}

/// A parameter's distance from its closed form; on a circle, round the
/// turn, so 0 and a hair below 1 agree.
fn param_gap(got: f64, want: f64, round: bool) -> f64 {
    let d = (got - want).abs();
    if round { d.min(1.0 - d) } else { d }
}

fn point_gap(got: [f64; 2], want: [f64; 2]) -> f64 {
    (got[0] - want[0]).abs().max((got[1] - want[1]).abs())
}

/// `got` against `want`, meeting by meeting: points within `acc` of the
/// scale, parameters within `acc` (round the turn on a circle, `rounds`).
#[track_caller]
fn check(c: &Case, got: &[Meeting], want: &[Meeting], acc: f64, rounds: [bool; 2]) {
    let pt = acc * c.s;
    assert_eq!(got.len(), want.len(), "{c:?}: {got:?} vs {want:?}");
    for (g, w) in got.iter().zip(want) {
        let ctx = || format!("{c:?}: {g:?} vs {w:?}");
        assert!(point_gap(g.at, w.at) <= pt, "{}", ctx());
        assert!(
            param_gap(g.on_first, w.on_first, rounds[0]) <= acc,
            "{}",
            ctx()
        );
        assert!(
            param_gap(g.on_second, w.on_second, rounds[1]) <= acc,
            "{}",
            ctx()
        );
        match (g.kind, w.kind) {
            (Kind::Crossing, Kind::Crossing) | (Kind::Touch, Kind::Touch) => {}
            (
                Kind::Overlap {
                    to,
                    to_first,
                    to_second,
                },
                Kind::Overlap {
                    to: wt,
                    to_first: wf,
                    to_second: ws,
                },
            ) => {
                assert!(point_gap(to, wt) <= pt, "{}", ctx());
                // A whole turn ends at 1, not round to 0.
                assert!((to_first - wf).abs() <= acc, "{}", ctx());
                assert!((to_second - ws).abs() <= acc, "{}", ctx());
            }
            _ => panic!("kind: {}", ctx()),
        }
    }
}

fn m(kind: Kind, at: [f64; 2], on_first: f64, on_second: f64) -> Meeting {
    Meeting {
        kind,
        at,
        on_first,
        on_second,
    }
}

/// The same meetings seen from the other curve: parameters swapped, an
/// overlap from its end lowest on the new first curve, reordered.
fn swapped(want: &[Meeting]) -> Vec<Meeting> {
    let mut out: Vec<Meeting> = want
        .iter()
        .map(|w| match w.kind {
            Kind::Overlap {
                to,
                to_first,
                to_second,
            } if to_second < w.on_second => m(
                Kind::Overlap {
                    to: w.at,
                    to_first: w.on_second,
                    to_second: w.on_first,
                },
                to,
                to_second,
                to_first,
            ),
            Kind::Overlap {
                to,
                to_first,
                to_second,
            } => m(
                Kind::Overlap {
                    to,
                    to_first: to_second,
                    to_second: to_first,
                },
                w.at,
                w.on_second,
                w.on_first,
            ),
            Kind::Crossing | Kind::Touch => m(w.kind, w.at, w.on_second, w.on_first),
        })
        .collect();
    out.sort_by(|x, y| x.on_first.total_cmp(&y.on_first));
    out
}

/// `first` and `second` meet as `want` (ordered by the first's parameter),
/// in both orders.
#[track_caller]
fn both(c: &Case, first: &Curve, second: &Curve, want: &[Meeting], acc: f64) {
    let round = |k: &Curve| matches!(k, Curve::Circle { .. });
    let (r1, r2) = (round(first), round(second));
    check(c, &meet(first, second, c.tol()), want, acc, [r1, r2]);
    check(
        c,
        &meet(second, first, c.tol()),
        &swapped(want),
        acc,
        [r2, r1],
    );
}

fn ends(curve: &Curve) -> [[f64; 2]; 2] {
    match *curve {
        Curve::Line { a, b } => [a, b],
        Curve::Circle { .. } | Curve::Arc { .. } => panic!("not a line"),
    }
}

#[test]
fn lines_cross_at_their_construction() {
    for c in cases() {
        // Through (1, 2): halfway along the first, four fifths along the
        // second.
        let l1 = c.line([-1.0, 0.0], [3.0, 4.0]);
        let l2 = c.line([1.0, -2.0], [1.0, 3.0]);
        let want = [m(Kind::Crossing, c.p([1.0, 2.0]), 0.5, 0.8)];
        both(&c, &l1, &l2, &want, ACC);
    }
}

#[test]
fn lines_that_miss_do_not_meet() {
    for c in cases() {
        let l1 = c.line([-1.0, 0.0], [3.0, 4.0]);
        // Their carriers cross a whole unit past the second's end.
        let short = c.line([1.0, -2.0], [1.0, 1.0]);
        // Parallel, √2 apart.
        let parallel = c.line([0.0, -1.0], [4.0, 3.0]);
        for other in [short, parallel] {
            assert!(meet(&l1, &other, c.tol()).is_empty(), "{c:?}");
            assert!(meet(&other, &l1, c.tol()).is_empty(), "{c:?}");
        }
    }
}

#[test]
fn an_end_on_a_line_is_at_that_end() {
    for c in cases() {
        let l1 = c.line([-1.0, 0.0], [3.0, 4.0]);
        // A T: the second ends on the first, exactly and half the tolerance
        // short of it.
        for gap in [0.0, 0.5 * REL] {
            let l2 = c.line([1.0, -2.0], [1.0, 2.0 - gap]);
            let end = ends(&l2)[1];
            let got = meet(&l1, &l2, c.tol());
            check(
                &c,
                &got,
                &[m(Kind::Crossing, end, 0.5, 1.0)],
                ACC,
                [false; 2],
            );
            assert_eq!((got[0].at, got[0].on_second), (end, 1.0), "{c:?}");
            let got = meet(&l2, &l1, c.tol());
            assert_eq!((got[0].at, got[0].on_first), (end, 1.0), "{c:?}");
        }
        // Twice the tolerance short: no meeting.
        let l2 = c.line([1.0, -2.0], [1.0, 2.0 - 2.0 * REL]);
        assert!(meet(&l1, &l2, c.tol()).is_empty(), "{c:?}");
    }
}

#[test]
fn an_unjoined_corner_is_a_crossing_at_the_first_end() {
    for c in cases() {
        let l1 = c.line([0.0, 0.0], [2.0, 0.0]);
        for gap in [0.0, 0.5 * REL] {
            let l2 = c.line([2.0, gap], [2.0, 3.0]);
            let got = meet(&l1, &l2, c.tol());
            assert_eq!(got.len(), 1, "{c:?}");
            assert_eq!(got[0].kind, Kind::Crossing, "{c:?}");
            assert_eq!(
                (got[0].at, got[0].on_first, got[0].on_second),
                (ends(&l1)[1], 1.0, 0.0),
                "{c:?}"
            );
            let got = meet(&l2, &l1, c.tol());
            assert_eq!(
                (got[0].at, got[0].on_first, got[0].on_second),
                (ends(&l2)[0], 0.0, 1.0),
                "{c:?}"
            );
        }
    }
}

#[test]
fn a_line_crosses_a_circle_twice_in_order() {
    for c in cases() {
        // Radius 5 about (1, −1), the line 3 above the centre: ±4 across.
        let k = c.circle([1.0, -1.0], 5.0);
        let l = c.line([-5.0, 2.0], [7.0, 2.0]);
        let want = [
            m(
                Kind::Crossing,
                c.p([-3.0, 2.0]),
                2.0 / 12.0,
                c.angle([-4.0, 3.0]),
            ),
            m(
                Kind::Crossing,
                c.p([5.0, 2.0]),
                10.0 / 12.0,
                c.angle([4.0, 3.0]),
            ),
        ];
        both(&c, &l, &k, &want, ACC);
        // Starting at the centre's abscissa, it crosses once.
        let half = c.line([1.0, 2.0], [7.0, 2.0]);
        let want = [m(
            Kind::Crossing,
            c.p([5.0, 2.0]),
            4.0 / 6.0,
            c.angle([4.0, 3.0]),
        )];
        both(&c, &half, &k, &want, ACC);
    }
}

#[test]
fn a_line_near_tangency_touches_within_the_tolerance() {
    for c in cases() {
        let k = c.circle([1.0, -1.0], 5.0);
        // Half the tolerance either side of tangency: a touch at the foot.
        for delta in [-0.5 * REL, 0.5 * REL] {
            let l = c.line([-5.0, 4.0 + delta], [7.0, 4.0 + delta]);
            let want = [m(
                Kind::Touch,
                c.p([1.0, 4.0 + delta]),
                0.5,
                c.angle([0.0, 1.0]),
            )];
            both(&c, &l, &k, &want, ACC);
        }
        // Twice the tolerance inside: two crossings `h` either side of the
        // foot, a double root's conditioning `r/h` on the abscissa and the
        // parameters.
        let d = 5.0 - 2.0 * REL;
        let h = ((5.0 - d) * (5.0 + d)).sqrt();
        let l = c.line([-5.0, 4.0 - 2.0 * REL], [7.0, 4.0 - 2.0 * REL]);
        let want = [
            m(
                Kind::Crossing,
                c.p([1.0 - h, -1.0 + d]),
                (6.0 - h) / 12.0,
                c.angle([-h, d]),
            ),
            m(
                Kind::Crossing,
                c.p([1.0 + h, -1.0 + d]),
                (6.0 + h) / 12.0,
                c.angle([h, d]),
            ),
        ];
        let acc = ACC * 5.0 / h;
        let got = meet(&l, &k, c.tol());
        check(&c, &got, &want, acc, [false, true]);
        // Twice the tolerance outside: none.
        let l = c.line([-5.0, 4.0 + 2.0 * REL], [7.0, 4.0 + 2.0 * REL]);
        assert!(meet(&l, &k, c.tol()).is_empty(), "{c:?}");
        // A line ending at the tangent point touches at its own end.
        let l = c.line([-5.0, 4.0], [1.0, 4.0]);
        let got = meet(&l, &k, c.tol());
        assert_eq!(got.len(), 1, "{c:?}");
        assert_eq!(got[0].kind, Kind::Touch, "{c:?}");
        assert_eq!((got[0].at, got[0].on_first), (ends(&l)[1], 1.0), "{c:?}");
    }
}

#[test]
fn two_circles_cross_at_the_closed_form() {
    for c in cases() {
        let k1 = c.circle([0.0, 0.0], 5.0);
        // Equal radii 6 apart, and radius 4 at 3: both through (3, ±4).
        for (centre, r) in [([6.0, 0.0], 5.0), ([3.0, 0.0], 4.0)] {
            let k2 = c.circle(centre, r);
            let mut want = [
                m(
                    Kind::Crossing,
                    c.p([3.0, 4.0]),
                    c.angle([3.0, 4.0]),
                    c.angle([3.0 - centre[0], 4.0]),
                ),
                m(
                    Kind::Crossing,
                    c.p([3.0, -4.0]),
                    c.angle([3.0, -4.0]),
                    c.angle([3.0 - centre[0], -4.0]),
                ),
            ];
            want.sort_by(|x, y| x.on_first.total_cmp(&y.on_first));
            both(&c, &k1, &k2, &want, ACC);
        }
    }
}

#[test]
fn circles_touch_within_the_tolerance() {
    for c in cases() {
        let k1 = c.circle([0.0, 0.0], 5.0);
        let at = c.p([5.0, 0.0]);
        for delta in [-0.5 * REL, 0.5 * REL] {
            // Outside each other, radii 5 and 3, centres 8 apart.
            let k2 = c.circle([8.0 + delta, 0.0], 3.0);
            let want = [m(
                Kind::Touch,
                at,
                c.angle([1.0, 0.0]),
                c.angle([-1.0, 0.0]),
            )];
            check(&c, &meet(&k1, &k2, c.tol()), &want, ACC, [true; 2]);
            // Radius 2 inside radius 5, centres 3 apart; the first round's
            // point on the line of centres, whichever is inside.
            let inner = c.circle([3.0 + delta, 0.0], 2.0);
            let want = [m(Kind::Touch, at, c.angle([1.0, 0.0]), c.angle([1.0, 0.0]))];
            check(&c, &meet(&k1, &inner, c.tol()), &want, ACC, [true; 2]);
            let want = [m(
                Kind::Touch,
                c.p([5.0 + delta, 0.0]),
                c.angle([1.0, 0.0]),
                c.angle([1.0, 0.0]),
            )];
            check(&c, &meet(&inner, &k1, c.tol()), &want, ACC, [true; 2]);
        }
        // Twice the tolerance into each other: two crossings `h` either side
        // of the line of centres; twice apart: none.
        let crossing = |k2: &Curve| {
            let got = meet(&k1, k2, c.tol());
            assert_eq!(got.len(), 2, "{c:?}: {got:?}");
            for g in &got {
                assert_eq!(g.kind, Kind::Crossing, "{c:?}");
            }
        };
        crossing(&c.circle([8.0 - 2.0 * REL, 0.0], 3.0));
        crossing(&c.circle([3.0 + 2.0 * REL, 0.0], 2.0));
        assert!(meet(&k1, &c.circle([8.0 + 2.0 * REL, 0.0], 3.0), c.tol()).is_empty());
        assert!(meet(&k1, &c.circle([3.0 - 2.0 * REL, 0.0], 2.0), c.tol()).is_empty());
    }
}

#[test]
fn concentric_circles_overlap_or_miss() {
    for c in cases() {
        let k1 = c.circle([1.0, -1.0], 5.0);
        let Curve::Circle { centre, .. } = k1 else {
            unreachable!()
        };
        // The first's point at angle 0 in the caller's coordinates.
        let at = [centre[0] + 5.0 * c.s, centre[1]];
        let whole = Kind::Overlap {
            to: at,
            to_first: 1.0,
            to_second: 1.0,
        };
        for k2 in [
            c.circle([1.0, -1.0], 5.0),
            c.circle([1.0 + 0.5 * REL, -1.0], 5.0 + 0.5 * REL),
        ] {
            let got = meet(&k1, &k2, c.tol());
            check(&c, &got, &[m(whole, at, 0.0, 0.0)], ACC, [false; 2]);
        }
        for k2 in [
            c.circle([1.0, -1.0], 4.0),
            c.circle([1.0, -1.0], 5.0 + 2.0 * REL),
        ] {
            assert!(meet(&k1, &k2, c.tol()).is_empty(), "{c:?}");
        }
    }
}

#[test]
fn collinear_lines_overlap_on_their_shared_stretch() {
    for c in cases() {
        let l1 = c.line([0.0, 0.0], [4.0, 0.0]);
        let e1 = ends(&l1);
        // Overhanging, the same way and the opposite way, and contained.
        let same = c.line([1.0, 0.0], [6.0, 0.0]);
        let opposite = c.line([6.0, 0.0], [1.0, 0.0]);
        let inside = c.line([1.0, 0.0], [3.0, 0.0]);
        let overlap = |to: [f64; 2], to_first: f64, to_second: f64| Kind::Overlap {
            to,
            to_first,
            to_second,
        };
        let cases_ = [
            (same, overlap(e1[1], 1.0, 0.6), ends(&same)[0], 0.0),
            (opposite, overlap(e1[1], 1.0, 0.4), ends(&opposite)[1], 1.0),
            (
                inside,
                overlap(ends(&inside)[1], 0.75, 1.0),
                ends(&inside)[0],
                0.0,
            ),
        ];
        for (l2, kind, at, on_second) in cases_ {
            let want = [m(kind, at, 0.25, on_second)];
            both(&c, &l1, &l2, &want, ACC);
            // Each end is a curve's own, bit for bit.
            let got = meet(&l1, &l2, c.tol());
            let Kind::Overlap { to, .. } = got[0].kind else {
                panic!("{c:?}")
            };
            let Kind::Overlap { to: want_to, .. } = kind else {
                unreachable!()
            };
            assert_eq!((got[0].at, to), (at, want_to), "{c:?}");
        }
    }
}

#[test]
fn collinear_lines_end_to_end_touch_or_miss() {
    for c in cases() {
        let l1 = c.line([0.0, 0.0], [4.0, 0.0]);
        for gap in [0.0, 0.5 * REL] {
            let l2 = c.line([4.0 + gap, 0.0], [6.0, 0.0]);
            let got = meet(&l1, &l2, c.tol());
            assert_eq!(got.len(), 1, "{c:?}");
            assert_eq!(got[0].kind, Kind::Touch, "{c:?}");
            assert_eq!(
                (got[0].at, got[0].on_first, got[0].on_second),
                (ends(&l1)[1], 1.0, 0.0),
                "{c:?}"
            );
            let got = meet(&l2, &l1, c.tol());
            assert_eq!(
                (got[0].at, got[0].on_first, got[0].on_second),
                (ends(&l2)[0], 0.0, 1.0),
                "{c:?}"
            );
        }
        let l2 = c.line([4.0 + 2.0 * REL, 0.0], [6.0, 0.0]);
        assert!(meet(&l1, &l2, c.tol()).is_empty(), "{c:?}");
    }
}

#[test]
fn degenerate_curves_meet_nothing() {
    for c in cases() {
        let line = c.line([-5.0, 0.0], [5.0, 0.0]);
        let circle = c.circle([0.0, 0.0], 3.0);
        for dot in [
            c.line([0.0, 0.0], [0.0, 0.0]),
            c.line([0.0, 0.0], [0.5 * REL, 0.0]),
            c.circle([0.0, 0.0], 0.0),
            c.circle([3.0, 0.0], 0.5 * REL),
        ] {
            for other in [&line, &circle] {
                assert!(meet(&dot, other, c.tol()).is_empty(), "{c:?} {dot:?}");
                assert!(meet(other, &dot, c.tol()).is_empty(), "{c:?} {dot:?}");
            }
        }
    }
}

// Arcs (ADR-0016 D6). Unit constructions on the circle of radius 5 about the
// origin: E, N, W, S the axis points; A, B, C, D the 3-4-5 points at
// (4, 3), (−4, 3), (−3, −4), (4, −3).
const E: [f64; 2] = [5.0, 0.0];
const N: [f64; 2] = [0.0, 5.0];
const W: [f64; 2] = [-5.0, 0.0];
const S: [f64; 2] = [0.0, -5.0];
const A: [f64; 2] = [4.0, 3.0];
const B: [f64; 2] = [-4.0, 3.0];
const C: [f64; 2] = [-3.0, -4.0];

impl Case {
    fn arc(&self, centre: [f64; 2], start: [f64; 2], end: [f64; 2]) -> Curve {
        Curve::Arc {
            centre: self.p(centre),
            start: self.p(start),
            end: self.p(end),
        }
    }
}

/// The counter-clockwise angle from `from`'s direction to `to`'s about
/// `centre`, in [0, 2π), on unit-construction points.
fn ccw(centre: [f64; 2], from: [f64; 2], to: [f64; 2]) -> f64 {
    let (u, v) = (sub(from, centre), sub(to, centre));
    let t = libm::atan2(cross(u, v), dot(u, v));
    if t < 0.0 { t + TAU } else { t }
}

/// A point's fraction of the arc `start` to `end` about `centre`.
fn frac(centre: [f64; 2], start: [f64; 2], end: [f64; 2], p: [f64; 2]) -> f64 {
    ccw(centre, start, p) / ccw(centre, start, end)
}

fn span(to: [f64; 2], to_first: f64, to_second: f64) -> Kind {
    Kind::Overlap {
        to,
        to_first,
        to_second,
    }
}

#[test]
fn an_arc_keeps_the_roots_in_its_sweep() {
    for c in cases() {
        let o = [0.0, 0.0];
        // Through A and B, at the height 3: along the line from x = −5.
        let line = c.line([-5.0, 3.0], [7.0, 3.0]);
        let t = |x: f64| (x + 5.0) / 12.0;
        let root = |p: [f64; 2], arc: &[[f64; 2]; 2]| {
            m(Kind::Crossing, c.p(p), t(p[0]), frac(o, arc[0], arc[1], p))
        };
        // The upper half holds both, a quarter only A, the left half only B.
        let half = [E, W];
        let quarter = [E, N];
        let left = [N, S];
        for (arc, roots) in [(half, vec![B, A]), (quarter, vec![A]), (left, vec![B])] {
            let k = c.arc(o, arc[0], arc[1]);
            // Ordered along the line: B (x = −4) before A.
            let want: Vec<Meeting> = roots.iter().map(|p| root(*p, &arc)).collect();
            let got = meet(&line, &k, c.tol());
            check(&c, &got, &want, ACC, [false; 2]);
            let mut back: Vec<Meeting> = want
                .iter()
                .map(|w| m(w.kind, w.at, w.on_second, w.on_first))
                .collect();
            back.sort_by(|x, y| x.on_first.total_cmp(&y.on_first));
            check(&c, &meet(&k, &line, c.tol()), &back, ACC, [false; 2]);
        }
    }
}

#[test]
fn an_arc_whose_circle_crosses_outside_its_sweep_does_not_meet() {
    for c in cases() {
        // The lower-left quarter, W to S, and a line across the top.
        let k = c.arc([0.0, 0.0], W, S);
        let line = c.line([-5.0, 3.0], [7.0, 3.0]);
        assert!(meet(&line, &k, c.tol()).is_empty(), "{c:?}");
        assert!(meet(&k, &line, c.tol()).is_empty(), "{c:?}");
        // The same circle whole does cross it.
        let whole = c.circle([0.0, 0.0], 5.0);
        assert_eq!(meet(&line, &whole, c.tol()).len(), 2, "{c:?}");
    }
}

#[test]
fn a_root_within_the_tolerance_beyond_an_arc_end_is_at_that_end() {
    for c in cases() {
        let o = [0.0, 0.0];
        let line = c.line([-5.0, 3.0], [7.0, 3.0]);
        // The arc from E stopping `gap` of arc length short of A, along the
        // circle (the tangent at A is (−0.8, 0.6) counter-clockwise).
        for gap in [0.0, 0.5 * REL] {
            let end = [4.0 + 0.8 * gap, 3.0 - 0.6 * gap];
            let k = c.arc(o, E, end);
            let Curve::Arc { end: own, .. } = k else {
                unreachable!()
            };
            let got = meet(&line, &k, c.tol());
            assert_eq!(got.len(), 1, "{c:?}");
            assert_eq!(got[0].kind, Kind::Crossing, "{c:?}");
            assert_eq!((got[0].at, got[0].on_second), (own, 1.0), "{c:?}");
            assert!((got[0].on_first - 9.0 / 12.0).abs() <= ACC, "{c:?}");
            let got = meet(&k, &line, c.tol());
            assert_eq!((got[0].at, got[0].on_first), (own, 1.0), "{c:?}");
        }
        // Twice the tolerance short: none.
        let k = c.arc(o, E, [4.0 + 1.6 * REL, 3.0 - 1.2 * REL]);
        assert!(meet(&line, &k, c.tol()).is_empty(), "{c:?}");
    }
}

#[test]
fn an_arc_end_on_a_line_is_a_crossing_at_that_end() {
    for c in cases() {
        let o = [0.0, 0.0];
        // E to A, and the vertical through A: its other root (4, −3) is
        // outside the sweep.
        let k = c.arc(o, E, A);
        let line = c.line([4.0, -5.0], [4.0, 9.0]);
        let Curve::Arc { end, .. } = k else {
            unreachable!()
        };
        let got = meet(&line, &k, c.tol());
        assert_eq!(got.len(), 1, "{c:?}");
        assert_eq!(
            (got[0].kind, got[0].at, got[0].on_second),
            (Kind::Crossing, end, 1.0),
            "{c:?}"
        );
        assert!((got[0].on_first - 8.0 / 14.0).abs() <= ACC, "{c:?}");
    }
}

#[test]
fn two_arcs_cross_where_both_sweeps_hold() {
    for c in cases() {
        // About the origin, the upper half; about (6, 0), N' to W' (its
        // upper left quarter). The circles cross at (3, ±4); only (3, 4) is
        // on both.
        let first = c.arc([0.0, 0.0], E, W);
        let (n2, w2) = ([6.0, 5.0], [1.0, 0.0]);
        let second = c.arc([6.0, 0.0], n2, w2);
        let p = [3.0, 4.0];
        let want = [m(
            Kind::Crossing,
            c.p(p),
            frac([0.0, 0.0], E, W, p),
            frac([6.0, 0.0], n2, w2, p),
        )];
        both(&c, &first, &second, &want, ACC);
    }
}

#[test]
fn arcs_of_one_circle_overlap_on_their_shared_sweeps() {
    for c in cases() {
        let o = [0.0, 0.0];
        // E→W and N→S share N→W.
        let (a, b) = (c.arc(o, E, W), c.arc(o, N, S));
        let (ea, eb) = (arc_ends(&a), arc_ends(&b));
        let want = [m(span(ea[1], 1.0, 0.5), eb[0], 0.5, 0.0)];
        check(&c, &meet(&a, &b, c.tol()), &want, ACC, [false; 2]);
        // From b's side: b's start is where the shared stretch starts on b.
        let want = [m(span(ea[1], 0.5, 1.0), eb[0], 0.0, 0.5)];
        check(&c, &meet(&b, &a, c.tol()), &want, ACC, [false; 2]);
        // Two stretches: E→C (to 4.07) and B→A (round the far side, through
        // S and E) share E→A and B→C.
        let (a, b) = (c.arc(o, E, C), c.arc(o, B, A));
        let (ea, eb) = (arc_ends(&a), arc_ends(&b));
        let sa = ccw(o, E, C);
        let sb = ccw(o, B, A);
        let want = [
            m(
                span(eb[1], ccw(o, E, A) / sa, 1.0),
                ea[0],
                0.0,
                ccw(o, B, E) / sb,
            ),
            m(
                span(ea[1], 1.0, ccw(o, B, C) / sb),
                eb[0],
                ccw(o, E, B) / sa,
                0.0,
            ),
        ];
        let got = meet(&a, &b, c.tol());
        check(&c, &got, &want, ACC, [false; 2]);
    }
}

fn arc_ends(k: &Curve) -> [[f64; 2]; 2] {
    match *k {
        Curve::Arc { start, end, .. } => [start, end],
        Curve::Line { .. } | Curve::Circle { .. } => panic!("not an arc"),
    }
}

#[test]
fn a_circle_and_an_arc_of_it_overlap_along_the_arc() {
    for c in cases() {
        let o = [0.0, 0.0];
        let k = c.circle(o, 5.0);
        // N to S through W: half a turn, from N's angle.
        let a = c.arc(o, N, S);
        let ea = arc_ends(&a);
        let want = [m(
            span(ea[1], c.angle(N) + 0.5, 1.0),
            ea[0],
            c.angle(N),
            0.0,
        )];
        check(&c, &meet(&k, &a, c.tol()), &want, ACC, [false; 2]);
        let want = [m(
            span(ea[1], 1.0, c.angle(N) + 0.5),
            ea[0],
            0.0,
            c.angle(N),
        )];
        check(&c, &meet(&a, &k, c.tol()), &want, ACC, [false; 2]);
        // W round through S, E to N: three quarters of a turn from W's angle, past 1 when it wraps.
        let wrap = c.arc(o, W, N);
        let ew = arc_ends(&wrap);
        let want = [m(
            span(ew[1], c.angle(W) + 0.75, 1.0),
            ew[0],
            c.angle(W),
            0.0,
        )];
        check(&c, &meet(&k, &wrap, c.tol()), &want, ACC, [false; 2]);
        // A circle of another radius meets none of it.
        let other = c.circle(o, 4.0);
        assert!(meet(&other, &a, c.tol()).is_empty(), "{c:?}");
    }
}

#[test]
fn arcs_end_to_end_touch_or_miss() {
    for c in cases() {
        let o = [0.0, 0.0];
        let a = c.arc(o, E, N);
        let ea = arc_ends(&a);
        // The second starts at N, a little past it (a gap) or before it (an
        // overlap shorter than the tolerance): a touch at the first's end.
        for dx in [-0.5 * REL, 0.0, 0.5 * REL] {
            let b = c.arc(o, [dx, 5.0], W);
            let got = meet(&a, &b, c.tol());
            assert_eq!(got.len(), 1, "{c:?} {dx}");
            assert_eq!(got[0].kind, Kind::Touch, "{c:?} {dx}");
            assert_eq!(
                (got[0].at, got[0].on_first, got[0].on_second),
                (ea[1], 1.0, 0.0),
                "{c:?} {dx}"
            );
        }
        // Twice the tolerance apart: none; twice overlapping: an overlap.
        let b = c.arc(o, [-2.0 * REL, 5.0], W);
        assert!(meet(&a, &b, c.tol()).is_empty(), "{c:?}");
        let b = c.arc(o, [2.0 * REL, 5.0], W);
        let got = meet(&a, &b, c.tol());
        assert_eq!(got.len(), 1, "{c:?}");
        assert!(matches!(got[0].kind, Kind::Overlap { .. }), "{c:?}");
    }
}

#[test]
fn a_degenerate_arc_meets_nothing() {
    for c in cases() {
        let line = c.line([-5.0, 3.0], [7.0, 3.0]);
        let k = c.circle([0.0, 0.0], 5.0);
        for dot in [
            c.arc([0.0, 0.0], [0.0, 0.0], [5.0, 0.0]),
            c.arc([3.0, 3.0], [3.0 + 0.5 * REL, 3.0], [3.0, 4.0]),
        ] {
            for other in [&line, &k] {
                assert!(meet(&dot, other, c.tol()).is_empty(), "{c:?}");
                assert!(meet(other, &dot, c.tol()).is_empty(), "{c:?}");
            }
        }
    }
}
