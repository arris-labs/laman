//! The equations of a constraint, and an arc's own, at any coordinates:
//! each one's value, its unit and its analytic gradient.
//!
//! [`equations`] evaluates a constraint of a [`Sketch`] (`docs/DATA-MODEL.md`
//! §Constraints lists the equations and their order); [`intrinsic`] the one
//! an arc carries (`|e − c| − |s − c|`). Both read the coordinates from a
//! [`Coordinates`] map and fall back to the sketch's own for any a map
//! lacks, so they are total: `None` says only that the id names nothing of
//! the right kind, and no input panics.
//!
//! A gradient is a short list of `((EntityId, Coordinate), f64)` terms over
//! the parameters the equation reads, **merged** (two lines sharing a point
//! add into one term per parameter) and **sorted** by parameter, so the
//! order is the same on every target. Fixed parameters have terms too; the
//! caller drops their columns.
//!
//! # Exact degeneracy
//!
//! Where an equation divides by a length that is exactly zero (`p = q` for
//! `distance`, an arc end on its centre), the undefined quotient is taken
//! as `0`, the undefined terms are left out, the value stays finite and
//! [`Equation::is_degenerate`] says so. Never NaN or infinity on finite
//! input. Nearly zero is not degenerate: the equation is evaluated as it
//! stands.

use crate::model::{
    Constraint, ConstraintId, Coordinate, Coordinates, Entity, EntityId, Point, Sketch, Span,
};

/// One parameter of the sketch: an entity and which of its coordinates.
pub type Param = (EntityId, Coordinate);

/// The most terms an [`Equation`] holds: the widest equations read four
/// points (`symmetric`, `angle`).
pub const MAX_TERMS: usize = 8;

/// The most equations a constraint contributes.
pub const MAX_EQUATIONS: usize = 2;

/// What an equation's value is measured in, hence which tolerance reads it
/// (`laman::model::tolerance`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    /// A length: compared with `tolerance::LENGTH`.
    Length,
    /// Dimensionless (`parallel`, `perpendicular`, `angle`): compared with
    /// `tolerance::ANGLE`.
    Ratio,
}

/// One equation `f = 0`: its value, unit and gradient at some coordinates.
///
/// ```
/// use laman::model::Sketch;
/// use laman::residual::{equations, Unit};
///
/// let mut sk = Sketch::new();
/// let (p, q) = (sk.point(0.0, 0.0)?, sk.point(3.0, 4.0)?);
/// let id = sk.distance(p, q, 2.0)?;
/// let eqs = equations(&sk, &sk.coordinates(), id).unwrap();
/// let eq = &eqs.as_slice()[0];
/// assert_eq!((eq.value(), eq.unit()), (3.0, Unit::Length));
/// assert!(!eq.is_degenerate());
/// # Ok::<(), laman::model::ModelError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Equation {
    value: f64,
    unit: Unit,
    degenerate: bool,
    terms: [(Param, f64); MAX_TERMS],
    len: usize,
}

const NO_TERM: (Param, f64) = ((EntityId(0), Coordinate::X), 0.0);

impl Equation {
    const EMPTY: Equation = Equation {
        value: 0.0,
        unit: Unit::Length,
        degenerate: false,
        terms: [NO_TERM; MAX_TERMS],
        len: 0,
    };

    /// Builds an equation from raw gradient contributions, merging those of
    /// one parameter (summed in the order given) and sorting by parameter.
    /// More than [`MAX_TERMS`] distinct parameters cannot arise from the
    /// constraints laman has; a further one would be dropped.
    fn new(value: f64, unit: Unit, degenerate: bool, raw: &Raw) -> Equation {
        let mut eq = Equation {
            value,
            unit,
            degenerate,
            ..Equation::EMPTY
        };
        for &(key, g) in raw.as_slice() {
            let at = eq.terms.iter().take(eq.len).position(|(k, _)| *k >= key);
            match at {
                Some(i) if eq.terms.get(i).is_some_and(|(k, _)| *k == key) => {
                    if let Some(term) = eq.terms.get_mut(i) {
                        term.1 += g;
                    }
                }
                _ if eq.len == MAX_TERMS => {}
                _ => {
                    let i = at.unwrap_or(eq.len);
                    eq.terms.copy_within(i..eq.len, i + 1);
                    if let Some(slot) = eq.terms.get_mut(i) {
                        *slot = (key, g);
                    }
                    eq.len += 1;
                }
            }
        }
        eq
    }

    /// The equation's value at the coordinates; zero when satisfied.
    pub fn value(&self) -> f64 {
        self.value
    }

    /// What the value is measured in.
    pub fn unit(&self) -> Unit {
        self.unit
    }

    /// Whether the equation was evaluated at an undefined point (exact
    /// degeneracy, in the module doc): its value is finite and the
    /// undefined gradient terms are missing.
    pub fn is_degenerate(&self) -> bool {
        self.degenerate
    }

    /// The gradient: one term per parameter the equation reads, sorted by
    /// parameter, in the order of `(EntityId, Coordinate)`.
    pub fn terms(&self) -> &[(Param, f64)] {
        self.terms.get(..self.len).unwrap_or(&[])
    }
}

/// A constraint's equations, in `docs/DATA-MODEL.md`'s order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Equations {
    items: [Equation; MAX_EQUATIONS],
    len: usize,
}

impl Equations {
    fn none() -> Equations {
        Equations {
            items: [Equation::EMPTY; MAX_EQUATIONS],
            len: 0,
        }
    }

    fn one(a: Equation) -> Equations {
        Equations {
            items: [a, Equation::EMPTY],
            len: 1,
        }
    }

    fn two(a: Equation, b: Equation) -> Equations {
        Equations {
            items: [a, b],
            len: 2,
        }
    }

    /// The equations, in order.
    pub fn as_slice(&self) -> &[Equation] {
        self.items.get(..self.len).unwrap_or(&[])
    }

    /// How many equations there are.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether there are none.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// Raw gradient contributions before they are merged.
struct Raw {
    items: [(Param, f64); 2 * MAX_TERMS],
    len: usize,
}

impl Raw {
    fn new() -> Raw {
        Raw {
            items: [NO_TERM; 2 * MAX_TERMS],
            len: 0,
        }
    }

    fn push(&mut self, entity: EntityId, coordinate: Coordinate, g: f64) {
        if let Some(slot) = self.items.get_mut(self.len) {
            *slot = ((entity, coordinate), g);
            self.len += 1;
        }
    }

    fn as_slice(&self) -> &[(Param, f64)] {
        self.items.get(..self.len).unwrap_or(&[])
    }
}

/// Reads parameters from a map, then from the sketch.
struct Reader<'a> {
    sketch: &'a Sketch,
    values: &'a Coordinates,
}

impl Reader<'_> {
    fn xy(&self, p: Point) -> Option<[f64; 2]> {
        let id = p.id();
        let own = || match self.sketch.entity(id) {
            Some(Entity::Point { x, y }) => Some([*x, *y]),
            Some(Entity::Line { .. } | Entity::Circle { .. } | Entity::Arc { .. }) | None => None,
        };
        let x = self.values.get(&(id, Coordinate::X)).copied();
        let y = self.values.get(&(id, Coordinate::Y)).copied();
        match (x, y) {
            (Some(x), Some(y)) => Some([x, y]),
            (x, y) => {
                let [ox, oy] = own()?;
                Some([x.unwrap_or(ox), y.unwrap_or(oy)])
            }
        }
    }

    /// The end points of a span.
    fn ends(&self, span: Span) -> Option<(Point, Point)> {
        match span {
            Span::Points(p, q) => Some((p, q)),
            Span::Line(l) => match self.sketch.entity(l.id())? {
                Entity::Line { a, b } => Some((*a, *b)),
                Entity::Point { .. } | Entity::Circle { .. } | Entity::Arc { .. } => None,
            },
        }
    }
}

/// `|v|`, its unit direction (zero where `|v| = 0`), and whether it was zero.
fn unit_vector(v: [f64; 2]) -> (f64, [f64; 2], bool) {
    let n = (v[0] * v[0] + v[1] * v[1]).sqrt();
    if n == 0.0 {
        (0.0, [0.0, 0.0], true)
    } else {
        (n, [v[0] / n, v[1] / n], false)
    }
}

/// `|to − from|` as an equation term: the length, and its gradient pushed
/// onto `raw`, scaled by `k`. Returns whether the vector was exactly zero
/// (the terms are then left out).
fn push_length(raw: &mut Raw, from: Point, to: Point, v: [f64; 2], k: f64) -> (f64, bool) {
    let (n, u, degenerate) = unit_vector(v);
    if !degenerate {
        raw.push(to.id(), Coordinate::X, k * u[0]);
        raw.push(to.id(), Coordinate::Y, k * u[1]);
        raw.push(from.id(), Coordinate::X, -k * u[0]);
        raw.push(from.id(), Coordinate::Y, -k * u[1]);
    }
    (n, degenerate)
}

/// The equations of constraint `id`, at `values` (a parameter `values`
/// lacks is read from the sketch), in `docs/DATA-MODEL.md`'s order.
///
/// `None`: the sketch has no such constraint. Never panics; never NaN or
/// infinite on finite input.
///
/// The point-only kinds are evaluated; the line and round kinds follow.
///
/// ```
/// use laman::model::{Coordinate, Sketch};
/// use laman::residual::equations;
///
/// let mut sk = Sketch::new();
/// let (p, q) = (sk.point(1.0, 2.0)?, sk.point(4.0, 6.0)?);
/// let id = sk.coincident(p, q)?;
/// let eqs = equations(&sk, &sk.coordinates(), id).unwrap();
/// let values: Vec<f64> = eqs.as_slice().iter().map(|e| e.value()).collect();
/// assert_eq!(values, [-3.0, -4.0]);
/// // The gradient of `x_p − x_q`: +1 on p's x, −1 on q's x.
/// assert_eq!(
///     eqs.as_slice()[0].terms(),
///     [((p.id(), Coordinate::X), 1.0), ((q.id(), Coordinate::X), -1.0)]
/// );
/// # Ok::<(), laman::model::ModelError>(())
/// ```
pub fn equations(sketch: &Sketch, values: &Coordinates, id: ConstraintId) -> Option<Equations> {
    let rd = Reader { sketch, values };
    let c = sketch.constraint(id)?;
    let length = |value: f64, raw: &Raw| Equation::new(value, Unit::Length, false, raw);
    match *c {
        Constraint::Coincident { p, q } => {
            let (a, b) = (rd.xy(p)?, rd.xy(q)?);
            let mut ex = Raw::new();
            ex.push(p.id(), Coordinate::X, 1.0);
            ex.push(q.id(), Coordinate::X, -1.0);
            let mut ey = Raw::new();
            ey.push(p.id(), Coordinate::Y, 1.0);
            ey.push(q.id(), Coordinate::Y, -1.0);
            Some(Equations::two(
                length(a[0] - b[0], &ex),
                length(a[1] - b[1], &ey),
            ))
        }
        Constraint::Fix { p, x, y } => {
            let a = rd.xy(p)?;
            let mut ex = Raw::new();
            ex.push(p.id(), Coordinate::X, 1.0);
            let mut ey = Raw::new();
            ey.push(p.id(), Coordinate::Y, 1.0);
            Some(Equations::two(length(a[0] - x, &ex), length(a[1] - y, &ey)))
        }
        Constraint::Horizontal(span) | Constraint::Vertical(span) => {
            let (a, b) = rd.ends(span)?;
            let (pa, pb) = (rd.xy(a)?, rd.xy(b)?);
            let (axis, i) = match c {
                Constraint::Horizontal(_) => (Coordinate::Y, 1),
                _ => (Coordinate::X, 0),
            };
            let mut raw = Raw::new();
            raw.push(b.id(), axis, 1.0);
            raw.push(a.id(), axis, -1.0);
            Some(Equations::one(length(pb[i] - pa[i], &raw)))
        }
        Constraint::Midpoint { p, line } => {
            let (a, b) = rd.ends(Span::Line(line))?;
            let (pp, pa, pb) = (rd.xy(p)?, rd.xy(a)?, rd.xy(b)?);
            let one = |axis: Coordinate, i: usize| {
                let mut raw = Raw::new();
                raw.push(p.id(), axis, 1.0);
                raw.push(a.id(), axis, -0.5);
                raw.push(b.id(), axis, -0.5);
                length(pp[i] - (pa[i] + pb[i]) / 2.0, &raw)
            };
            Some(Equations::two(one(Coordinate::X, 0), one(Coordinate::Y, 1)))
        }
        Constraint::Distance { p, q, value } => {
            let (pp, pq) = (rd.xy(p)?, rd.xy(q)?);
            let mut raw = Raw::new();
            let (n, degenerate) = push_length(&mut raw, p, q, [pq[0] - pp[0], pq[1] - pp[1]], 1.0);
            Some(Equations::one(Equation::new(
                n - value,
                Unit::Length,
                degenerate,
                &raw,
            )))
        }
        // The line and round kinds: the following steps of the plan.
        Constraint::PointOnLine { .. }
        | Constraint::PointOnCircle { .. }
        | Constraint::Parallel { .. }
        | Constraint::Perpendicular { .. }
        | Constraint::EqualLength { .. }
        | Constraint::EqualRadius { .. }
        | Constraint::TangentLineCircle { .. }
        | Constraint::TangentCircles { .. }
        | Constraint::Symmetric { .. }
        | Constraint::PointLineDistance { .. }
        | Constraint::Radius { .. }
        | Constraint::Angle { .. } => Some(Equations::none()),
    }
}

/// The equation an entity carries on its own: an arc's `|e − c| − |s − c|`
/// (its end as far from the centre as its start; ADR-0001). `None` for any
/// other kind of entity and for an id the sketch lacks.
///
/// ```
/// use laman::model::Sketch;
/// use laman::residual::intrinsic;
///
/// let mut sk = Sketch::new();
/// let (c, s, e) = (sk.point(0.0, 0.0)?, sk.point(3.0, 4.0)?, sk.point(0.0, 6.0)?);
/// let arc = sk.arc(c, s, e)?;
/// let eq = intrinsic(&sk, &sk.coordinates(), arc.id()).unwrap();
/// assert_eq!(eq.value(), 1.0); // |e − c| = 6, |s − c| = 5
/// assert!(intrinsic(&sk, &sk.coordinates(), c.id()).is_none());
/// # Ok::<(), laman::model::ModelError>(())
/// ```
pub fn intrinsic(sketch: &Sketch, values: &Coordinates, id: EntityId) -> Option<Equation> {
    let rd = Reader { sketch, values };
    match sketch.entity(id)? {
        Entity::Arc { c, s, e } => {
            let (pc, ps, pe) = (rd.xy(*c)?, rd.xy(*s)?, rd.xy(*e)?);
            let mut raw = Raw::new();
            let (re, de) = push_length(&mut raw, *c, *e, [pe[0] - pc[0], pe[1] - pc[1]], 1.0);
            let (rs, ds) = push_length(&mut raw, *c, *s, [ps[0] - pc[0], ps[1] - pc[1]], -1.0);
            Some(Equation::new(re - rs, Unit::Length, de || ds, &raw))
        }
        Entity::Point { .. } | Entity::Line { .. } | Entity::Circle { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests: a failing unwrap is the test failing (clippy.toml)

    use super::*;

    fn eqs(sk: &Sketch, id: ConstraintId) -> Vec<Equation> {
        equations(sk, &sk.coordinates(), id)
            .unwrap()
            .as_slice()
            .to_vec()
    }

    #[test]
    fn values_follow_the_data_model() {
        let mut sk = Sketch::new();
        let a = sk.point(1.0, 2.0).unwrap();
        let b = sk.point(5.0, 10.0).unwrap();
        let l = sk.line(a, b).unwrap();
        let m = sk.point(3.0, 7.0).unwrap();
        let v = |sk: &Sketch, id| eqs(sk, id).iter().map(Equation::value).collect::<Vec<_>>();
        assert_eq!(
            {
                let id = sk.horizontal(l).unwrap();
                v(&sk, id)
            },
            [8.0]
        );
        assert_eq!(
            {
                let id = sk.vertical((a, b)).unwrap();
                v(&sk, id)
            },
            [4.0]
        );
        assert_eq!(
            {
                let id = sk.midpoint(m, l).unwrap();
                v(&sk, id)
            },
            [0.0, 1.0]
        );
        assert_eq!(
            {
                let id = sk.fix(a, 0.0, 5.0).unwrap();
                v(&sk, id)
            },
            [1.0, -3.0]
        );
        assert_eq!(
            {
                let id = sk.distance(a, b, 4.0).unwrap();
                v(&sk, id)
            },
            [(80.0_f64).sqrt() - 4.0]
        );
    }

    #[test]
    fn a_shared_point_merges_into_one_sorted_term() {
        let mut sk = Sketch::new();
        let a = sk.point(0.0, 0.0).unwrap();
        let b = sk.point(4.0, 2.0).unwrap();
        let l = sk.line(a, b).unwrap();
        // The midpoint of a line at its own start: p and a are one point.
        let id = sk.midpoint(a, l).unwrap();
        let x = &eqs(&sk, id)[0];
        assert_eq!(
            x.terms(),
            [
                ((a.id(), Coordinate::X), 0.5),
                ((b.id(), Coordinate::X), -0.5)
            ]
        );
    }

    #[test]
    fn coincident_distance_is_degenerate_not_nan() {
        let mut sk = Sketch::new();
        let a = sk.point(1.0, 1.0).unwrap();
        let b = sk.point(2.0, 2.0).unwrap();
        let id = sk.distance(a, b, 3.0).unwrap();
        let mut at = sk.coordinates();
        at.insert((b.id(), Coordinate::X), 1.0);
        at.insert((b.id(), Coordinate::Y), 1.0);
        let eq = equations(&sk, &at, id).unwrap().as_slice()[0];
        assert!(eq.is_degenerate());
        assert_eq!(eq.value(), -3.0);
        assert!(eq.terms().is_empty());
    }

    #[test]
    fn the_map_wins_over_the_sketch_and_unknown_ids_are_none() {
        let mut sk = Sketch::new();
        let a = sk.point(0.0, 0.0).unwrap();
        let id = sk.fix(a, 0.0, 0.0).unwrap();
        let mut at = Coordinates::new();
        at.insert((a.id(), Coordinate::X), 7.0);
        let eq = equations(&sk, &at, id).unwrap();
        assert_eq!(eq.as_slice()[0].value(), 7.0);
        assert_eq!(eq.as_slice()[1].value(), 0.0);
        assert!(equations(&sk, &at, ConstraintId(99)).is_none());
        assert!(intrinsic(&sk, &at, EntityId(99)).is_none());
    }

    #[test]
    fn an_arc_end_on_its_centre_is_degenerate() {
        let mut sk = Sketch::new();
        let c = sk.point(0.0, 0.0).unwrap();
        let s = sk.point(3.0, 4.0).unwrap();
        let e = sk.point(1.0, 1.0).unwrap();
        let arc = sk.arc(c, s, e).unwrap();
        let mut at = sk.coordinates();
        at.insert((e.id(), Coordinate::X), 0.0);
        at.insert((e.id(), Coordinate::Y), 0.0);
        let eq = intrinsic(&sk, &at, arc.id()).unwrap();
        assert!(eq.is_degenerate());
        assert_eq!(eq.value(), -5.0);
        assert!(eq.terms().iter().all(|(k, _)| k.0 != e.id()));
    }
}
