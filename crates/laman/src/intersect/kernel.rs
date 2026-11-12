//! The meetings of one pair of curves (ADR-0016 D3, D4, D7, D8): lines and
//! circles, each pair measured in its own frame and classified at a length
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
//! A line's root counts when it lies within `L` of the segment, along it.
//! One within `L` of a line's end is at that end (ADR-0016 D4): its
//! parameter 0 or 1, its point the caller's own end (the first curve's when
//! both have an end there). A line no longer than `L`, or a circle whose
//! radius is within it, meets nothing (D1).
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
    /// A line's `t` in [0, 1] from `a`; a circle's angle from +x over 2π,
    /// in [0, 1).
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
        (Local::Line(l), Local::Circle(k)) | (Local::Circle(k), Local::Line(l)) => {
            line_circle(l, k, lt)
        }
        (Local::Circle(k), Local::Circle(n)) => circle_circle(k, n, lt),
    };
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
        }
    }
    let origin = match *first {
        Curve::Line { a, .. } => a,
        Curve::Circle { centre, .. } => centre,
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

#[derive(Clone, Copy, Debug)]
enum Local {
    Line(Line),
    Circle(Circle),
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
        }
    }

    /// A line no longer than `lt`, or a circle whose radius is within it,
    /// has no direction to classify by (ADR-0016 D1).
    fn degenerate(&self, lt: f64) -> bool {
        match self {
            Local::Line(l) => l.len <= lt,
            Local::Circle(k) => k.r <= lt,
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
    let end = |curve: &Local| match curve {
        Local::Line(l) => l.end(p, lt),
        Local::Circle(_) => None,
    };
    let (e1, e2) = (end(first), end(second));
    let (fp, at) = match (first, e1, second, e2) {
        (Local::Line(l), Some(i), _, _) | (_, None, Local::Line(l), Some(i)) => {
            (if i == 0 { l.a } else { l.b }, l.ends[i])
        }
        _ => (p, frame.caller(p)),
    };
    let param = |curve: &Local, e: Option<usize>| match (curve, e) {
        (Local::Line(_), Some(i)) => i as f64,
        (Local::Line(l), None) => (l.along(fp) / l.len).clamp(0.0, 1.0),
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
