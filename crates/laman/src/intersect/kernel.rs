//! The meetings of one pair of curves (ADR-0016 D3, D4, D6, D7, D8): lines,
//! circles and arcs, each pair measured in its own frame and classified at a length
//! tolerance the caller passes.
//!
//! A pair is measured in the frame of ADR-0012 D3 ([`Frame`]): translated
//! to the first curve's origin (a line's `a`, a circle's centre) and scaled
//! by exact powers of two to an extent near 1, the tolerance with it, so
//! nothing overflows near 1e±300 and a small pair far from the origin keeps
//! its digits. The tolerance is an input, not `Tolerances::at`, so the
//! closed forms hold at 1e-300 when the caller passes one at that scale
//! (ADR-0016 D2).
//!
//! With `L` the tolerance and `d` the distance from a centre to a line's
//! carrier, or between two centres:
//!
//! - two lines meet once where their carriers cross, a **crossing**, unless
//!   they are collinear (the shorter's ends within `L` of the longer's
//!   carrier): then their shared stretch is an **overlap** when longer than
//!   `L`, a **touch** when its ends are within `L`, and nothing beyond;
//! - a line and a circle **touch** at the foot of the perpendicular from the
//!   centre when `|d − r| ≤ L`, and **cross** twice when `d < r − L`;
//! - two circles **touch** on the line of centres when `d` is within `L` of
//!   `r₁ + r₂` or `|r₁ − r₂|`, **cross** twice strictly between, and
//!   **overlap** all the way round when concentric (`d ≤ L`) with radii
//!   within `L`.
//!
//! A line's root counts when it lies within `L` of the segment, along it;
//! an arc is measured on its circle, and a root of the circle counts when
//! its angle lies in the sweep widened at each end by `L / r` (a widened
//! sweep of a full turn is the whole circle, D6). One within `L` of a line's
//! or an arc's end is at that end (ADR-0016 D4): its parameter 0 or 1, its
//! point the caller's own end (the first curve's when both have an end
//! there). A line no longer than `L`, or a round whose radius is within it,
//! meets nothing (D1).
//!
//! Two rounds with `d ≤ L` and radii within `L` share their sweeps' common
//! part, by the same reading as two lines: a stretch longer than `L` is an
//! **overlap**, one of `L` or less a **touch** at its middle, a gap beyond
//! `L` nothing. Two arcs can share two stretches (each of their sweeps
//! reaching round to the other's start). A circle and an arc share the whole
//! arc: one stretch, its parameters on the circle running past 1 when it
//! crosses angle 0.
//!
//! Square roots of differences are taken as products, `√((r − d)(r + d))`;
//! angles through `libm::atan2`. Inputs are finite and `L ≥ 0`; the caller
//! (`intersections()`) refuses anything else first.

use crate::frame::{Frame, cross, dot, sub};
use std::f64::consts::TAU;

/// A curve as the kernel reads it, in the caller's coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Curve {
    /// A line from `a` to `b`.
    Line { a: [f64; 2], b: [f64; 2] },
    /// A full circle.
    Circle { centre: [f64; 2], radius: f64 },
    /// The counter-clockwise arc from `start` to `end` about `centre`, of
    /// radius `|start − centre|` (ADR-0001); `end` only gives the direction
    /// the sweep stops at.
    Arc {
        centre: [f64; 2],
        start: [f64; 2],
        end: [f64; 2],
    },
}

/// How two curves meet (ADR-0016 D3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Kind {
    /// A simple root.
    Crossing,
    /// Tangent within the tolerance: one point.
    Touch,
    /// A shared stretch, from the meeting's `at` to `to`, with `to`'s
    /// parameters on both curves.
    Overlap {
        to: [f64; 2],
        to_first: f64,
        to_second: f64,
    },
}

/// One place where two curves meet (ADR-0016 D7).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Meeting {
    pub kind: Kind,
    /// In the caller's coordinates; a curve's own end when the meeting is at
    /// one.
    pub at: [f64; 2],
    /// A line's `t` in [0, 1] from `a`; an arc's fraction of its sweep from
    /// its start, in [0, 1]; a circle's angle from +x over 2π, in [0, 1).
    pub on_first: f64,
    pub on_second: f64,
}

/// The meetings of `first` and `second` at the length tolerance `length`,
/// ordered by their parameter on `first`, ties by `second`'s.
pub(crate) fn meet(first: &Curve, second: &Curve, length: f64) -> Vec<Meeting> {
    let frame = frame(first, second);
    let lt = frame.length(length);
    let (p, q) = (Local::new(first, &frame), Local::new(second, &frame));
    if p.degenerate(lt) || q.degenerate(lt) {
        return Vec::new();
    }
    let raws = match (&p, &q) {
        (Local::Line(l), Local::Line(m)) => line_line(l, m, lt),
        (Local::Line(l), round) | (round, Local::Line(l)) => match round.circle() {
            Some(k) => line_circle(l, k, lt),
            None => Vec::new(),
        },
        (a, b) => match (a.circle(), b.circle()) {
            (Some(k), Some(n)) => round_round(&p, &q, k, n, lt),
            _ => Vec::new(),
        },
    };
    let raws: Vec<Raw> = raws
        .into_iter()
        .filter(|raw| match raw {
            Raw::Point(_, at) => [&p, &q].iter().all(|c| c.holds(*at, lt)),
            Raw::Stretch(..) | Raw::Round(_) | Raw::Span { .. } => true,
        })
        .collect();
    let mut meetings: Vec<Meeting> = raws
        .into_iter()
        .map(|raw| match raw {
            Raw::Point(kind, at) => {
                let (at, on_first, on_second) = place(at, &p, &q, lt, &frame);
                let kind = match kind {
                    PointKind::Crossing => Kind::Crossing,
                    PointKind::Touch => Kind::Touch,
                };
                Meeting {
                    kind,
                    at,
                    on_first,
                    on_second,
                }
            }
            Raw::Stretch(from, to) => {
                let mut ends = [
                    place(from, &p, &q, lt, &frame),
                    place(to, &p, &q, lt, &frame),
                ];
                if ends[1].1.total_cmp(&ends[0].1).is_lt() {
                    ends.swap(0, 1);
                }
                let [(at, on_first, on_second), (to, to_first, to_second)] = ends;
                Meeting {
                    kind: Kind::Overlap {
                        to,
                        to_first,
                        to_second,
                    },
                    at,
                    on_first,
                    on_second,
                }
            }
            Raw::Span {
                at,
                to,
                first,
                second,
            } => Meeting {
                kind: Kind::Overlap {
                    to,
                    to_first: first[1],
                    to_second: second[1],
                },
                at,
                on_first: first[0],
                on_second: second[0],
            },
            Raw::Round(at) => {
                let at = frame.caller(at);
                Meeting {
                    kind: Kind::Overlap {
                        to: at,
                        to_first: 1.0,
                        to_second: 1.0,
                    },
                    at,
                    on_first: 0.0,
                    on_second: 0.0,
                }
            }
        })
        .collect();
    meetings.sort_by(|x, y| {
        x.on_first
            .total_cmp(&y.on_first)
            .then(x.on_second.total_cmp(&y.on_second))
    });
    meetings
}

/// The pair's frame: every point and the larger radius in its extent,
/// translated to the first curve's origin (ADR-0016 D8).
fn frame(first: &Curve, second: &Curve) -> Frame {
    let mut points = Vec::with_capacity(4);
    let mut radius = 0.0_f64;
    for curve in [first, second] {
        match *curve {
            Curve::Line { a, b } => points.extend([a, b]),
            Curve::Circle { centre, radius: r } => {
                points.push(centre);
                radius = radius.max(r.abs());
            }
            Curve::Arc { centre, start, end } => points.extend([centre, start, end]),
        }
    }
    let origin = match *first {
        Curve::Line { a, .. } => a,
        Curve::Circle { centre, .. } | Curve::Arc { centre, .. } => centre,
    };
    Frame::new(&points, origin, radius)
}

/// A line in the frame, with the caller's ends kept for D4.
#[derive(Clone, Copy, Debug)]
struct Line {
    a: [f64; 2],
    b: [f64; 2],
    /// `b − a`.
    u: [f64; 2],
    len: f64,
    /// The caller's `a` and `b`.
    ends: [[f64; 2]; 2],
}

impl Line {
    /// The point at distance `s` from `a` along the line.
    fn at(&self, s: f64) -> [f64; 2] {
        let f = s / self.len;
        [self.a[0] + f * self.u[0], self.a[1] + f * self.u[1]]
    }

    /// The distance from `a` along the line of `p`'s projection.
    fn along(&self, p: [f64; 2]) -> f64 {
        dot(sub(p, self.a), self.u) / self.len
    }

    /// Whether the distance `s` along the carrier is within `lt` of the
    /// segment.
    fn holds(&self, s: f64, lt: f64) -> bool {
        s >= -lt && s <= self.len + lt
    }

    /// The end (0 for `a`, 1 for `b`) within `lt` of `p` along the line.
    fn end(&self, p: [f64; 2], lt: f64) -> Option<usize> {
        let s = self.along(p);
        if s <= self.len / 2.0 {
            (s <= lt).then_some(0)
        } else {
            (s >= self.len - lt).then_some(1)
        }
    }
}

/// A circle in the frame.
#[derive(Clone, Copy, Debug)]
struct Circle {
    c: [f64; 2],
    r: f64,
}

impl Circle {
    /// The angle of `p` about the centre from +x over 2π, in [0, 1).
    fn param(&self, p: [f64; 2]) -> f64 {
        let v = sub(p, self.c);
        let t = libm::atan2(v[1], v[0]) / TAU;
        let t = if t < 0.0 { t + 1.0 } else { t };
        // A tiny negative angle rounds to 1 when lifted.
        if t >= 1.0 { 0.0 } else { t }
    }
}

/// An arc in the frame: its circle, its sweep from the start direction
/// counter-clockwise, and the caller's ends kept for D4.
#[derive(Clone, Copy, Debug)]
struct Arc {
    k: Circle,
    /// The start and end in the frame.
    s: [f64; 2],
    e: [f64; 2],
    /// In [0, 2π).
    sweep: f64,
    ends: [[f64; 2]; 2],
}

impl Arc {
    /// The counter-clockwise angle from the start to `p`'s direction,
    /// folded to the nearer side of the gap: in (−(2π − sweep)/2, …) behind
    /// the start, past the end up to the middle of the gap.
    fn delta(&self, p: [f64; 2]) -> f64 {
        let (us, v) = (sub(self.s, self.k.c), sub(p, self.k.c));
        let t = libm::atan2(cross(us, v), dot(us, v));
        let t = if t < 0.0 { t + TAU } else { t };
        let t = if t >= TAU { 0.0 } else { t };
        if t > (self.sweep + TAU) / 2.0 {
            t - TAU
        } else {
            t
        }
    }

    /// Whether `p`'s direction is in the sweep widened by `lt / r` at each
    /// end (ADR-0016 D6).
    fn holds(&self, p: [f64; 2], lt: f64) -> bool {
        let w = lt / self.k.r;
        if self.sweep + 2.0 * w >= TAU {
            return true;
        }
        let d = self.delta(p);
        d >= -w && d <= self.sweep + w
    }

    /// The end (0 for the start, 1 for the end) within `lt` of `p` along the
    /// arc.
    fn end(&self, p: [f64; 2], lt: f64) -> Option<usize> {
        let d = self.delta(p);
        if d <= self.sweep / 2.0 {
            (self.k.r * d <= lt).then_some(0)
        } else {
            (self.k.r * (self.sweep - d) <= lt).then_some(1)
        }
    }

    /// `p`'s fraction of the sweep, in [0, 1].
    fn fraction(&self, p: [f64; 2]) -> f64 {
        if self.sweep > 0.0 {
            (self.delta(p) / self.sweep).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// The point at angle `psi` counter-clockwise from the start, at the
    /// radius.
    fn at(&self, psi: f64) -> [f64; 2] {
        let us = sub(self.s, self.k.c);
        let len = libm::hypot(us[0], us[1]);
        let (c, s) = (libm::cos(psi), libm::sin(psi));
        let u = [us[0] / len, us[1] / len];
        [
            self.k.c[0] + self.k.r * (c * u[0] - s * u[1]),
            self.k.c[1] + self.k.r * (s * u[0] + c * u[1]),
        ]
    }
}

#[derive(Clone, Copy, Debug)]
enum Local {
    Line(Line),
    Circle(Circle),
    Arc(Arc),
}

impl Local {
    fn new(curve: &Curve, frame: &Frame) -> Self {
        match *curve {
            Curve::Line { a, b } => {
                let (fa, fb) = (frame.point(a), frame.point(b));
                let u = sub(fb, fa);
                Local::Line(Line {
                    a: fa,
                    b: fb,
                    u,
                    len: libm::hypot(u[0], u[1]),
                    ends: [a, b],
                })
            }
            Curve::Circle { centre, radius } => Local::Circle(Circle {
                c: frame.point(centre),
                r: frame.length(radius),
            }),
            Curve::Arc { centre, start, end } => {
                let (c, s, e) = (frame.point(centre), frame.point(start), frame.point(end));
                let (us, ue) = (sub(s, c), sub(e, c));
                let t = libm::atan2(cross(us, ue), dot(us, ue));
                let sweep = if t < 0.0 { t + TAU } else { t };
                Local::Arc(Arc {
                    k: Circle {
                        c,
                        r: libm::hypot(us[0], us[1]),
                    },
                    s,
                    e,
                    sweep: if sweep >= TAU { 0.0 } else { sweep },
                    ends: [start, end],
                })
            }
        }
    }

    /// The circle a round is measured on.
    fn circle(&self) -> Option<&Circle> {
        match self {
            Local::Line(_) => None,
            Local::Circle(k) => Some(k),
            Local::Arc(a) => Some(&a.k),
        }
    }

    /// Whether the frame point `p`, on the curve's carrier, is on the curve
    /// (ADR-0016 D6; a line's was tested with its root).
    fn holds(&self, p: [f64; 2], lt: f64) -> bool {
        match self {
            Local::Line(_) | Local::Circle(_) => true,
            Local::Arc(a) => a.holds(p, lt),
        }
    }

    /// The end (0 or 1) of a line or an arc within `lt` of `p`.
    fn end(&self, p: [f64; 2], lt: f64) -> Option<usize> {
        match self {
            Local::Line(l) => l.end(p, lt),
            Local::Arc(a) => a.end(p, lt),
            Local::Circle(_) => None,
        }
    }

    /// End `i`: in the frame, and the caller's own.
    fn own(&self, i: usize) -> Option<([f64; 2], [f64; 2])> {
        match self {
            Local::Line(l) => Some((if i == 0 { l.a } else { l.b }, l.ends[i])),
            Local::Arc(a) => Some((if i == 0 { a.s } else { a.e }, a.ends[i])),
            Local::Circle(_) => None,
        }
    }

    /// A line no longer than `lt`, or a circle whose radius is within it,
    /// has no direction to classify by (ADR-0016 D1).
    fn degenerate(&self, lt: f64) -> bool {
        match self {
            Local::Line(l) => l.len <= lt,
            Local::Circle(k) => k.r <= lt,
            Local::Arc(a) => a.k.r <= lt,
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum PointKind {
    Crossing,
    Touch,
}

/// A meeting in the frame, before its parameters and D4.
#[derive(Clone, Copy, Debug)]
enum Raw {
    Point(PointKind, [f64; 2]),
    /// A shared stretch of two lines, by its two ends.
    Stretch([f64; 2], [f64; 2]),
    /// Two coincident circles, from the first's point at angle 0.
    Round([f64; 2]),
    /// A stretch shared by two rounds, in the caller's coordinates, with
    /// its parameters `[from, to]` on each curve.
    Span {
        at: [f64; 2],
        to: [f64; 2],
        first: [f64; 2],
        second: [f64; 2],
    },
}

/// The meeting at frame point `p`: its point in the caller's coordinates
/// and its parameters on `first` and `second`. A line end within `lt` of
/// `p` takes the meeting (ADR-0016 D4), the first curve's before the
/// second's.
fn place(
    p: [f64; 2],
    first: &Local,
    second: &Local,
    lt: f64,
    frame: &Frame,
) -> ([f64; 2], f64, f64) {
    let (e1, e2) = (first.end(p, lt), second.end(p, lt));
    let own = match (e1, e2) {
        (Some(i), _) => first.own(i),
        (None, Some(i)) => second.own(i),
        (None, None) => None,
    };
    let (fp, at) = own.unwrap_or((p, frame.caller(p)));
    let param = |curve: &Local, e: Option<usize>| match (curve, e) {
        (Local::Line(_) | Local::Arc(_), Some(i)) => i as f64,
        (Local::Line(l), None) => (l.along(fp) / l.len).clamp(0.0, 1.0),
        (Local::Arc(a), None) => a.fraction(fp),
        (Local::Circle(k), _) => k.param(fp),
    };
    (at, param(first, e1), param(second, e2))
}

fn line_line(p: &Line, q: &Line, lt: f64) -> Vec<Raw> {
    // The shorter one's ends (ties: the second's) tested against the
    // longer one's carrier.
    let (long, short) = if p.len < q.len { (q, p) } else { (p, q) };
    let off = |e: [f64; 2]| (cross(long.u, sub(e, long.a)) / long.len).abs();
    if off(short.a) <= lt && off(short.b) <= lt {
        let (s0, s1) = (long.along(short.a), long.along(short.b));
        let (lo_end, lo) = if s0 <= s1 {
            (short.a, s0)
        } else {
            (short.b, s1)
        };
        let (hi_end, hi) = if s0 <= s1 {
            (short.b, s1)
        } else {
            (short.a, s0)
        };
        let (from, from_s) = if lo >= 0.0 {
            (lo_end, lo)
        } else {
            (long.a, 0.0)
        };
        let (to, to_s) = if hi <= long.len {
            (hi_end, hi)
        } else {
            (long.b, long.len)
        };
        let shared = to_s - from_s;
        return if shared > lt {
            vec![Raw::Stretch(from, to)]
        } else if shared >= -lt {
            vec![Raw::Point(PointKind::Touch, long.at((from_s + to_s) / 2.0))]
        } else {
            Vec::new()
        };
    }
    let denom = cross(p.u, q.u);
    if denom == 0.0 {
        return Vec::new();
    }
    let w = sub(q.a, p.a);
    let (t, s) = (cross(w, q.u) / denom, cross(w, p.u) / denom);
    if p.holds(t * p.len, lt) && q.holds(s * q.len, lt) {
        vec![Raw::Point(PointKind::Crossing, p.at(t * p.len))]
    } else {
        Vec::new()
    }
}

fn line_circle(l: &Line, k: &Circle, lt: f64) -> Vec<Raw> {
    let w = sub(k.c, l.a);
    let along = dot(w, l.u) / l.len;
    let d = (cross(l.u, w) / l.len).abs();
    if (d - k.r).abs() <= lt {
        return if l.holds(along, lt) {
            vec![Raw::Point(PointKind::Touch, l.at(along))]
        } else {
            Vec::new()
        };
    }
    if d >= k.r - lt {
        return Vec::new();
    }
    let h = ((k.r - d) * (k.r + d)).sqrt();
    [along - h, along + h]
        .into_iter()
        .filter(|s| l.holds(*s, lt))
        .map(|s| Raw::Point(PointKind::Crossing, l.at(s)))
        .collect()
}

/// Two rounds, at least one an arc, on one circle (`d ≤ L`, radii within
/// `L`): their sweeps' shared part. Otherwise as two circles.
fn round_round(first: &Local, second: &Local, k: &Circle, n: &Circle, lt: f64) -> Vec<Raw> {
    let v = sub(n.c, k.c);
    let concentric = libm::hypot(v[0], v[1]) <= lt && (k.r - n.r).abs() <= lt;
    match (first, second) {
        (Local::Arc(a), Local::Arc(b)) if concentric => arc_arc(a, b, lt),
        (Local::Circle(_), Local::Arc(b)) if concentric => {
            // The whole arc, on the circle from its start's angle.
            let on = Circle::param(k, b.s);
            vec![Raw::Span {
                at: b.ends[0],
                to: b.ends[1],
                first: [on, on + b.sweep / TAU],
                second: [0.0, 1.0],
            }]
        }
        (Local::Arc(a), Local::Circle(_)) if concentric => {
            let on = Circle::param(n, a.s);
            vec![Raw::Span {
                at: a.ends[0],
                to: a.ends[1],
                first: [0.0, 1.0],
                second: [on, on + a.sweep / TAU],
            }]
        }
        _ => circle_circle(k, n, lt),
    }
}

/// The sweeps of two concentric arcs of one radius, shared: `b`'s sweep
/// starts `beta` counter-clockwise of `a`'s, and again a turn earlier, so
/// each copy is clipped to `a`'s. A stretch longer than `lt` is an overlap
/// between the ends that clip it, one of `lt` or less a touch at its middle.
fn arc_arc(a: &Arc, b: &Arc, lt: f64) -> Vec<Raw> {
    let (us, ub) = (sub(a.s, a.k.c), sub(b.s, b.k.c));
    let t = libm::atan2(cross(us, ub), dot(us, ub));
    let beta = if t < 0.0 { t + TAU } else { t };
    let frac = |x: f64, sweep: f64| {
        if sweep > 0.0 {
            (x / sweep).clamp(0.0, 1.0)
        } else {
            0.0
        }
    };
    let mut out = Vec::new();
    for k in [beta, beta - TAU] {
        let (lo, hi) = (k.max(0.0), (k + b.sweep).min(a.sweep));
        let shared = a.k.r * (hi - lo);
        if shared > lt {
            let at = if k > 0.0 { b.ends[0] } else { a.ends[0] };
            let to = if k + b.sweep < a.sweep {
                b.ends[1]
            } else {
                a.ends[1]
            };
            out.push(Raw::Span {
                at,
                to,
                first: [frac(lo, a.sweep), frac(hi, a.sweep)],
                second: [frac(lo - k, b.sweep), frac(hi - k, b.sweep)],
            });
        } else if shared >= -lt {
            out.push(Raw::Point(PointKind::Touch, a.at((lo + hi) / 2.0)));
        }
    }
    out
}

fn circle_circle(k: &Circle, n: &Circle, lt: f64) -> Vec<Raw> {
    let v = sub(n.c, k.c);
    let d = libm::hypot(v[0], v[1]);
    let (r1, r2) = (k.r, n.r);
    if d <= lt {
        return if (r1 - r2).abs() <= lt {
            vec![Raw::Round([k.c[0] + r1, k.c[1]])]
        } else {
            Vec::new()
        };
    }
    let dir = [v[0] / d, v[1] / d];
    let along = |s: f64| [k.c[0] + s * dir[0], k.c[1] + s * dir[1]];
    if (d - (r1 + r2)).abs() <= lt {
        return vec![Raw::Point(PointKind::Touch, along(r1))];
    }
    let gap = (r1 - r2).abs();
    if (d - gap).abs() <= lt {
        // The first lies inside the second when it is the smaller: their
        // contact is on its far side from the second's centre.
        let s = if r1 >= r2 { r1 } else { -r1 };
        return vec![Raw::Point(PointKind::Touch, along(s))];
    }
    if d <= gap + lt || d >= r1 + r2 - lt {
        return Vec::new();
    }
    // The chord's foot `a` from the first centre, and half the chord `h`,
    // from the four factors of Heron's form, none a cancelling difference
    // of squares.
    let a = (d + (r1 - r2) * (r1 + r2) / d) / 2.0;
    let h = (((r1 + r2 - d) * (d + r1 + r2)).max(0.0).sqrt()
        * ((d + r2 - r1) * (d + r1 - r2)).max(0.0).sqrt())
        / (2.0 * d);
    let foot = along(a);
    let perp = [-dir[1], dir[0]];
    [-h, h]
        .into_iter()
        .map(|s| {
            Raw::Point(
                PointKind::Crossing,
                [foot[0] + s * perp[0], foot[1] + s * perp[1]],
            )
        })
        .collect()
}

#[cfg(test)]
mod tests;
