//! Containment for the profile check (ADR-0013 D4): a probe point against a
//! boundary by signed crossings of a ray toward +x.
//!
//! The count is the boundary's winding number about the probe, found by the
//! crossings of the ray, each signed by the direction the boundary crosses
//! it, not by extraction's angle sum. A crossing is half-open in `y`: a line,
//! or an arc cut at its highest and lowest points, crosses where one end lies
//! strictly above the probe and the other at or below it, so a vertex on the
//! ray counts once. A probe within the boundary's tolerance of it is
//! undecided.
//!
//! All coordinates are in one frame scaled by a power of two, so no product
//! overflows at any finite input.

use super::{Xy, cross, dist, sub};
use std::f64::consts::{FRAC_PI_2, TAU};

/// One piece of a boundary, in the direction the loop traverses it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Seg {
    /// A line from `from` to `to`.
    Line { from: Xy, to: Xy },
    /// An arc counter-clockwise from `s` to `e` about `c`, traversed the
    /// other way when `reversed`.
    Arc { c: Xy, s: Xy, e: Xy, reversed: bool },
    /// A circle counter-clockwise, clockwise when `reversed`.
    Circle { c: Xy, r: f64, reversed: bool },
}

impl Seg {
    /// The largest magnitude among the coordinates the piece reads (a
    /// circle's radius counted).
    pub(crate) fn largest(&self) -> f64 {
        let big = |p: Xy| p.0.abs().max(p.1.abs());
        match *self {
            Seg::Line { from, to } => big(from).max(big(to)),
            Seg::Arc { c, s, e, .. } => big(c).max(big(s)).max(big(e)),
            Seg::Circle { c, r, .. } => big(c).max(r.abs()),
        }
    }

    /// The piece scaled by `2^k`.
    pub(crate) fn scaled(&self, k: i32) -> Seg {
        let sc = |p: Xy| (libm::scalbn(p.0, k), libm::scalbn(p.1, k));
        match *self {
            Seg::Line { from, to } => Seg::Line {
                from: sc(from),
                to: sc(to),
            },
            Seg::Arc { c, s, e, reversed } => Seg::Arc {
                c: sc(c),
                s: sc(s),
                e: sc(e),
                reversed,
            },
            Seg::Circle { c, r, reversed } => Seg::Circle {
                c: sc(c),
                r: libm::scalbn(r, k),
                reversed,
            },
        }
    }

    /// The signed crossings of the ray from `q` toward +x.
    fn crossings(&self, q: Xy) -> i32 {
        match *self {
            Seg::Line { from, to } => line_crossing(from, to, q),
            Seg::Arc { c, s, e, reversed } => {
                let n = arc_crossings(c, s, e, q);
                if reversed { -n } else { n }
            }
            Seg::Circle { c, r, reversed } => {
                let dy = q.1 - c.1;
                let n = if dy.abs() < r.abs() {
                    let half = libm::sqrt(r * r - dy * dy);
                    // The right side runs upward, the left downward.
                    i32::from(c.0 + half > q.0) - i32::from(c.0 - half > q.0)
                } else {
                    0
                };
                if reversed { -n } else { n }
            }
        }
    }

    /// How far `q` is from the piece.
    fn distance(&self, q: Xy) -> f64 {
        match *self {
            Seg::Line { from, to } => to_segment(from, to, q),
            Seg::Arc { c, s, e, .. } => {
                let (u, w) = (sub(s, c), sub(q, c));
                let start = libm::atan2(u.1, u.0);
                let off = (libm::atan2(w.1, w.0) - start).rem_euclid(TAU);
                if off <= sweep(c, s, e) {
                    (dist(q, c) - dist(s, c)).abs()
                } else {
                    dist(q, s).min(dist(q, e))
                }
            }
            Seg::Circle { c, r, .. } => (dist(q, c) - r.abs()).abs(),
        }
    }

    /// A point on the piece in the middle of its traversal.
    pub(crate) fn midpoint(&self) -> Xy {
        match *self {
            Seg::Line { from, to } => ((from.0 + to.0) * 0.5, (from.1 + to.1) * 0.5),
            Seg::Arc { c, s, e, .. } => {
                let half = 0.5 * sweep(c, s, e);
                let (sin, cos) = (libm::sin(half), libm::cos(half));
                let u = sub(s, c);
                (c.0 + cos * u.0 - sin * u.1, c.1 + sin * u.0 + cos * u.1)
            }
            Seg::Circle { c, r, .. } => (c.0 + r.abs(), c.1),
        }
    }
}

/// The turn from `s` to `e` about `c`, counter-clockwise in `[0, 2π)`.
fn sweep(c: Xy, s: Xy, e: Xy) -> f64 {
    let (u, v) = (sub(s, c), sub(e, c));
    let turn = libm::atan2(cross(u, v), u.0 * v.0 + u.1 * v.1);
    if turn < 0.0 { turn + TAU } else { turn }
}

/// A line's signed crossing of the ray: `+1` for an upward edge right of
/// `q`, `-1` for a downward one, 0 otherwise.
fn line_crossing(a: Xy, b: Xy, q: Xy) -> i32 {
    if (a.1 > q.1) == (b.1 > q.1) {
        return 0;
    }
    let x = a.0 + (q.1 - a.1) * (b.0 - a.0) / (b.1 - a.1);
    if x > q.0 {
        if b.1 > a.1 { 1 } else { -1 }
    } else {
        0
    }
}

/// A counter-clockwise arc's signed crossings, cut at its highest and
/// lowest points so that every piece is monotone in `y`.
fn arc_crossings(c: Xy, s: Xy, e: Xy, q: Xy) -> i32 {
    let r = dist(c, s);
    let total = sweep(c, s, e);
    let start = libm::atan2(s.1 - c.1, s.0 - c.0);
    let mut cuts: Vec<(f64, Xy)> = [(FRAC_PI_2, r), (3.0 * FRAC_PI_2, -r)]
        .into_iter()
        .map(|(angle, h)| ((angle - start).rem_euclid(TAU), (c.0, c.1 + h)))
        .filter(|(off, _)| *off > 0.0 && *off < total)
        .collect();
    cuts.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut points = vec![s];
    points.extend(cuts.into_iter().map(|(_, p)| p));
    points.push(e);
    let dy = q.1 - c.1;
    let half = libm::sqrt((r * r - dy * dy).max(0.0));
    points
        .windows(2)
        .map(|w| {
            let (p0, p1) = (w[0], w[1]);
            if (p0.1 > q.1) == (p1.1 > q.1) {
                return 0;
            }
            // Counter-clockwise, upward is the right half of the circle.
            let up = p1.1 > p0.1;
            let x = if up { c.0 + half } else { c.0 - half };
            if x > q.0 { if up { 1 } else { -1 } } else { 0 }
        })
        .sum()
}

/// The distance from `q` to the segment `a`–`b`.
fn to_segment(a: Xy, b: Xy, q: Xy) -> f64 {
    let (u, w) = (sub(b, a), sub(q, a));
    let len2 = u.0 * u.0 + u.1 * u.1;
    let t = if len2 > 0.0 {
        ((w.0 * u.0 + w.1 * u.1) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    dist(q, (a.0 + t * u.0, a.1 + t * u.1))
}

/// Where a probe stands against a boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Standing {
    Inside,
    Outside,
    /// Within the boundary's tolerance of it: no verdict.
    Undecided,
}

/// `probe` against `segs`, undecided within `tolerance` of them.
pub(crate) fn standing(segs: &[Seg], tolerance: f64, probe: Xy) -> Standing {
    if segs.iter().any(|s| s.distance(probe) <= tolerance) {
        return Standing::Undecided;
    }
    let winding: i32 = segs.iter().map(|s| s.crossings(probe)).sum();
    if winding == 0 {
        Standing::Outside
    } else {
        Standing::Inside
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(from: Xy, to: Xy) -> Seg {
        Seg::Line { from, to }
    }

    fn diamond() -> Vec<Seg> {
        let (a, b, c, d) = ((0.0, -2.0), (2.0, 0.0), (0.0, 2.0), (-2.0, 0.0));
        vec![line(a, b), line(b, c), line(c, d), line(d, a)]
    }

    /// A slot: two semicircles of radius 1 about (0, 0) and (4, 0).
    fn slot() -> Vec<Seg> {
        vec![
            line((0.0, -1.0), (4.0, -1.0)),
            Seg::Arc {
                c: (4.0, 0.0),
                s: (4.0, -1.0),
                e: (4.0, 1.0),
                reversed: false,
            },
            line((4.0, 1.0), (0.0, 1.0)),
            Seg::Arc {
                c: (0.0, 0.0),
                s: (0.0, 1.0),
                e: (0.0, -1.0),
                reversed: false,
            },
        ]
    }

    #[test]
    fn a_vertex_on_the_ray_counts_once() {
        // The ray from (-1, 0) passes the diamond's right vertex (2, 0) and
        // its left one (-2, 0) is behind it: one crossing, not two or none.
        assert_eq!(standing(&diamond(), 0.0, (-1.0, 0.0)), Standing::Inside);
        assert_eq!(standing(&diamond(), 0.0, (3.0, 0.0)), Standing::Outside);
        assert_eq!(standing(&diamond(), 0.0, (-3.0, 0.0)), Standing::Outside);
        // A ray along the diamond's top vertex.
        assert_eq!(standing(&diamond(), 0.0, (-1.0, 2.0)), Standing::Outside);
        assert_eq!(standing(&diamond(), 0.0, (-5.0, 2.0)), Standing::Outside);
    }

    #[test]
    fn arcs_are_cut_at_their_extremes() {
        let s = slot();
        // Rays at the semicircles' top and bottom heights, and through
        // their centres, inside and outside.
        for (probe, want) in [
            ((2.0, 0.0), Standing::Inside),
            ((-0.5, 0.0), Standing::Inside),
            ((4.5, 0.0), Standing::Inside),
            ((5.5, 0.0), Standing::Outside),
            ((-1.5, 0.0), Standing::Outside),
            ((2.0, 2.0), Standing::Outside),
            ((2.0, -2.0), Standing::Outside),
            ((-0.5, 0.8), Standing::Inside),
            ((-0.6, 0.9), Standing::Outside),
        ] {
            assert_eq!(standing(&s, 0.0, probe), want, "{probe:?}");
        }
        // Reversed: the same set traversed clockwise winds -1, still inside.
        let back: Vec<Seg> = s
            .iter()
            .rev()
            .map(|seg| match *seg {
                Seg::Line { from, to } => line(to, from),
                Seg::Arc { c, s, e, reversed } => Seg::Arc {
                    c,
                    s,
                    e,
                    reversed: !reversed,
                },
                Seg::Circle { .. } => *seg,
            })
            .collect();
        assert_eq!(standing(&back, 0.0, (2.0, 0.0)), Standing::Inside);
        assert_eq!(standing(&back, 0.0, (5.5, 0.0)), Standing::Outside);
    }

    #[test]
    fn a_circle_tangent_to_the_ray_counts_nothing() {
        let c = Seg::Circle {
            c: (0.0, 0.0),
            r: 1.0,
            reversed: false,
        };
        for y in [1.0, -1.0, 2.0] {
            assert_eq!(standing(&[c], 0.0, (-3.0, y)), Standing::Outside);
        }
        assert_eq!(standing(&[c], 0.0, (-0.5, 0.0)), Standing::Inside);
        assert_eq!(standing(&[c], 0.0, (-0.5, 0.8)), Standing::Inside);
    }

    #[test]
    fn a_probe_near_the_boundary_is_undecided() {
        let s = slot();
        // On a line, on an arc, near a corner; just inside the band.
        assert_eq!(standing(&s, 0.01, (2.0, 1.0 - 0.005)), Standing::Undecided);
        assert_eq!(standing(&s, 0.01, (5.0 - 0.005, 0.0)), Standing::Undecided);
        assert_eq!(standing(&s, 0.01, (2.0, 1.0 - 0.02)), Standing::Inside);
        assert_eq!(standing(&s, 0.01, (5.02, 0.0)), Standing::Outside);
    }

    #[test]
    fn midpoints_lie_on_the_piece() {
        let s = slot();
        for seg in &s {
            assert!(seg.distance(seg.midpoint()) < 1e-12);
        }
        let m = s[3].midpoint();
        assert!((m.0 + 1.0).abs() < 1e-12 && m.1.abs() < 1e-12, "{m:?}");
    }
}
