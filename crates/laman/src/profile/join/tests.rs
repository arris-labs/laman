#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! The order at a join against closed forms (ADR-0017 D2, D3): lines,
//! a line against arcs tangent on either side, two arcs tangent with
//! different radii and with equal radii, each built at a join at the origin,
//! then rotated, scaled from 1e-6 to 1e6, translated up to 1e4 sizes, and
//! near 1e±300. A declared tangent is also nudged off by an eighth of the
//! length tolerance, to the side its exact direction would order wrongly.
use super::*;
use crate::model::tolerance;
use std::f64::consts::PI;

const SCALES: [f64; 5] = [1e-6, 1e-3, 1.0, 1e3, 1e6];
/// Translations, in units of the construction (about a tenth of its size).
const SHIFTS: [f64; 3] = [0.0, 10.0, 1e5];
const ROTATIONS: [f64; 3] = [0.0, 0.3, 2.5];

/// The construction's extent: lines of length 10, arcs of radius 5 and 10.
const SIZE: f64 = 10.0;
const LINE: f64 = 10.0;
const R1: f64 = 5.0;
const R2: f64 = 10.0;

/// Where a construction is placed, and the length tolerance its edges get.
#[derive(Clone, Copy, Debug)]
struct Place {
    rotation: f64,
    scale: f64,
    shift: f64,
    /// The length tolerance, in the caller's units.
    tolerance: f64,
}

impl Place {
    fn at(rotation: f64, scale: f64, shift: f64) -> Self {
        Place {
            rotation,
            scale,
            shift,
            tolerance: tolerance::length(SIZE * scale),
        }
    }

    /// A construction point, rotated about the join, scaled, translated
    /// along `(1, 0.7)`.
    fn point(&self, p: [f64; 2]) -> [f64; 2] {
        let (sin, cos) = libm::sincos(self.rotation);
        let q = [cos * p[0] - sin * p[1], sin * p[0] + cos * p[1]];
        [
            self.scale * (q[0] + self.shift),
            self.scale * (q[1] + 0.7 * self.shift),
        ]
    }

    /// The length tolerance in construction units: the nudge's measure.
    fn unit_tolerance(&self) -> f64 {
        self.tolerance / self.scale
    }

    /// A line from the join toward `to`.
    fn line(&self, id: u32, to: [f64; 2]) -> End {
        End {
            edge: EntityId(id),
            reversed: false,
            leaving: Edge::Line {
                from: self.point([0.0, 0.0]),
                to: self.point(to),
            },
            tolerance: self.tolerance,
        }
    }

    /// An arc about `c`, counter-clockwise from `s` to `e`, leaving the join
    /// from its end when `reversed`.
    fn arc(&self, id: u32, c: [f64; 2], s: [f64; 2], e: [f64; 2], reversed: bool) -> End {
        End {
            edge: EntityId(id),
            reversed,
            leaving: Edge::Arc {
                centre: self.point(c),
                start: self.point(s),
                end: self.point(e),
                reversed,
            },
            tolerance: self.tolerance,
        }
    }
}

fn polar(r: f64, angle: f64) -> [f64; 2] {
    let (sin, cos) = libm::sincos(angle);
    [r * cos, r * sin]
}

/// Every place the closed forms are checked at.
fn places() -> Vec<Place> {
    let mut out = Vec::new();
    for &rotation in &ROTATIONS {
        for &scale in &SCALES {
            for &shift in &SHIFTS {
                out.push(Place::at(rotation, scale, shift));
            }
        }
    }
    out
}

/// The edge ids of `order(ends)`, checked against `want` as a cyclic order,
/// for the ends as given, reversed and rotated (the order does not read
/// the input's).
fn assert_cyclic(ends: &[End], want: &[u32], what: &str) {
    let mut inputs = vec![ends.to_vec()];
    inputs.push(ends.iter().rev().copied().collect());
    let mut turned = ends.to_vec();
    turned.rotate_left(ends.len() / 2);
    inputs.push(turned);
    for input in inputs {
        let got: Vec<u32> = order(&input).iter().map(|&i| input[i].edge.0).collect();
        assert_eq!(got.len(), want.len(), "{what}: {got:?}");
        let at = got.iter().position(|&id| id == want[0]);
        let mut rotated = got.clone();
        rotated.rotate_left(at.unwrap_or_else(|| panic!("{what}: {got:?} lacks {}", want[0])));
        assert_eq!(rotated, want, "{what}");
    }
}

/// Lines in four directions, one that leaves just outside the tie (four
/// times `L` apart over its length) and one inside it (an eighth), which is
/// an overlap and goes by id against its direction.
fn lines(p: &Place, near: bool) -> (Vec<End>, Vec<u32>) {
    let mut ends = vec![
        p.line(7, [LINE, 0.0]),
        p.line(2, polar(LINE, 2.0 * PI / 3.0)),
        p.line(3, polar(LINE, 4.0 * PI / 3.0)),
        p.line(4, polar(LINE, 5.0 * PI / 3.0)),
    ];
    if !near {
        return (ends, vec![7, 2, 3, 4]);
    }
    let apart = 4.0 * p.unit_tolerance() / LINE;
    ends.push(p.line(5, polar(LINE, apart)));
    ends.push(p.line(6, polar(LINE, apart / 32.0)));
    (ends, vec![6, 7, 5, 2, 3, 4])
}

#[test]
fn lines_order_by_direction_and_an_overlap_by_id() {
    for p in places() {
        let (ends, want) = lines(&p, true);
        assert_cyclic(&ends, &want, &format!("{p:?}"));
    }
}

/// East: an arc of radius `R1` turning left (leaving from its start) and
/// one turning right (leaving from its end), the line between them; a line
/// west. `nudge` moves each arc's centre by that many construction units
/// along +x, which turns the left arc's direction below east and the right
/// arc's above: the exact directions then order them wrongly.
fn tangent_east(p: &Place, nudge: f64) -> Vec<End> {
    vec![
        p.line(1, [LINE, 0.0]),
        p.arc(2, [nudge, R1], [0.0, 0.0], [R1, R1], false),
        p.arc(3, [nudge, -R1], [R1, -R1], [0.0, 0.0], true),
        p.line(4, [-LINE, 0.0]),
    ]
}

#[test]
fn a_line_and_arcs_tangent_on_either_side_order_by_curvature() {
    for p in places() {
        for nudge in [0.0, p.unit_tolerance() / 8.0] {
            let ends = tangent_east(&p, nudge);
            assert_cyclic(&ends, &[4, 3, 1, 2], &format!("{p:?} nudge {nudge}"));
        }
    }
}

/// The same tangent heading west, so the run's exact directions straddle
/// −x, where the order starts, and the run wraps.
#[test]
fn a_tangent_run_wraps_past_minus_x() {
    for p in places() {
        for nudge in [0.0, p.unit_tolerance() / 8.0] {
            let ends = vec![
                p.line(1, [-LINE, 0.0]),
                p.arc(2, [nudge, -R1], [0.0, 0.0], [-R1, -R1], false),
                p.arc(3, [nudge, R1], [-R1, R1], [0.0, 0.0], true),
                p.line(4, [LINE, 0.0]),
            ];
            assert_cyclic(&ends, &[3, 1, 2, 4], &format!("{p:?} nudge {nudge}"));
        }
    }
}

/// Arcs tangent east: radius `R1` and `R2` turning left, `R2` turning
/// right, and a line west. The tighter left turn lies above.
fn two_radii(p: &Place, nudge: f64) -> Vec<End> {
    vec![
        p.arc(1, [nudge, R1], [0.0, 0.0], [R1, R1], false),
        p.arc(2, [-nudge, R2], [0.0, 0.0], [R2, R2], false),
        p.line(3, [-LINE, 0.0]),
        p.arc(4, [nudge, -R2], [R2, -R2], [0.0, 0.0], true),
    ]
}

#[test]
fn two_arcs_tangent_with_different_radii_order_by_curvature() {
    for p in places() {
        for nudge in [0.0, p.unit_tolerance() / 8.0] {
            let ends = two_radii(&p, nudge);
            assert_cyclic(&ends, &[3, 4, 2, 1], &format!("{p:?} nudge {nudge}"));
        }
    }
}

/// Two arcs of equal radius leaving east, both turning left, of a quarter
/// and a half turn: an overlap, ordered by id whatever their sweeps.
fn equal_radii(p: &Place, nudge: f64) -> Vec<End> {
    vec![
        p.arc(9, [nudge, R1], [0.0, 0.0], [R1, R1], false),
        p.arc(4, [-nudge, R1], [0.0, 0.0], [0.0, 2.0 * R1], false),
        p.line(1, [-LINE, 0.0]),
        p.line(6, [0.0, -LINE]),
    ]
}

#[test]
fn two_arcs_tangent_with_equal_radii_tie_to_edge_id() {
    for p in places() {
        for nudge in [0.0, p.unit_tolerance() / 8.0] {
            let ends = equal_radii(&p, nudge);
            assert_cyclic(&ends, &[1, 6, 4, 9], &format!("{p:?} nudge {nudge}"));
        }
    }
}

/// At 1e±300 with a tolerance relative to the sketch, the order is the
/// one at sketch scales: the frame keeps every product in range.
#[test]
fn the_order_holds_near_f64s_extremes() {
    for scale in [1e-300, 1e300] {
        for &rotation in &ROTATIONS {
            for shift in [0.0, 10.0] {
                let mut p = Place::at(rotation, scale, shift);
                p.tolerance = tolerance::LENGTH_RELATIVE * SIZE * scale;
                let what = format!("{p:?}");
                let (ends, want) = lines(&p, true);
                assert_cyclic(&ends, &want, &what);
                assert_cyclic(&tangent_east(&p, 0.0), &[4, 3, 1, 2], &what);
                assert_cyclic(&two_radii(&p, 0.0), &[3, 4, 2, 1], &what);
                assert_cyclic(&equal_radii(&p, 0.0), &[1, 6, 4, 9], &what);
            }
        }
    }
}

/// Below `LENGTH_FLOOR` (ADR-0003) the sketch's length tolerance is the
/// floor, larger than the sketch: ends leaving within a quarter turn of one
/// another all tie, and go by id. Ends further apart still go by direction.
#[test]
fn below_the_length_floor_tangent_ends_go_by_id() {
    for &rotation in &ROTATIONS {
        let p = Place::at(rotation, 1e-300, 0.0);
        assert_eq!(p.tolerance, tolerance::LENGTH_FLOOR);
        let what = format!("{p:?}");
        let (ends, want) = lines(&p, false);
        assert_cyclic(&ends, &want, &what);
        // Nudged a thousandth of the construction: far below the floor,
        // and enough that the exact directions alone would order otherwise.
        for nudge in [0.0, 1e-3] {
            assert_cyclic(&tangent_east(&p, nudge), &[4, 1, 2, 3], &what);
            assert_cyclic(&two_radii(&p, nudge), &[3, 1, 2, 4], &what);
        }
    }
}

/// One end alone, and a join of two, come back whole.
#[test]
fn small_joins() {
    let p = Place::at(0.0, 1.0, 0.0);
    assert_eq!(order(&[]), Vec::<usize>::new());
    assert_eq!(order(&[p.line(1, [LINE, 0.0])]), vec![0]);
    let pair = [p.line(1, [LINE, 0.0]), p.line(2, [0.0, LINE])];
    assert_cyclic(&pair, &[1, 2], "two lines");
    // Two ends that tie: one run of two, by curvature.
    let tangent = [
        p.arc(2, [0.0, R1], [0.0, 0.0], [R1, R1], false),
        p.line(1, [LINE, 0.0]),
    ];
    assert_cyclic(&tangent, &[1, 2], "a tangent pair");
}

/// An end of zero length leaves east and keeps the order total.
#[test]
fn a_zero_length_end_leaves_east() {
    let p = Place::at(0.0, 1.0, 0.0);
    let ends = [
        p.line(1, [0.0, LINE]),
        p.line(2, [0.0, 0.0]),
        p.line(3, [0.0, -LINE]),
        p.arc(4, [0.0, 0.0], [0.0, 0.0], [0.0, 0.0], false),
    ];
    assert_cyclic(&ends, &[3, 2, 4, 1], "zero length");
}

/// The exact turn test tells apart directions one ulp apart, where the
/// two products round alike.
#[test]
fn turn_is_exact_where_the_products_round_alike() {
    let a = [1.0 + f64::EPSILON, 1.0];
    let b = [1.0, 1.0 - f64::EPSILON / 2.0];
    // a₀b₁ = (1 + ε)(1 − ε/2) = 1 + ε/2 − ε²/2, rounds to 1; a₁b₀ = 1.
    assert_eq!(a[0] * b[1], a[1] * b[0]);
    assert_eq!(turn(a, b), Ordering::Greater);
    assert_eq!(turn(b, a), Ordering::Less);
    assert_eq!(turn(a, a), Ordering::Equal);
}
