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
    Constraint, ConstraintId, Contact, Coordinate, Coordinates, Entity, EntityId, Line, Point,
    Round, Sketch, Span,
};

/// One parameter of the sketch: an entity and which of its coordinates.
pub type Param = (EntityId, Coordinate);

/// The most terms an [`Equation`] holds. The widest read eight parameters:
/// `symmetric` and `angle` (four points), a `tangent` of a line and an arc
/// (the line's two ends, the centre, the start), a `tangent` of two arcs
/// (two centres, two starts).
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
                _ if eq.len == MAX_TERMS => {
                    debug_assert!(false, "an equation with more than MAX_TERMS parameters");
                }
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
        } else {
            debug_assert!(false, "more than 2 * MAX_TERMS gradient contributions");
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

/// How a circle or an arc gives its radius.
#[derive(Clone, Copy)]
enum Radius {
    /// A circle's own parameter, with its value.
    Param(EntityId, f64),
    /// An arc's: the distance from its centre to its start.
    Start(Point),
}

impl Reader<'_> {
    /// The centre of a circle or arc, and where its radius comes from.
    fn round(&self, round: Round) -> Option<(Point, Radius)> {
        let id = round.id();
        match self.sketch.entity(id)? {
            Entity::Circle { c, r } => {
                let r = self.values.get(&(id, Coordinate::R)).copied().unwrap_or(*r);
                Some((*c, Radius::Param(id, r)))
            }
            Entity::Arc { c, s, .. } => Some((*c, Radius::Start(*s))),
            Entity::Point { .. } | Entity::Line { .. } => None,
        }
    }
}

/// A radius as an equation term: its value, and its gradient pushed onto
/// `raw` scaled by `k`; whether an arc's start sat exactly on its centre.
fn push_radius(
    rd: &Reader<'_>,
    raw: &mut Raw,
    centre: Point,
    radius: Radius,
    k: f64,
) -> Option<(f64, bool)> {
    match radius {
        Radius::Param(id, value) => {
            raw.push(id, Coordinate::R, k);
            Some((value, false))
        }
        Radius::Start(s) => Some(push_length(
            raw,
            centre,
            s,
            diff(rd.xy(centre)?, rd.xy(s)?),
            k,
        )),
    }
}

/// `|v|`, its unit direction (zero where `|v| = 0`), and whether it was zero.
fn unit_vector(v: [f64; 2]) -> (f64, [f64; 2], bool) {
    // Scaled by the larger component so the squares cannot overflow (or
    // underflow to zero): |v| = m * |v / m| with |v / m| in [1, sqrt 2].
    let m = v[0].abs().max(v[1].abs());
    if m == 0.0 {
        return (0.0, [0.0, 0.0], true);
    }
    let (sx, sy) = (v[0] / m, v[1] / m);
    let s = (sx * sx + sy * sy).sqrt();
    (m * s, [sx / s, sy / s], false)
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

/// The signed distance `cross(u, w) / |u|` from the infinite line `a → b`
/// of the point at `at`, with its gradient pushed onto `raw`. The point is
/// one parameter pair per `(Point, weight)` of `holders` (a single point
/// with weight 1, or the two ends of a midpoint with weight 1/2 each);
/// `pa`, `pb` are the line's end coordinates. A zero-length line has value
/// `0`, no terms, and is reported degenerate.
fn push_side(
    raw: &mut Raw,
    (holders, at): (&[(Point, f64)], [f64; 2]),
    (a, pa): (Point, [f64; 2]),
    (b, pb): (Point, [f64; 2]),
) -> (f64, bool) {
    let u = diff(pa, pb);
    let w = diff(pa, at);
    let (n, e, degenerate) = unit_vector(u);
    if degenerate {
        return (0.0, true);
    }
    // cross(u, w) / |u| = cross(e, w), without the product u * w that can
    // overflow where the quotient is representable.
    let f = e[0] * w[1] - e[1] * w[0];
    // d cross / d(point, a, b) over |u|, then the quotient rule with
    // d|u| = ±e.
    let dp = [-e[1], e[0]];
    let da = [-w[1] / n + e[1], w[0] / n - e[0]];
    let db = [w[1] / n, -w[0] / n];
    for (i, axis) in [Coordinate::X, Coordinate::Y].into_iter().enumerate() {
        for &(holder, weight) in holders {
            raw.push(holder.id(), axis, weight * dp[i]);
        }
        raw.push(a.id(), axis, da[i] + f * e[i] / n);
        raw.push(b.id(), axis, db[i] - f * e[i] / n);
    }
    (f, false)
}

/// `to − from`.
fn diff(from: [f64; 2], to: [f64; 2]) -> [f64; 2] {
    [to[0] - from[0], to[1] - from[1]]
}

/// The equation `(A cross(u1, u2) + B dot(u1, u2)) / (|u1| |u2|)` of two
/// lines, dimensionless (`parallel`, `perpendicular`, `angle`).
fn ratio_of_lines(rd: &Reader<'_>, l1: Line, l2: Line, coeffs: (f64, f64)) -> Option<Equations> {
    let (first, second) = (rd.ends(Span::Line(l1))?, rd.ends(Span::Line(l2))?);
    let at = [
        rd.xy(first.0)?,
        rd.xy(first.1)?,
        rd.xy(second.0)?,
        rd.xy(second.1)?,
    ];
    let mut raw = Raw::new();
    let (f, degenerate) = quotient_of_lines(&mut raw, first, second, at, coeffs);
    Some(Equations::one(Equation::new(
        f,
        Unit::Ratio,
        degenerate,
        &raw,
    )))
}

/// `(A cross(u1, u2) + B dot(u1, u2)) / (|u1| |u2|)` over lines `a1 → b1`
/// and `a2 → b2`, with its gradient. A zero-length line: value `0`, no terms.
fn quotient_of_lines(
    raw: &mut Raw,
    first: (Point, Point),
    second: (Point, Point),
    at: [[f64; 2]; 4],
    (ca, cb): (f64, f64),
) -> (f64, bool) {
    let u1 = [at[1][0] - at[0][0], at[1][1] - at[0][1]];
    let u2 = [at[3][0] - at[2][0], at[3][1] - at[2][1]];
    let ((n1, e1, d1), (n2, e2, d2)) = (unit_vector(u1), unit_vector(u2));
    if d1 || d2 {
        return (0.0, true);
    }
    // On the unit directions, so no product of two lengths can overflow.
    let cross = e1[0] * e2[1] - e1[1] * e2[0];
    let dot = e1[0] * e2[0] + e1[1] * e2[1];
    let f = ca * cross + cb * dot;
    let g1 = [
        (ca * e2[1] + cb * e2[0] - f * e1[0]) / n1,
        (-ca * e2[0] + cb * e2[1] - f * e1[1]) / n1,
    ];
    let g2 = [
        (-ca * e1[1] + cb * e1[0] - f * e2[0]) / n2,
        (ca * e1[0] + cb * e1[1] - f * e2[1]) / n2,
    ];
    for (i, axis) in [Coordinate::X, Coordinate::Y].into_iter().enumerate() {
        raw.push(first.1.id(), axis, g1[i]);
        raw.push(first.0.id(), axis, -g1[i]);
        raw.push(second.1.id(), axis, g2[i]);
        raw.push(second.0.id(), axis, -g2[i]);
    }
    (f, false)
}

/// `horizontal` / `vertical`: the span's ends agree on `axis`, whose index
/// in an `[x, y]` pair is the second element.
fn axis_aligned(rd: &Reader<'_>, span: Span, (axis, i): (Coordinate, usize)) -> Option<Equations> {
    let (a, b) = rd.ends(span)?;
    let (pa, pb) = (rd.xy(a)?, rd.xy(b)?);
    let mut raw = Raw::new();
    raw.push(b.id(), axis, 1.0);
    raw.push(a.id(), axis, -1.0);
    Some(Equations::one(Equation::new(
        pb[i] - pa[i],
        Unit::Length,
        false,
        &raw,
    )))
}

/// The equations of constraint `id`, at `values` (a parameter `values`
/// lacks is read from the sketch), in `docs/DATA-MODEL.md`'s order.
///
/// `None`: the sketch has no such constraint. Never panics; never NaN or
/// infinite on finite input.
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
        Constraint::Horizontal(span) => axis_aligned(&rd, span, (Coordinate::Y, 1)),
        Constraint::Vertical(span) => axis_aligned(&rd, span, (Coordinate::X, 0)),
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
        Constraint::PointOnLine { p, line } => {
            let (a, b) = rd.ends(Span::Line(line))?;
            let mut raw = Raw::new();
            let (f, degenerate) = push_side(
                &mut raw,
                (&[(p, 1.0)], rd.xy(p)?),
                (a, rd.xy(a)?),
                (b, rd.xy(b)?),
            );
            Some(Equations::one(Equation::new(
                f,
                Unit::Length,
                degenerate,
                &raw,
            )))
        }
        Constraint::PointLineDistance {
            p,
            line,
            value,
            sense,
        } => {
            let (a, b) = rd.ends(Span::Line(line))?;
            let mut raw = Raw::new();
            let (f, degenerate) = push_side(
                &mut raw,
                (&[(p, 1.0)], rd.xy(p)?),
                (a, rd.xy(a)?),
                (b, rd.xy(b)?),
            );
            let f = f - sense.sign() * value;
            Some(Equations::one(Equation::new(
                f,
                Unit::Length,
                degenerate,
                &raw,
            )))
        }
        Constraint::Parallel { a, b } => ratio_of_lines(&rd, a, b, (1.0, 0.0)),
        Constraint::Perpendicular { a, b } => ratio_of_lines(&rd, a, b, (0.0, 1.0)),
        Constraint::Angle { a, b, value } => {
            ratio_of_lines(&rd, a, b, (libm::cos(value), -libm::sin(value)))
        }
        Constraint::EqualLength { a, b } => {
            let ((a1, b1), (a2, b2)) = (rd.ends(Span::Line(a))?, rd.ends(Span::Line(b))?);
            let mut raw = Raw::new();
            let (n1, d1) = push_length(&mut raw, a1, b1, diff(rd.xy(a1)?, rd.xy(b1)?), 1.0);
            let (n2, d2) = push_length(&mut raw, a2, b2, diff(rd.xy(a2)?, rd.xy(b2)?), -1.0);
            Some(Equations::one(Equation::new(
                n1 - n2,
                Unit::Length,
                d1 || d2,
                &raw,
            )))
        }
        Constraint::Symmetric { p, q, axis } => {
            let (a, b) = rd.ends(Span::Line(axis))?;
            let (pp, pq, pa, pb) = (rd.xy(p)?, rd.xy(q)?, rd.xy(a)?, rd.xy(b)?);
            let m = [(pp[0] + pq[0]) / 2.0, (pp[1] + pq[1]) / 2.0];
            // The midpoint of p and q on the axis.
            let mut on = Raw::new();
            let (f_on, d_on) = push_side(&mut on, (&[(p, 0.5), (q, 0.5)], m), (a, pa), (b, pb));
            // q − p perpendicular to the axis: dot(u, q − p) / |u|.
            let u = diff(pa, pb);
            let r = diff(pp, pq);
            let (n, e, d_perp) = unit_vector(u);
            let mut perp = Raw::new();
            let mut f_perp = 0.0;
            if !d_perp {
                f_perp = e[0] * r[0] + e[1] * r[1];
                for (i, axis) in [Coordinate::X, Coordinate::Y].into_iter().enumerate() {
                    let gu = (r[i] - f_perp * e[i]) / n;
                    perp.push(q.id(), axis, e[i]);
                    perp.push(p.id(), axis, -e[i]);
                    perp.push(b.id(), axis, gu);
                    perp.push(a.id(), axis, -gu);
                }
            }
            Some(Equations::two(
                Equation::new(f_on, Unit::Length, d_on, &on),
                Equation::new(f_perp, Unit::Length, d_perp, &perp),
            ))
        }
        Constraint::PointOnCircle { p, on } => {
            let (c, radius) = rd.round(on)?;
            let mut raw = Raw::new();
            let (n, d1) = push_length(&mut raw, c, p, diff(rd.xy(c)?, rd.xy(p)?), 1.0);
            let (r, d2) = push_radius(&rd, &mut raw, c, radius, -1.0)?;
            Some(Equations::one(Equation::new(
                n - r,
                Unit::Length,
                d1 || d2,
                &raw,
            )))
        }
        Constraint::EqualRadius { a, b } => {
            let ((ca, ra), (cb, rb)) = (rd.round(a)?, rd.round(b)?);
            let mut raw = Raw::new();
            let (x, d1) = push_radius(&rd, &mut raw, ca, ra, 1.0)?;
            let (y, d2) = push_radius(&rd, &mut raw, cb, rb, -1.0)?;
            Some(Equations::one(Equation::new(
                x - y,
                Unit::Length,
                d1 || d2,
                &raw,
            )))
        }
        Constraint::Radius { round, value } => {
            let (c, radius) = rd.round(round)?;
            let mut raw = Raw::new();
            let (r, degenerate) = push_radius(&rd, &mut raw, c, radius, 1.0)?;
            Some(Equations::one(Equation::new(
                r - value,
                Unit::Length,
                degenerate,
                &raw,
            )))
        }
        Constraint::TangentLineCircle { line, round, sense } => {
            let (a, b) = rd.ends(Span::Line(line))?;
            let (c, radius) = rd.round(round)?;
            let mut raw = Raw::new();
            let (side, d1) = push_side(
                &mut raw,
                (&[(c, 1.0)], rd.xy(c)?),
                (a, rd.xy(a)?),
                (b, rd.xy(b)?),
            );
            let (r, d2) = push_radius(&rd, &mut raw, c, radius, -sense.sign())?;
            Some(Equations::one(Equation::new(
                side - sense.sign() * r,
                Unit::Length,
                d1 || d2,
                &raw,
            )))
        }
        Constraint::TangentCircles { a, b, contact } => {
            let ((ca, ra), (cb, rb)) = (rd.round(a)?, rd.round(b)?);
            let mut raw = Raw::new();
            let (n, d1) = push_length(&mut raw, cb, ca, diff(rd.xy(cb)?, rd.xy(ca)?), 1.0);
            // external: r_a + r_b; internal: σ (r_a − r_b).
            let (ka, kb) = match contact {
                Contact::External => (-1.0, -1.0),
                Contact::Internal(sigma) => (-sigma.sign(), sigma.sign()),
            };
            let (x, d2) = push_radius(&rd, &mut raw, ca, ra, ka)?;
            let (y, d3) = push_radius(&rd, &mut raw, cb, rb, kb)?;
            Some(Equations::one(Equation::new(
                n + ka * x + kb * y,
                Unit::Length,
                d1 || d2 || d3,
                &raw,
            )))
        }
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
    fn line_kinds_follow_the_data_model() {
        let mut sk = Sketch::new();
        let o = sk.point(0.0, 0.0).unwrap();
        let a = sk.point(2.0, 0.0).unwrap();
        let b = sk.point(0.0, 3.0).unwrap();
        let (l1, l2) = (sk.line(o, a).unwrap(), sk.line(o, b).unwrap());
        let v = |sk: &Sketch, id| eqs(sk, id).iter().map(Equation::value).collect::<Vec<_>>();
        let id = sk.perpendicular(l1, l2).unwrap();
        assert_eq!(v(&sk, id), [0.0]);
        let id = sk.parallel(l1, l2).unwrap();
        assert_eq!(v(&sk, id), [1.0]); // cross(u1, u2) / (|u1||u2|) = sin 90°
        let id = sk.equal_length(l1, l2).unwrap();
        assert_eq!(v(&sk, id), [-1.0]);
        let id = sk.angle(l1, l2, std::f64::consts::FRAC_PI_2).unwrap();
        assert!(v(&sk, id)[0].abs() < 1e-15);
        let id = sk.point_on_line(b, l1).unwrap();
        assert_eq!(v(&sk, id), [3.0]); // left of o → a: positive
        let id = sk.point_line_distance(b, l1, 1.0).unwrap();
        assert_eq!(v(&sk, id), [2.0]);
        // b mirrored across l1 is (0, −3); the midpoint (0, 0) is on it.
        let m = sk.point(0.0, -3.0).unwrap();
        let id = sk.symmetric(b, m, l1).unwrap();
        assert_eq!(v(&sk, id), [0.0, 0.0]); // q − p = (0, −6) ⟂ u
    }

    #[test]
    fn a_zero_length_line_is_degenerate_not_nan() {
        let mut sk = Sketch::new();
        let (o, a, p) = (
            sk.point(0.0, 0.0).unwrap(),
            sk.point(1.0, 0.0).unwrap(),
            sk.point(1.0, 1.0).unwrap(),
        );
        let l = sk.line(o, a).unwrap();
        let id = sk.point_on_line(p, l).unwrap();
        let mut at = sk.coordinates();
        at.insert((a.id(), Coordinate::X), 0.0);
        let eq = equations(&sk, &at, id).unwrap().as_slice()[0];
        assert!(eq.is_degenerate());
        assert_eq!(eq.value(), 0.0);
        assert!(eq.terms().is_empty());
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
    fn round_kinds_follow_the_data_model() {
        let mut sk = Sketch::new();
        let (c1, c2) = (sk.point(0.0, 0.0).unwrap(), sk.point(5.0, 0.0).unwrap());
        let (x, y) = (sk.circle(c1, 2.0).unwrap(), sk.circle(c2, 1.0).unwrap());
        let v = |sk: &Sketch, id| eqs(sk, id).iter().map(Equation::value).collect::<Vec<_>>();
        let id = sk.tangent_circles(x, y).unwrap(); // external: 5 - 3
        assert_eq!(v(&sk, id), [2.0]);
        let id = sk.equal_radius(x, y).unwrap();
        assert_eq!(v(&sk, id), [1.0]);
        let id = sk.radius(x, 0.5).unwrap();
        assert_eq!(v(&sk, id), [1.5]);
        let p = sk.point(0.0, 3.0).unwrap();
        let id = sk.point_on_circle(p, x).unwrap();
        assert_eq!(v(&sk, id), [1.0]);
        let (a, b) = (sk.point(-1.0, 4.0).unwrap(), sk.point(1.0, 4.0).unwrap());
        let l = sk.line(a, b).unwrap();
        // The centre is below the line (negative side), 4 away; radius 2.
        let id = sk.tangent_line_circle(l, x).unwrap();
        assert_eq!(v(&sk, id), [-4.0 + 2.0]);
    }

    #[test]
    fn the_widest_equations_fit_their_capacity() {
        // Two arcs on eight distinct points, and a line with an arc: the
        // widest `tangent`s read exactly MAX_TERMS parameters, none dropped.
        let mut sk = Sketch::new();
        let arc = |sk: &mut Sketch, x: f64| {
            let c = sk.point(x, 0.0).unwrap();
            let s = sk.point(x + 1.0, 0.0).unwrap();
            let e = sk.point(x, 1.0).unwrap();
            sk.arc(c, s, e).unwrap()
        };
        let (a, b) = (arc(&mut sk, 0.0), arc(&mut sk, 5.0));
        let id = sk.tangent_circles(a, b).unwrap();
        assert_eq!(eqs(&sk, id)[0].terms().len(), MAX_TERMS);
        let (p, q) = (sk.point(0.0, 3.0).unwrap(), sk.point(1.0, 3.0).unwrap());
        let l = sk.line(p, q).unwrap();
        let id = sk.tangent_line_circle(l, a).unwrap();
        assert_eq!(eqs(&sk, id)[0].terms().len(), MAX_TERMS);
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

    /// Squaring a coordinate difference of 1e200 overflows; the equations
    /// of a sketch at that scale stay finite, values and gradients.
    #[test]
    fn huge_finite_coordinates_stay_finite() {
        const BIG: f64 = 1e200;
        let mut sk = Sketch::new();
        let o = sk.point(0.0, 0.0).unwrap();
        let a = sk.point(BIG, 0.0).unwrap();
        let b = sk.point(0.0, BIG).unwrap();
        let c = sk.point(BIG, BIG).unwrap();
        let m = sk.point(BIG / 2.0, BIG / 4.0).unwrap();
        let l1 = sk.line(o, a).unwrap();
        let l2 = sk.line(o, b).unwrap();
        let l3 = sk.line(b, c).unwrap();
        let circle = sk.circle(m, BIG / 8.0).unwrap();
        let ids = [
            sk.distance(o, a, BIG).unwrap(),
            sk.point_on_line(m, l1).unwrap(),
            sk.point_line_distance(m, l1, BIG / 4.0).unwrap(),
            sk.parallel(l1, l3).unwrap(),
            sk.perpendicular(l1, l2).unwrap(),
            sk.angle(l1, l2, 1.0).unwrap(),
            sk.tangent_line_circle(l1, circle).unwrap(),
            sk.equal_length(l1, l2).unwrap(),
            sk.midpoint(m, l1).unwrap(),
            sk.symmetric(o, b, l1).unwrap(),
        ];
        for id in ids {
            for eq in eqs(&sk, id) {
                assert!(eq.value().is_finite(), "{:?}", sk.constraint(id));
                assert!(!eq.is_degenerate());
                for (_, g) in eq.terms() {
                    assert!(g.is_finite(), "{:?}", sk.constraint(id));
                }
            }
        }
        // Known values: |oa| = 1e200, m is BIG/4 above l1, l1 ⟂ l2.
        assert_eq!(eqs(&sk, ids[0])[0].value(), 0.0);
        assert_eq!(eqs(&sk, ids[1])[0].value(), BIG / 4.0);
        assert_eq!(eqs(&sk, ids[4])[0].value(), 0.0);
    }
}
