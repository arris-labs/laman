//! The geometry of a loop (ADR-0012 D3): its signed area, its orientation,
//! and the winding number of a point about it.
//!
//! Every measure reads the loop's coordinates translated to its first join
//! and scaled by exact powers of two to an extent near 1 ([`Frame`]), so no
//! product of two coordinates overflows or underflows at any finite input,
//! and orientation survives an area that does not fit in `f64`.
//!
//! Area is Green's theorem, `½∮(x dy − y dx)`, summed edge by edge in loop
//! order: a line gives `cross(from, to)`, an arc its chord's cross product
//! plus the circular segment between chord and arc, `r²(θ − sin θ)`, with
//! `θ` its sweep (DATA-MODEL.md §Entities). The winding number sums each
//! edge's subtended angle (`libm::atan2`); an arc's is its chord's, plus a
//! whole turn when the point lies between chord and arc.
//!
//! Inputs are finite (the extraction refuses anything else first).

use std::f64::consts::TAU;

/// An edge with its coordinates, in the direction its loop traverses it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Edge {
    /// A line, traversed from `from` to `to`.
    Line { from: [f64; 2], to: [f64; 2] },
    /// An arc running counter-clockwise from `start` to `end` about
    /// `centre` (DATA-MODEL.md §Entities); its loop traverses it from `end`
    /// to `start` when `reversed`.
    Arc {
        centre: [f64; 2],
        start: [f64; 2],
        end: [f64; 2],
        reversed: bool,
    },
}

impl Edge {
    /// Where the traversal enters the edge.
    fn first(&self) -> [f64; 2] {
        match *self {
            Edge::Line { from, .. } => from,
            Edge::Arc {
                start,
                end,
                reversed,
                ..
            } => {
                if reversed {
                    end
                } else {
                    start
                }
            }
        }
    }
}

/// A closed boundary: a loop of edges, each ending where the next begins,
/// or a circle, counter-clockwise unless `reversed`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Boundary<'a> {
    Edges(&'a [Edge]),
    Circle {
        centre: [f64; 2],
        radius: f64,
        reversed: bool,
    },
}

/// The sense a boundary turns in: the sign of its area at the scale it is
/// measured at, so it is known even when the area under- or overflows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Orientation {
    CounterClockwise,
    Clockwise,
    /// Zero area: no sense to report.
    Flat,
}

/// A boundary's signed area, positive counter-clockwise, in the caller's
/// units squared, and its orientation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Area {
    /// Saturates at `±f64::MAX`; may underflow to zero, keeping its sign in
    /// `orientation`.
    pub signed: f64,
    pub orientation: Orientation,
    /// The area unsaturated: `twice · 2^exponent`, with `twice` of an extent
    /// near 1, so areas can be summed ([`net`]) where each alone overflows.
    pub twice: f64,
    pub exponent: i32,
}

/// The sum of areas given as `(twice, exponent)`, saturated once at
/// `±f64::MAX`: summed in the frame of the largest exponent, so terms that
/// each overflow cancel as they should.
pub(crate) fn net(terms: &[(f64, i32)]) -> f64 {
    let Some(top) = terms.iter().filter(|t| t.0 != 0.0).map(|t| t.1).max() else {
        return 0.0;
    };
    let mut sum = 0.0;
    for &(twice, e) in terms {
        sum += libm::scalbn(twice, e - top);
    }
    saturating_scale(sum, top)
}

/// Below this sweep, in radians, `θ − sin θ` is summed from its Taylor
/// series ([`SEGMENT_SERIES`]); at and above it, computed directly.
///
/// Not a tolerance: the point where the direct form's cancellation stops
/// mattering. Measured against a double-double reference over `θ` from
/// `1e-8` to `2π` (`small_sweep_threshold_is_measured`): the direct form's
/// error grows as `6ε/θ²` below 1 (33 ulps at 0.25, 9 at 0.5), and stays
/// within 1.2 ulps from 1 up; the eight-term series is within 1.6 ulps
/// everywhere below 1, its truncation `θ¹⁸/19!·6` under a fifth of an ulp.
const SMALL_SWEEP: f64 = 1.0;

/// `(θ − sin θ)/θ³` as a polynomial in `θ²`: the Taylor coefficients
/// `(−1)^k / (2k + 3)!`, eight terms, enough below [`SMALL_SWEEP`].
const SEGMENT_SERIES: [f64; 8] = [
    1.0 / 6.0,
    -1.0 / 120.0,
    1.0 / 5_040.0,
    -1.0 / 362_880.0,
    1.0 / 39_916_800.0,
    -1.0 / 6_227_020_800.0,
    1.0 / 1_307_674_368_000.0,
    -1.0 / 355_687_428_096_000.0,
];

/// `θ − sin θ`, twice the area of the circular segment of unit radius and
/// sweep `θ ∈ [0, 2π)`, to a couple of ulps.
fn segment(theta: f64) -> f64 {
    if theta < SMALL_SWEEP {
        let t2 = theta * theta;
        let mut acc = 0.0;
        for c in SEGMENT_SERIES.iter().rev() {
            acc = c + t2 * acc;
        }
        theta * t2 * acc
    } else {
        theta - libm::sin(theta)
    }
}

/// A boundary's coordinates, translated to its first join and scaled to an
/// extent in `[½, 1)` by exact powers of two.
///
/// Two stages, so that neither the translation nor a product overflows:
/// the points are first scaled so the largest coordinate is below 1
/// (`coarse`), then translated, then scaled again so the largest offset is
/// (`fine`). Scaling by a power of two is exact away from the subnormals,
/// so the measures are those of the caller's coordinates.
#[derive(Clone, Copy, Debug)]
struct Frame {
    coarse: i32,
    origin: [f64; 2],
    fine: i32,
}

impl Frame {
    /// The frame of `points`, translated to `origin`, with `radius` (a
    /// circle's, or 0) counted in its extent.
    fn new(points: &[[f64; 2]], origin: [f64; 2], radius: f64) -> Self {
        let largest = points
            .iter()
            .chain(std::iter::once(&origin))
            .flat_map(|p| p.iter())
            .fold(0.0_f64, |m, x| m.max(x.abs()));
        let coarse = exponent(largest);
        let origin = scale(origin, -coarse);
        let mut frame = Frame {
            coarse,
            origin,
            fine: 0,
        };
        let offset = points
            .iter()
            .map(|p| frame.point(*p))
            .flat_map(|p| p.into_iter())
            .fold(libm::scalbn(radius, -coarse), |m, x| m.max(x.abs()));
        frame.fine = exponent(offset);
        frame
    }

    /// `p` in the frame.
    fn point(&self, p: [f64; 2]) -> [f64; 2] {
        let p = scale(p, -self.coarse);
        scale([p[0] - self.origin[0], p[1] - self.origin[1]], -self.fine)
    }

    /// A point of the frame in the caller's coordinates.
    fn caller(&self, p: [f64; 2]) -> [f64; 2] {
        let p = scale(p, self.fine);
        scale([p[0] + self.origin[0], p[1] + self.origin[1]], self.coarse)
    }

    /// A caller's length in the frame.
    fn length(&self, l: f64) -> f64 {
        libm::scalbn(l, -(self.coarse + self.fine))
    }

    /// The power of two a frame length is multiplied by to be the caller's.
    fn exponent(&self) -> i32 {
        self.coarse + self.fine
    }
}

/// The `e` with `m = f·2^e`, `f ∈ [½, 1)`; 0 for 0 (or a non-finite `m`).
fn exponent(m: f64) -> i32 {
    if m > 0.0 && m.is_finite() {
        libm::frexp(m).1
    } else {
        0
    }
}

fn scale(p: [f64; 2], e: i32) -> [f64; 2] {
    [libm::scalbn(p[0], e), libm::scalbn(p[1], e)]
}

fn cross(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}

fn dot(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

fn sub(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}

/// An arc's sweep from `s` to `e` about `c`, in `[0, 2π)` (DATA-MODEL.md
/// §Entities), on frame coordinates.
fn sweep(c: [f64; 2], s: [f64; 2], e: [f64; 2]) -> f64 {
    let (us, ue) = (sub(s, c), sub(e, c));
    let theta = libm::atan2(cross(us, ue), dot(us, ue));
    if theta < 0.0 { theta + TAU } else { theta }
}

/// The frame a loop of edges is measured in: its points, from its first join.
fn edges_frame(edges: &[Edge]) -> Frame {
    let mut points = Vec::with_capacity(3 * edges.len());
    for edge in edges {
        match *edge {
            Edge::Line { from, to } => points.extend([from, to]),
            Edge::Arc {
                centre, start, end, ..
            } => points.extend([centre, start, end]),
        }
    }
    let origin = edges.first().map_or([0.0, 0.0], Edge::first);
    Frame::new(&points, origin, 0.0)
}

/// `x · 2^e`, saturating at `±f64::MAX`.
fn saturating_scale(x: f64, e: i32) -> f64 {
    let y = libm::scalbn(x, e);
    if y.is_infinite() {
        f64::MAX.copysign(y)
    } else {
        y
    }
}

fn orientation(twice: f64) -> Orientation {
    if twice > 0.0 {
        Orientation::CounterClockwise
    } else if twice < 0.0 {
        Orientation::Clockwise
    } else {
        Orientation::Flat
    }
}

/// The boundary's signed area and orientation (ADR-0012 D3).
pub(crate) fn area(boundary: &Boundary<'_>) -> Area {
    match *boundary {
        Boundary::Edges(edges) => {
            let frame = edges_frame(edges);
            // Twice the area, in the frame, summed in loop order.
            let mut twice = 0.0;
            for edge in edges {
                twice += match *edge {
                    Edge::Line { from, to } => cross(frame.point(from), frame.point(to)),
                    Edge::Arc {
                        centre,
                        start,
                        end,
                        reversed,
                    } => {
                        let (c, s, e) = (frame.point(centre), frame.point(start), frame.point(end));
                        let r = sub(s, c);
                        let ccw = cross(s, e) + dot(r, r) * segment(sweep(c, s, e));
                        if reversed { -ccw } else { ccw }
                    }
                };
            }
            Area {
                signed: saturating_scale(twice, 2 * frame.exponent() - 1),
                orientation: orientation(twice),
                twice,
                exponent: 2 * frame.exponent() - 1,
            }
        }
        Boundary::Circle {
            radius, reversed, ..
        } => {
            let (f, e) = libm::frexp(radius);
            let twice = if reversed { -TAU * f * f } else { TAU * f * f };
            Area {
                signed: saturating_scale(twice, 2 * e - 1),
                orientation: orientation(twice),
                twice,
                exponent: 2 * e - 1,
            }
        }
    }
}

/// The angle `[from, to]` subtends at the origin, in `(−π, π]`, and the
/// cross product its sign comes from: `cross(from, to) < 0` exactly when the
/// origin lies right of the chord `from → to`. A zero cross product counts
/// as `+0`, so a point on the chord's line reads `+π` and is on its left.
fn subtended(from: [f64; 2], to: [f64; 2]) -> (f64, f64) {
    let cr = cross(from, to) + 0.0;
    (libm::atan2(cr, dot(from, to)), cr)
}

/// The winding number of the boundary about `q`: the turns it makes around
/// `q`, positive counter-clockwise; 0 outside it. Undefined, but finite and
/// deterministic, for a `q` on the boundary.
pub(crate) fn winding(boundary: &Boundary<'_>, q: [f64; 2]) -> i64 {
    match *boundary {
        Boundary::Edges(edges) => {
            let frame = edges_frame(edges);
            let q = frame.point(q);
            // Outside the edges' bounding box (an arc's being its circle's)
            // the winding is 0, and a `q` too far to fit the frame is there.
            let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
            let mut grow = |p: [f64; 2], r: f64| {
                for i in 0..2 {
                    lo[i] = lo[i].min(p[i] - r);
                    hi[i] = hi[i].max(p[i] + r);
                }
            };
            for edge in edges {
                match *edge {
                    Edge::Line { from, to } => {
                        grow(frame.point(from), 0.0);
                        grow(frame.point(to), 0.0);
                    }
                    Edge::Arc {
                        centre, start, end, ..
                    } => {
                        let (c, s, e) = (frame.point(centre), frame.point(start), frame.point(end));
                        let (rs, re) = (sub(s, c), sub(e, c));
                        grow(c, dot(rs, rs).max(dot(re, re)).sqrt());
                    }
                }
            }
            let inside_box = (0..2).all(|i| q[i] >= lo[i] && q[i] <= hi[i]);
            if !inside_box {
                return 0;
            }
            let mut turned = 0.0;
            for edge in edges {
                turned += match *edge {
                    Edge::Line { from, to } => {
                        subtended(sub(frame.point(from), q), sub(frame.point(to), q)).0
                    }
                    Edge::Arc {
                        centre,
                        start,
                        end,
                        reversed,
                    } => {
                        let (c, s, e) = (frame.point(centre), frame.point(start), frame.point(end));
                        let (chord, cr) = subtended(sub(s, q), sub(e, q));
                        // Right of the chord and inside the circle: between
                        // chord and arc, which goes once round `q`.
                        let (r, d) = (sub(s, c), sub(q, c));
                        let ccw = if cr < 0.0 && dot(d, d) < dot(r, r) {
                            chord + TAU
                        } else {
                            chord
                        };
                        if reversed { -ccw } else { ccw }
                    }
                };
            }
            // Each angle is within a few ulps; the sum is a multiple of 2π.
            libm::round(turned / TAU) as i64
        }
        Boundary::Circle {
            centre,
            radius,
            reversed,
        } => {
            let frame = Frame::new(&[centre], centre, radius);
            let (d, r) = (frame.point(q), frame.length(radius));
            if dot(d, d) < r * r {
                if reversed { -1 } else { 1 }
            } else {
                0
            }
        }
    }
}

/// A point on the boundary away from its joins: its first edge's midpoint
/// (an arc's at half its sweep), or a circle's point at angle 0. `None` for
/// a loop without edges. Where containment is tested (ADR-0012 D3).
pub(crate) fn probe(boundary: &Boundary<'_>) -> Option<[f64; 2]> {
    match *boundary {
        Boundary::Edges(edges) => {
            let first = edges.first()?;
            let frame = edges_frame(std::slice::from_ref(first));
            let p = match *first {
                Edge::Line { from, to } => {
                    let (a, b) = (frame.point(from), frame.point(to));
                    [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0]
                }
                Edge::Arc {
                    centre, start, end, ..
                } => {
                    let (c, s, e) = (frame.point(centre), frame.point(start), frame.point(end));
                    let (sin, cos) = libm::sincos(sweep(c, s, e) / 2.0);
                    let r = sub(s, c);
                    [
                        c[0] + cos * r[0] - sin * r[1],
                        c[1] + sin * r[0] + cos * r[1],
                    ]
                }
            };
            Some(frame.caller(p))
        }
        Boundary::Circle { centre, radius, .. } => {
            let frame = Frame::new(&[centre], centre, radius);
            Some(frame.caller([frame.length(radius), 0.0]))
        }
    }
}

#[cfg(test)]
mod tests;
