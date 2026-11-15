//! The order of the edge ends at a join (ADR-0017 D2, D3): counter-clockwise
//! by the direction each leaves in; ends that leave tangent by signed
//! curvature; ends that overlap by edge id.
//!
//! The ends are measured in the [`Frame`] of the edges meeting at the join,
//! so the order reads alike at every scale and translation. Directions are
//! compared exactly, by half-plane and the sign of a cross product, no angle
//! computed: the sort is a total order on the measured directions, whatever
//! their rounding. Two directions **tie** when the ends would stay within
//! `L` of one another over the shorter edge, `s · |sin Δθ| ≤ L` with
//! `cos Δθ > 0`; two curvatures when `s² |Δκ| / 2 ≤ L`. `s` is the shorter
//! edge's whole length and `L` the smaller of the two edges' length
//! tolerances, so the tests have no constant of their own.

use super::geometry::{Edge, sweep};
use crate::frame::{Frame, cross, dot, sub};
use crate::model::EntityId;
use std::cmp::Ordering;

/// An edge end at a join, as the walk hands it to [`order`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct End {
    /// The edge.
    pub edge: EntityId,
    /// Whether this is the edge's second end as drawn (a line's `b`, an
    /// arc's end), so the edge leaves the join reversed.
    pub reversed: bool,
    /// The edge, traversed away from the join: its first point is this end.
    pub leaving: Edge,
    /// The edge's length tolerance in the caller's units
    /// (`Tolerances::entity`, ADR-0012 D4).
    pub tolerance: f64,
}

/// An end measured in its join's frame.
#[derive(Clone, Copy, Debug)]
struct Measured {
    /// The direction it leaves in, as computed, never zero: an exact input
    /// to the order.
    direction: [f64; 2],
    /// `direction` at unit length, for the tie test.
    unit: [f64; 2],
    /// Signed: `+1/r` for an arc turning left, `−1/r` turning right, 0 for a
    /// line.
    curvature: f64,
    /// The edge's whole length (a line's, an arc's `r` times its sweep).
    length: f64,
    /// The edge's length tolerance in the frame.
    tolerance: f64,
    /// The last resort: edge id, its start before its end.
    key: (EntityId, bool),
}

/// Leaving east: the direction a zero-length end is given, so every end has
/// one and the order stays total. Not a tolerance: an end of zero length
/// leaves in no direction, and any fixed one is as good.
const NO_DIRECTION: [f64; 2] = [1.0, 0.0];

fn measure(end: &End, frame: &Frame) -> Measured {
    let (direction, curvature, length) = match end.leaving {
        Edge::Line { from, to } => {
            let d = sub(frame.point(to), frame.point(from));
            (d, 0.0, libm::hypot(d[0], d[1]))
        }
        Edge::Arc {
            centre,
            start,
            end,
            reversed,
        } => {
            let (c, s, e) = (frame.point(centre), frame.point(start), frame.point(end));
            let r = sub(if reversed { e } else { s }, c);
            let radius = libm::hypot(r[0], r[1]);
            // Perpendicular to the radius, turning with the traversal:
            // counter-clockwise from the start, clockwise from the end.
            let (d, k) = if reversed {
                ([r[1], -r[0]], -1.0 / radius)
            } else {
                ([-r[1], r[0]], 1.0 / radius)
            };
            (d, k, radius * sweep(c, s, e))
        }
    };
    let norm = libm::hypot(direction[0], direction[1]);
    let (direction, unit) = if norm > 0.0 {
        (direction, [direction[0] / norm, direction[1] / norm])
    } else {
        (NO_DIRECTION, NO_DIRECTION)
    };
    Measured {
        direction,
        unit,
        curvature,
        length,
        tolerance: frame.length(end.tolerance),
        key: (end.edge, end.reversed),
    }
}

/// 0 for a direction in `[π, 2π)`, from −x through −y; 1 for `[0, π)`. The
/// order starts at −x.
fn half(v: [f64; 2]) -> u8 {
    if v[1] < 0.0 || (v[1] == 0.0 && v[0] < 0.0) {
        0
    } else {
        1
    }
}

/// `a₀b₁` against `a₁b₀`, exactly: `Greater` when `b` lies counter-clockwise
/// of `a` within a half-turn. Rounding is monotone, so two products that
/// round apart compare as their exact values do; two that round alike
/// differ by their rounding errors, which `mul_add` gives exactly (short of
/// underflow).
fn turn(a: [f64; 2], b: [f64; 2]) -> Ordering {
    let (p, q) = (a[0] * b[1], a[1] * b[0]);
    match p.partial_cmp(&q) {
        Some(Ordering::Equal) => a[0]
            .mul_add(b[1], -p)
            .partial_cmp(&a[1].mul_add(b[0], -q))
            .unwrap_or(Ordering::Equal),
        Some(o) => o,
        None => Ordering::Equal,
    }
}

/// Counter-clockwise from −x, exactly; equal directions by edge id.
fn by_direction(a: &Measured, b: &Measured) -> Ordering {
    half(a.direction)
        .cmp(&half(b.direction))
        .then_with(|| turn(b.direction, a.direction))
        .then(a.key.cmp(&b.key))
}

/// `s` and `L` of a pair: the shorter edge's length, the smaller tolerance.
fn reach(a: &Measured, b: &Measured) -> (f64, f64) {
    (a.length.min(b.length), a.tolerance.min(b.tolerance))
}

/// ADR-0017 D3: the ends would stay within `L` over the shorter edge.
fn directions_tie(a: &Measured, b: &Measured) -> bool {
    let (s, l) = reach(a, b);
    dot(a.unit, b.unit) > 0.0 && s * cross(a.unit, b.unit).abs() <= l
}

/// ADR-0017 D2: the curves part by at most `L` over the shorter edge.
fn curvatures_tie(a: &Measured, b: &Measured) -> bool {
    let (s, l) = reach(a, b);
    s * (s * (a.curvature - b.curvature).abs()) / 2.0 <= l
}

/// The ends of one join in counter-clockwise order, as indices into `ends`:
/// the order is cyclic, and the walk reads the next clockwise as the one
/// before.
///
/// Directions in exact order from −x; a chain of ends whose neighbours tie
/// in direction is a tangent run, which may wrap past −x, and is sorted by
/// curvature ascending (counter-clockwise just past the join); a chain whose
/// curvatures tie is an overlap, sorted by edge id, an edge's start before
/// its end. The list starts at the first run that does not wrap.
pub(crate) fn order(ends: &[End]) -> Vec<usize> {
    let mut points = Vec::with_capacity(3 * ends.len());
    for end in ends {
        match end.leaving {
            Edge::Line { from, to } => points.extend([from, to]),
            Edge::Arc {
                centre, start, end, ..
            } => points.extend([centre, start, end]),
        }
    }
    let origin = ends.first().map_or([0.0, 0.0], |e| e.leaving.first());
    let frame = Frame::new(&points, origin, 0.0);
    let m: Vec<Measured> = ends.iter().map(|e| measure(e, &frame)).collect();

    let mut sorted: Vec<usize> = (0..m.len()).collect();
    sorted.sort_by(|&i, &j| by_direction(&m[i], &m[j]));
    let n = sorted.len();
    if n < 2 {
        return sorted;
    }
    // `ties[k]`: the end at `k` ties with the next, cyclically.
    let ties: Vec<bool> = (0..n)
        .map(|k| directions_tie(&m[sorted[k]], &m[sorted[(k + 1) % n]]))
        .collect();
    let start = (0..n).find(|&k| !ties[(k + n - 1) % n]).unwrap_or(0);

    let mut out = Vec::with_capacity(n);
    let mut k = 0;
    while k < n {
        let mut run = vec![sorted[(start + k) % n]];
        while k + 1 < n && ties[(start + k) % n] {
            k += 1;
            run.push(sorted[(start + k) % n]);
        }
        k += 1;
        by_curvature(&mut run, &m);
        out.extend(run);
    }
    out
}

/// Sorts a tangent run by curvature ascending; a chain of tied curvatures
/// by edge id.
fn by_curvature(run: &mut [usize], m: &[Measured]) {
    run.sort_by(|&i, &j| {
        m[i].curvature
            .total_cmp(&m[j].curvature)
            .then(m[i].key.cmp(&m[j].key))
    });
    let mut g = 0;
    while g < run.len() {
        let mut h = g + 1;
        while h < run.len() && curvatures_tie(&m[run[h - 1]], &m[run[h]]) {
            h += 1;
        }
        run[g..h].sort_by_key(|&i| m[i].key);
        g = h;
    }
}

#[cfg(test)]
mod tests;
