//! The profile check's own measure of one loop (ADR-0013 D1, D2): its signed
//! area, perimeter, extent, join separations and arc radius mismatch, in the
//! loop's power-of-two frame.
//!
//! Written from DATA-MODEL.md, not from extraction: an arc is triangle,
//! sector, triangle about the loop's first point, `½ cross(s − o, c − o) +
//! ½ r²θ + ½ cross(c − o, e − o)`, exact for any origin, with no series and
//! no small-sweep branch. Every value is in the frame: the caller's
//! coordinates translated to the loop's first point and scaled by an exact
//! power of two to an extent near 1, arc centres and a circle's radius
//! counted in that extent. A caller's length or area is brought into the
//! frame ([`Measured::length`], [`Measured::area_of`]), never the other way,
//! so nothing overflows or underflows at any finite input.
//!
//! **The rounding term** (ADR-0013 D2, settled by `plans/m10-profile-check`
//! step 2). `L·P` alone does not cover the measure's rounding. `L` is
//! relative to a component's scale (ADR-0003), and a loop no equation
//! reads has none: each edge is measured alone, so `L` is its longest
//! edge's, not the loop's. The sector form's cancellation on a
//! near-straight arc stays inside it, since the arc's own extent counts its
//! centre: measured at 1.6e-6 of `L·P`, radii 1e3 to 1e15. A long loop
//! does not: each cross product rounds by `ε` of its two products, of order
//! `D²` however thin the triangle, so the error grows with the edge count
//! while `L·P` does not. Measured on a regular polygon of unconstrained
//! lines: 2.8e-3 of `L·P` at 500 edges, 2.2 at 50 000. So the allowance adds
//! [`ROUNDING`] times the sum's magnitude `M`: every product's size and
//! every partial sum's, as accumulated.
//!
//! Inputs are expected finite; the caller lists a non-finite coordinate as
//! missing first (ADR-0013 D3). A non-finite one still never panics: it
//! propagates to the values here, never to a decision.

use super::{Xy, cross, dist, sub};
use std::f64::consts::{PI, TAU};

/// The rounding allowed per unit of an area sum's magnitude (the sizes of
/// its products and partial sums), on top of `L·P + ½·D·Σg`.
///
/// A product, a difference and an addition each round by at most `ε/2` of
/// their size, and an arc's `atan2` and `hypot` by an ulp of theirs:
/// summed, at most about `ε` of the magnitude per measure to first order.
/// Twice that covers extraction's sum as well as the check's (its terms
/// are no larger: a chord's cross product against the two triangles
/// through the centre), and twice again the second-order terms and the
/// frames' differing exponents. Measured against closed forms by the
/// tests below: the worst error is 0.070 of this term (a near-straight arc
/// of radius 1e15), 1.8e-4 of it on the 50 000-edge polygon where `L·P`
/// alone is short by 2.2 times.
pub(crate) const ROUNDING: f64 = 4.0 * f64::EPSILON;

/// An edge of a loop with its coordinates in the caller's units, in the
/// direction the loop traverses it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Piece {
    /// A line, traversed from `from` to `to`.
    Line { from: Xy, to: Xy },
    /// An arc counter-clockwise from `s` to `e` about `c` (DATA-MODEL.md
    /// §Entities), traversed from `e` to `s` when `reversed`.
    Arc { c: Xy, s: Xy, e: Xy, reversed: bool },
}

impl Piece {
    /// Where the traversal enters the piece.
    fn entry(&self) -> Xy {
        match *self {
            Piece::Line { from, .. } => from,
            Piece::Arc { s, e, reversed, .. } => {
                if reversed {
                    e
                } else {
                    s
                }
            }
        }
    }

    /// Where the traversal leaves it.
    fn exit(&self) -> Xy {
        match *self {
            Piece::Line { to, .. } => to,
            Piece::Arc { s, e, reversed, .. } => {
                if reversed {
                    s
                } else {
                    e
                }
            }
        }
    }
}

/// A closed boundary to measure.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Shape<'a> {
    /// Pieces end to end, in loop order; the last one's exit joins the
    /// first one's entry.
    Pieces(&'a [Piece]),
    /// A circle of radius `r`, counter-clockwise unless `reversed`.
    Circle { r: f64, reversed: bool },
}

/// The loop's frame: a caller's point `p` is `(p·2^−coarse − origin)·2^−fine`.
///
/// Two stages, so that neither the translation nor a product overflows: the
/// coordinates are scaled first so the largest is below 1, translated, then
/// scaled again so the largest offset is in `[½, 1)`. A power-of-two scale
/// is exact away from the subnormals.
#[derive(Debug, Clone, Copy, PartialEq)]
struct LoopFrame {
    coarse: i32,
    origin: Xy,
    fine: i32,
}

/// The `k` with `|m| = f·2^k`, `f ∈ [½, 1)`; 0 for 0 or a non-finite `m`.
fn binary_exponent(m: f64) -> i32 {
    if m.is_finite() && m != 0.0 {
        libm::frexp(m).1
    } else {
        0
    }
}

fn scaled((x, y): Xy, k: i32) -> Xy {
    (libm::scalbn(x, k), libm::scalbn(y, k))
}

impl LoopFrame {
    /// The frame of `points` about `origin`, with `radius` (a circle's, or
    /// 0) counted in the offsets.
    fn new(points: &[Xy], origin: Xy, radius: f64) -> LoopFrame {
        let largest = points
            .iter()
            .chain([&origin])
            .fold(0.0_f64, |m, p| m.max(p.0.abs()).max(p.1.abs()));
        let coarse = binary_exponent(largest);
        let mut frame = LoopFrame {
            coarse,
            origin: scaled(origin, -coarse),
            fine: 0,
        };
        let offset = points.iter().fold(libm::scalbn(radius, -coarse), |m, p| {
            let q = frame.point(*p);
            m.max(q.0.abs()).max(q.1.abs())
        });
        frame.fine = binary_exponent(offset);
        frame
    }

    /// A caller's point in the frame.
    fn point(&self, p: Xy) -> Xy {
        scaled(sub(scaled(p, -self.coarse), self.origin), -self.fine)
    }

    /// The power of two a frame length is multiplied by to be the caller's.
    fn exponent(&self) -> i32 {
        self.coarse.saturating_add(self.fine)
    }
}

/// One loop as the profile check measures it. Every length and area is in
/// the loop's frame; [`Measured::length`] and [`Measured::area_of`] bring a
/// caller's into it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Measured {
    /// The signed area, positive counter-clockwise.
    pub area: f64,
    /// The boundary's length: lines, and each arc at its larger radius.
    pub perimeter: f64,
    /// The diagonal of the bounding box of every point the loop reads, arc
    /// centres included.
    pub extent: f64,
    /// Each join's separation: the distance from piece `i`'s exit to piece
    /// `i + 1`'s entry, the last one's to the first's. Empty for a circle.
    pub gaps: Vec<f64>,
    /// The largest `|r_s − r_e|` over the loop's arcs, 0 without one.
    pub off_radius: f64,
    /// The area sum's magnitude `M`, in area units: half the sizes of
    /// every product and partial sum the twice-area accumulates.
    pub magnitude: f64,
    frame: LoopFrame,
}

impl Measured {
    /// A caller's length in the frame; may saturate to infinity where the
    /// frame is far below the caller's unit (a subnormal loop).
    pub fn length(&self, caller: f64) -> f64 {
        libm::scalbn(caller, -self.frame.exponent())
    }

    /// A caller's area in the frame.
    pub fn area_of(&self, caller: f64) -> f64 {
        libm::scalbn(caller, self.frame.exponent().saturating_mul(-2))
    }

    /// The power of two a frame area is multiplied by to be the caller's.
    pub fn area_exponent(&self) -> i32 {
        self.frame.exponent().saturating_mul(2)
    }

    /// The allowance `L·P + ½·D·Σg + ROUNDING·M` (ADR-0013 D2),
    /// `tolerance` the loop's largest entity tolerance in caller units.
    /// Never `NaN`: a zero factor makes its product 0 even against an
    /// infinite one.
    pub fn allowance(&self, tolerance: f64) -> f64 {
        let gaps = self.gaps.iter().fold(0.0, |sum, g| sum + g);
        product(self.length(tolerance), self.perimeter)
            + product(0.5 * self.extent, gaps)
            + ROUNDING * self.magnitude
    }
}

/// `a·b`, with 0 for a zero factor whatever the other.
fn product(a: f64, b: f64) -> f64 {
    if a == 0.0 || b == 0.0 { 0.0 } else { a * b }
}

/// An arc's sweep from `s` to `e` about `c`, the counter-clockwise turn in
/// `[0, 2π)` (DATA-MODEL.md §Entities).
fn sweep(c: Xy, s: Xy, e: Xy) -> f64 {
    let (u, v) = (sub(s, c), sub(e, c));
    let turn = libm::atan2(cross(u, v), u.0 * v.0 + u.1 * v.1);
    if turn < 0.0 { turn + TAU } else { turn }
}

/// Measures `shape` (ADR-0013 D1).
pub(crate) fn measure(shape: Shape<'_>) -> Measured {
    match shape {
        Shape::Pieces(pieces) => measure_pieces(pieces),
        Shape::Circle { r, reversed } => {
            let frame = LoopFrame::new(&[], (0.0, 0.0), r.abs());
            let r = libm::scalbn(r, -frame.exponent());
            let area = PI * r * r;
            Measured {
                area: if reversed { -area } else { area },
                perimeter: TAU * r.abs(),
                extent: 2.0 * r.abs(),
                gaps: Vec::new(),
                off_radius: 0.0,
                magnitude: area,
                frame,
            }
        }
    }
}

fn measure_pieces(pieces: &[Piece]) -> Measured {
    let mut points = Vec::with_capacity(3 * pieces.len());
    for piece in pieces {
        match *piece {
            Piece::Line { from, to } => points.extend([from, to]),
            Piece::Arc { c, s, e, .. } => points.extend([c, s, e]),
        }
    }
    let origin = pieces.first().map_or((0.0, 0.0), Piece::entry);
    let frame = LoopFrame::new(&points, origin, 0.0);
    // Twice the area, about the origin (0, 0) of the frame, in loop order.
    let mut twice = 0.0;
    // Twice `M`: the products' sizes, then the partial sums'.
    let mut size = 0.0;
    let mut perimeter = 0.0;
    let mut off_radius: f64 = 0.0;
    for piece in pieces {
        match *piece {
            Piece::Line { from, to } => {
                let (a, b) = (frame.point(from), frame.point(to));
                twice += cross(a, b);
                size += products(a, b);
                perimeter += dist(a, b);
            }
            Piece::Arc { c, s, e, reversed } => {
                let (c, s, e) = (frame.point(c), frame.point(s), frame.point(e));
                let (rs, re) = (dist(c, s), dist(c, e));
                let theta = sweep(c, s, e);
                let sector = rs * re * theta;
                let ccw = cross(s, c) + sector + cross(c, e);
                twice += if reversed { -ccw } else { ccw };
                size += products(s, c) + sector + products(c, e);
                perimeter += rs.max(re) * theta;
                off_radius = off_radius.max((rs - re).abs());
            }
        }
        size += twice.abs();
    }
    let gaps = pieces
        .iter()
        .zip(pieces.iter().cycle().skip(1))
        .map(|(here, next)| dist(frame.point(here.exit()), frame.point(next.entry())))
        .collect();
    let (mut lo, mut hi) = ((0.0_f64, 0.0_f64), (0.0_f64, 0.0_f64));
    for p in &points {
        let q = frame.point(*p);
        lo = (lo.0.min(q.0), lo.1.min(q.1));
        hi = (hi.0.max(q.0), hi.1.max(q.1));
    }
    Measured {
        area: 0.5 * twice,
        perimeter,
        extent: dist(lo, hi),
        gaps,
        off_radius,
        magnitude: 0.5 * size,
        frame,
    }
}

/// The sizes of the two products in `cross(a, b)`.
fn products(a: Xy, b: Xy) -> f64 {
    (a.0 * b.1).abs() + (a.1 * b.0).abs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use laman::model::Sketch;
    use laman::model::tolerance::Tolerances;

    /// A measured area in caller units (may over- or underflow).
    fn caller_area(m: &Measured) -> f64 {
        libm::scalbn(m.area, m.area_exponent())
    }

    fn line(from: Xy, to: Xy) -> Piece {
        Piece::Line { from, to }
    }

    fn arc(c: Xy, s: Xy, e: Xy) -> Piece {
        Piece::Arc {
            c,
            s,
            e,
            reversed: false,
        }
    }

    /// The loop traversed the other way: pieces in reverse order, each
    /// reversed.
    fn reversed(pieces: &[Piece]) -> Vec<Piece> {
        pieces
            .iter()
            .rev()
            .map(|p| match *p {
                Piece::Line { from, to } => line(to, from),
                Piece::Arc { c, s, e, reversed } => Piece::Arc {
                    c,
                    s,
                    e,
                    reversed: !reversed,
                },
            })
            .collect()
    }

    fn rectangle(w: f64, h: f64) -> Vec<Piece> {
        let (a, b, c, d) = ((0.0, 0.0), (w, 0.0), (w, h), (0.0, h));
        vec![line(a, b), line(b, c), line(c, d), line(d, a)]
    }

    /// Two semicircles of radius `r` about `(0, 0)` and `(a, 0)`, joined by
    /// lines: area `2ra + πr²`.
    fn slot(a: f64, r: f64) -> Vec<Piece> {
        vec![
            line((0.0, -r), (a, -r)),
            arc((a, 0.0), (a, -r), (a, r)),
            line((a, r), (0.0, r)),
            arc((0.0, 0.0), (0.0, r), (0.0, -r)),
        ]
    }

    /// Three quarters of a disc of radius `r`: a sweep of `3π/2`, then the
    /// two radii back. Area `¾πr²`.
    fn three_quarters(r: f64) -> Vec<Piece> {
        let (c, s, e) = ((0.0, 0.0), (r, 0.0), (0.0, -r));
        vec![arc(c, s, e), line(e, c), line(c, s)]
    }

    fn scale_pieces(pieces: &[Piece], k: i32) -> Vec<Piece> {
        pieces
            .iter()
            .map(|p| match *p {
                Piece::Line { from, to } => line(scaled(from, k), scaled(to, k)),
                Piece::Arc { c, s, e, reversed } => Piece::Arc {
                    c: scaled(c, k),
                    s: scaled(s, k),
                    e: scaled(e, k),
                    reversed,
                },
            })
            .collect()
    }

    fn close(got: f64, want: f64, ulps: f64) {
        assert!(
            (got - want).abs() <= ulps * f64::EPSILON * want.abs(),
            "{got} against {want}"
        );
    }

    #[test]
    fn closed_forms_both_ways() {
        let cases: [(Vec<Piece>, f64); 3] = [
            (rectangle(120.0, 60.0), 7200.0),
            (slot(40.0, 5.0), 400.0 + 25.0 * PI),
            (three_quarters(10.0), 75.0 * PI),
        ];
        for (pieces, want) in cases {
            close(caller_area(&measure(Shape::Pieces(&pieces))), want, 4.0);
            let back = reversed(&pieces);
            close(caller_area(&measure(Shape::Pieces(&back))), -want, 4.0);
        }
        let circle = |reversed| measure(Shape::Circle { r: 10.0, reversed });
        close(caller_area(&circle(false)), 100.0 * PI, 2.0);
        close(caller_area(&circle(true)), -100.0 * PI, 2.0);
        // Perimeters and extents, in caller units.
        let m = measure(Shape::Pieces(&slot(40.0, 5.0)));
        close(m.perimeter / m.length(1.0), 80.0 + 10.0 * PI, 4.0);
        close(m.extent / m.length(1.0), libm::hypot(40.0, 10.0), 4.0);
        assert_eq!(m.gaps, vec![0.0; 4]);
        assert_eq!(m.off_radius, 0.0);
        let m = measure(Shape::Pieces(&three_quarters(10.0)));
        close(m.perimeter / m.length(1.0), 15.0 * PI + 20.0, 4.0);
    }

    /// The same loop scaled by `2^k` measures the same bits in its frame,
    /// out to 1e±300, and a loop spanning `±f64::MAX` has a finite frame
    /// area.
    #[test]
    fn extremes_measure_the_same_bits() {
        let shapes = [
            rectangle(120.0, 60.0),
            slot(40.0, 5.0),
            three_quarters(10.0),
        ];
        for pieces in &shapes {
            let unit = measure(Shape::Pieces(pieces));
            for k in [-1000, -996, 996] {
                let m = measure(Shape::Pieces(&scale_pieces(pieces, k)));
                assert_eq!(m.area.to_bits(), unit.area.to_bits(), "k = {k}");
                assert_eq!(m.area_exponent(), unit.area_exponent() + 2 * k);
                assert_eq!(m.perimeter, unit.perimeter);
                assert_eq!(m.extent, unit.extent);
                assert_eq!(m.gaps, unit.gaps);
            }
        }
        for k in [-1000, 996] {
            let r = libm::scalbn(10.0, k);
            let m = measure(Shape::Circle { r, reversed: false });
            let unit = measure(Shape::Circle {
                r: 10.0,
                reversed: false,
            });
            assert_eq!(m.area.to_bits(), unit.area.to_bits());
            assert_eq!(m.area_exponent(), unit.area_exponent() + 2 * k);
        }
        let big = f64::MAX;
        let (a, b, c, d) = ((-big, -big), (big, -big), (big, big), (-big, big));
        let m = measure(Shape::Pieces(&[
            line(a, b),
            line(b, c),
            line(c, d),
            line(d, a),
        ]));
        let half = libm::scalbn(big, -1024);
        assert_eq!(m.area, half * half);
        assert_eq!(m.area_exponent(), 2 * 1025);
        assert!(caller_area(&m).is_infinite());
        assert_eq!(m.area_of(f64::MAX), libm::scalbn(f64::MAX, -2050));
    }

    /// Subnormal and collapsed loops: no `NaN`, an allowance of 0 where
    /// nothing has length.
    #[test]
    fn tiny_and_collapsed_loops_are_never_nan() {
        let tiny = scale_pieces(&rectangle(3.0, 2.0), -1070);
        let m = measure(Shape::Pieces(&tiny));
        assert!(m.area > 0.0);
        assert!(!m.allowance(1e-12).is_nan());
        let p = (5.0, 5.0);
        let m = measure(Shape::Pieces(&[line(p, p), line(p, p)]));
        assert_eq!((m.area, m.perimeter, m.extent), (0.0, 0.0, 0.0));
        assert_eq!(m.allowance(f64::MAX), 0.0);
        assert_eq!(m.allowance(1e-12), 0.0);
        let m = measure(Shape::Pieces(&[]));
        assert_eq!((m.area, m.allowance(1.0)), (0.0, 0.0));
        let m = measure(Shape::Circle {
            r: 0.0,
            reversed: false,
        });
        assert_eq!((m.area, m.allowance(1.0)), (0.0, 0.0));
    }

    /// A join left open by `g`, and an arc whose end sits `δ` off its
    /// start's circle, are measured in caller units.
    #[test]
    fn gaps_and_off_radius() {
        let g = 0.25;
        let mut pieces = rectangle(120.0, 60.0);
        pieces[2] = line((120.0, 60.0 + g), (0.0, 60.0));
        let m = measure(Shape::Pieces(&pieces));
        let unit = m.length(1.0);
        assert_eq!(m.gaps, vec![0.0, g * unit, 0.0, 0.0]);
        let d = 0.5;
        let mut pieces = three_quarters(10.0);
        pieces[0] = arc((0.0, 0.0), (10.0, 0.0), (0.0, -10.0 - d));
        let m = measure(Shape::Pieces(&pieces));
        assert_eq!(m.off_radius, d * m.length(1.0));
        assert_eq!(m.gaps, vec![d * m.length(1.0), 0.0, 0.0]);
    }

    /// `θ − sin θ` for a small `θ`, from its series (a reference only).
    fn segment(theta: f64) -> f64 {
        let t2 = theta * theta;
        let mut acc = 0.0;
        // Ten terms `(−1)^k / (2k + 3)!`, Horner from the last.
        for k in (0..10_u32).rev() {
            let fact: f64 = (1..=2 * k + 3).map(f64::from).product();
            let sign = if k % 2 == 0 { 1.0 } else { -1.0 };
            acc = sign / fact + t2 * acc;
        }
        theta * t2 * acc
    }

    /// The largest `Tolerances::at` entity tolerance over the sketch's
    /// entities (the loop's `L`).
    fn loop_tolerance(sk: &Sketch) -> f64 {
        let t = Tolerances::at(sk, &sk.coordinates());
        sk.entities()
            .map(|(id, _)| t.entity(id))
            .fold(0.0, f64::max)
    }

    /// A measure's error against an exact frame area, over each term of
    /// the allowance and over the whole.
    #[derive(Debug, Clone, Copy, Default)]
    struct Margin {
        /// Over `L·P`.
        tolerance: f64,
        /// Over `ROUNDING·M`.
        rounding: f64,
        /// Over the allowance.
        allowance: f64,
    }

    impl Margin {
        fn of(m: &Measured, want: f64, sk: &Sketch) -> Margin {
            let err = (m.area - want).abs();
            let l = loop_tolerance(sk);
            Margin {
                tolerance: err / (m.length(l) * m.perimeter),
                rounding: err / (ROUNDING * m.magnitude),
                allowance: err / m.allowance(l),
            }
        }

        fn worst(self, other: Margin) -> Margin {
            Margin {
                tolerance: self.tolerance.max(other.tolerance),
                rounding: self.rounding.max(other.rounding),
                allowance: self.allowance.max(other.allowance),
            }
        }
    }

    /// A `w × h` box whose top is an arc of radius about `R` bulging up,
    /// measured from the bottom-left corner, `h` from the arc: the sector
    /// and the two triangles each about `h·R`, their sum a sliver.
    fn near_straight(w: f64, h: f64, radius: f64) -> Margin {
        let half = 0.5 * w;
        let cy = h - (radius * radius - half * half).sqrt();
        let mut sk = Sketch::new();
        let a = sk.point(0.0, 0.0).unwrap();
        let b = sk.point(w, 0.0).unwrap();
        let s = sk.point(w, h).unwrap();
        let e = sk.point(0.0, h).unwrap();
        let c = sk.point(half, cy).unwrap();
        sk.line(a, b).unwrap();
        sk.line(b, s).unwrap();
        sk.arc(c, s, e).unwrap();
        sk.line(e, a).unwrap();
        let pieces = [
            line((0.0, 0.0), (w, 0.0)),
            line((w, 0.0), (w, h)),
            arc((half, cy), (w, h), (0.0, h)),
            line((0.0, h), (0.0, 0.0)),
        ];
        let m = measure(Shape::Pieces(&pieces));
        // The segment above the chord, from the coordinates as rounded.
        let dy = h - cy;
        let theta = 2.0 * libm::atan2(half, dy);
        let want = w * h + 0.5 * (half * half + dy * dy) * segment(theta);
        Margin::of(&m, m.area_of(want), &sk)
    }

    /// The sector form's cancellation on a near-straight arc, radii 1e3 to
    /// 1e15, is covered by `L·P` alone: measured worst 1.6e-6 of it (R =
    /// 1e15, h = 1), and 0.070 of `ROUNDING·M`.
    #[test]
    fn near_straight_arc_is_inside_the_allowance() {
        let mut worst = Margin::default();
        for radius in [1e3, 1e6, 1e9, 1e12, 1e15] {
            for h in [1.0, 1e3, 1e6] {
                let margin = near_straight(1.0, h, radius);
                println!("R = {radius:e}, h = {h:e}: {margin:?}");
                worst = worst.worst(margin);
            }
        }
        println!("worst {worst:?}");
        assert!(worst.tolerance < 1e-4, "{worst:?}");
        assert!(worst.rounding < 0.25, "{worst:?}");
        assert!(worst.allowance < 1e-4, "{worst:?}");
    }

    /// A regular `n`-gon on integer coordinates, its area exact in `i128`,
    /// near the origin and `2^40` from it.
    fn polygon(n: u32) -> Margin {
        let radius = f64::from(1u32 << 29);
        let corners: Vec<(i64, i64)> = (0..n)
            .map(|i| {
                let t = TAU * f64::from(i) / f64::from(n);
                (
                    (radius * libm::cos(t)).round() as i64,
                    (radius * libm::sin(t)).round() as i64,
                )
            })
            .collect();
        let next = |i: usize| corners[(i + 1) % corners.len()];
        let mut twice: i128 = 0;
        for (i, p) in corners.iter().enumerate() {
            let q = next(i);
            twice += i128::from(p.0) * i128::from(q.1) - i128::from(q.0) * i128::from(p.1);
        }
        let mut worst = Margin::default();
        for shift in [0_i64, 1 << 40] {
            let at = |p: (i64, i64)| ((p.0 + shift) as f64, (p.1 + shift) as f64);
            let mut sk = Sketch::new();
            let points: Vec<_> = corners
                .iter()
                .map(|p| {
                    let (x, y) = at(*p);
                    sk.point(x, y).unwrap()
                })
                .collect();
            for i in 0..points.len() {
                sk.line(points[i], points[(i + 1) % points.len()]).unwrap();
            }
            let pieces: Vec<_> = (0..corners.len())
                .map(|i| line(at(corners[i]), at(next(i))))
                .collect();
            let m = measure(Shape::Pieces(&pieces));
            // The shoelace sum is invariant under the shift; one rounding.
            let margin = Margin::of(&m, m.area_of(0.5 * twice as f64), &sk);
            println!("n = {n}, shift {shift}: {margin:?}");
            worst = worst.worst(margin);
        }
        worst
    }

    /// Why the allowance has a rounding term: on a long loop the products'
    /// rounding outgrows `L·P`. Measured: 2.8e-3 of `L·P` at 500 edges and
    /// 2.2 of it at 50 000; 2.3e-3 and 1.8e-4 of `ROUNDING·M`.
    #[test]
    fn long_loops_are_inside_the_allowance() {
        let short = polygon(500);
        assert!(short.tolerance < 1e-2, "{short:?}");
        assert!(short.allowance < 1e-2, "{short:?}");
        let long = polygon(50_000);
        assert!(long.tolerance > 1.0, "L·P alone covers it: {long:?}");
        assert!(long.rounding < 0.01, "{long:?}");
        assert!(long.allowance < 0.01, "{long:?}");
    }
}
