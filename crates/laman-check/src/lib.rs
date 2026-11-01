//! `laman-check`: an independent check of a sketch at given coordinates.
//!
//! [`check`] re-measures every constraint and every arc's intrinsic equation
//! from the definitions in `docs/DATA-MODEL.md` and reports what is off by
//! more than the tolerance. It measures what a constraint *means* (a
//! distance as a distance, tangency as the distance from a centre to a line
//! against the radius, an angle by `atan2`) and shares no code with the
//! solver's residuals: it names only `laman::model`, which
//! `tools/check-layers.sh` enforces.
//!
//! ```
//! use laman::model::Sketch;
//! use laman_check::check;
//!
//! let mut sk = Sketch::new();
//! let a = sk.point(0.0, 0.0)?;
//! let b = sk.point(3.0, 4.0)?;
//! sk.distance(a, b, 5.0)?;
//! assert!(check(&sk, &sk.coordinates()).is_clean());
//!
//! // Move `b` and the distance is off.
//! let mut moved = sk.coordinates();
//! moved.insert((b.id(), laman::model::Coordinate::X), 6.0);
//! let report = check(&sk, &moved);
//! assert_eq!(report.violations.len(), 1);
//! # Ok::<(), laman::model::ModelError>(())
//! ```
#![forbid(unsafe_code)]

use laman::model::tolerance::{ANGLE, Tolerances};
use laman::model::{
    Constraint, ConstraintId, ConstraintKind, Contact, Coordinate, Coordinates, Entity, EntityId,
    EntityKind, Line, Point, Round, Sketch, Span,
};
use std::f64::consts::PI;

/// What a [`Violation`] is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Source {
    /// A constraint of the sketch.
    Constraint(ConstraintId),
    /// An arc's intrinsic equation: its end is as far from the centre as
    /// its start.
    Arc(EntityId),
}

/// The kind of thing a [`Violation`] is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Subject {
    /// A constraint of this kind.
    Constraint(ConstraintKind),
    /// An arc's intrinsic equation.
    ArcIntrinsic,
}

/// One equation that does not hold at the checked coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct Violation {
    /// The constraint, or the arc, that is off.
    pub id: Source,
    /// The caller's tag on it, if any.
    pub tag: Option<String>,
    /// What kind of constraint (or the arc equation) it is.
    pub kind: Subject,
    /// The value measured: a length, or an angle in radians.
    pub measured: f64,
    /// The value it should have had, in the same unit.
    pub wanted: f64,
}

/// An entity the checker flags because an equation involving it is
/// ill-conditioned, even when every constraint holds.
#[derive(Debug, Clone, PartialEq)]
pub struct Degenerate {
    /// The entity.
    pub id: EntityId,
    /// The caller's tag on it, if any.
    pub tag: Option<String>,
    /// Its kind.
    pub kind: EntityKind,
    /// How it is degenerate.
    pub what: Degeneracy,
    /// The length that is too small, in the caller's units.
    pub size: f64,
}

/// How an entity is degenerate at the checked coordinates (length below
/// the component-relative length tolerance).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Degeneracy {
    /// A line whose end points coincide: its direction, and every equation
    /// that divides by its length, is undefined.
    ZeroLengthLine,
    /// A circle of radius about zero: tangency and point-on-circle lose
    /// their meaning.
    ZeroRadius,
    /// An arc whose centre meets its start: its radius is zero.
    ZeroRadiusArc,
    /// An arc whose end meets its start: its sweep is empty or full.
    EmptyArc,
}

/// A parameter the coordinates did not hold, so what depends on it could
/// not be measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Missing {
    /// The entity.
    pub id: EntityId,
    /// Its parameter, or `None` when the entity is not in the sketch.
    pub coordinate: Option<Coordinate>,
}

/// The result of [`check`]: everything off, in id order (constraints, then
/// arcs).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Report {
    /// Equations that do not hold.
    pub violations: Vec<Violation>,
    /// Degenerate entities, in id order.
    pub degenerate: Vec<Degenerate>,
    /// Parameters the coordinates lacked. A constraint depending on one is
    /// neither confirmed nor violated.
    pub missing: Vec<Missing>,
}

impl Report {
    /// True when nothing is violated, degenerate or missing.
    ///
    /// ```
    /// use laman_check::Report;
    /// assert!(Report::default().is_clean());
    /// ```
    pub fn is_clean(&self) -> bool {
        self.violations.is_empty() && self.degenerate.is_empty() && self.missing.is_empty()
    }
}

/// What one equation measured, and how far from right that is.
struct Measure {
    measured: f64,
    wanted: f64,
    /// How far off, in the unit of `tolerance`.
    deviation: f64,
    tolerance: f64,
}

impl Measure {
    /// A length against the length it should be.
    fn length(measured: f64, wanted: f64, tolerance: f64) -> Measure {
        Measure {
            measured,
            wanted,
            deviation: (measured - wanted).abs(),
            tolerance,
        }
    }

    /// An angle against the one it should be, `deviation` already folded
    /// by the caller to the symmetry of the constraint.
    fn angle(measured: f64, wanted: f64, deviation: f64) -> Measure {
        Measure {
            measured,
            wanted,
            deviation,
            tolerance: ANGLE,
        }
    }

    /// `NaN` is never within tolerance.
    fn holds(&self) -> bool {
        self.deviation <= self.tolerance
    }
}

/// A point's position as `(x, y)`.
type Xy = (f64, f64);

/// Reads the sketch's shape and the caller's parameters.
struct View<'a> {
    sketch: &'a Sketch,
    values: &'a Coordinates,
    tolerance: f64,
}

impl View<'_> {
    fn value(&self, id: EntityId, coordinate: Coordinate) -> Result<f64, Missing> {
        self.values.get(&(id, coordinate)).copied().ok_or(Missing {
            id,
            coordinate: Some(coordinate),
        })
    }

    fn at(&self, p: Point) -> Result<Xy, Missing> {
        Ok((
            self.value(p.id(), Coordinate::X)?,
            self.value(p.id(), Coordinate::Y)?,
        ))
    }

    /// A line's end points, start then end.
    fn ends(&self, l: Line) -> Result<(Xy, Xy), Missing> {
        match self.sketch.entity(l.id()) {
            Some(Entity::Line { a, b }) => Ok((self.at(*a)?, self.at(*b)?)),
            Some(Entity::Point { .. } | Entity::Circle { .. } | Entity::Arc { .. }) | None => {
                Err(Missing {
                    id: l.id(),
                    coordinate: None,
                })
            }
        }
    }

    /// A circle's or arc's centre and radius; an arc's radius is its
    /// centre-to-start distance.
    fn round(&self, x: Round) -> Result<(Xy, f64), Missing> {
        match (x, self.sketch.entity(x.id())) {
            (Round::Circle(c), Some(Entity::Circle { c: centre, .. })) => {
                Ok((self.at(*centre)?, self.value(c.id(), Coordinate::R)?))
            }
            (Round::Arc(_), Some(Entity::Arc { c, s, .. })) => {
                let (centre, start) = (self.at(*c)?, self.at(*s)?);
                Ok((centre, dist(centre, start)))
            }
            (
                Round::Circle(_) | Round::Arc(_),
                Some(
                    Entity::Point { .. }
                    | Entity::Line { .. }
                    | Entity::Circle { .. }
                    | Entity::Arc { .. },
                )
                | None,
            ) => Err(Missing {
                id: x.id(),
                coordinate: None,
            }),
        }
    }

    fn two_points(&self, span: Span) -> Result<(Xy, Xy), Missing> {
        match span {
            Span::Line(l) => self.ends(l),
            Span::Points(p, q) => Ok((self.at(p)?, self.at(q)?)),
        }
    }
}

/// Euclidean distance.
fn dist(p: Xy, q: Xy) -> f64 {
    let (dx, dy) = (q.0 - p.0, q.1 - p.1);
    libm::hypot(dx, dy)
}

fn sub(p: Xy, q: Xy) -> Xy {
    (p.0 - q.0, p.1 - q.1)
}

fn dot(v: Xy, w: Xy) -> f64 {
    v.0 * w.0 + v.1 * w.1
}

fn cross(v: Xy, w: Xy) -> f64 {
    v.0 * w.1 - v.1 * w.0
}

/// The foot of the perpendicular from `p` to the infinite line `a`–`b`, by
/// projection; `NaN` for a zero-length line.
fn foot(p: Xy, (a, b): (Xy, Xy)) -> Xy {
    let u = sub(b, a);
    let t = dot(sub(p, a), u) / dot(u, u);
    (a.0 + t * u.0, a.1 + t * u.1)
}

/// Distance from `p` to the infinite line, signed `+` on the left of
/// `a → b` and `+` for a point exactly on it, as the model's senses are.
fn signed_distance(p: Xy, line: (Xy, Xy)) -> f64 {
    let d = dist(p, foot(p, line));
    if cross(sub(line.1, line.0), sub(p, line.0)) < 0.0 {
        -d
    } else {
        d
    }
}

/// The angle in `[0, π)` that turns the first direction into the second,
/// up to a half-turn: what two lines make, ignoring which way each points.
fn between(u: Xy, v: Xy) -> f64 {
    libm::atan2(cross(u, v), dot(u, v)).rem_euclid(PI)
}

/// How far a line-angle is from 0 (mod π).
fn off_zero(phi: f64) -> f64 {
    phi.min(PI - phi)
}

fn measure(view: &View<'_>, c: &Constraint) -> Result<Measure, Missing> {
    let length = |a, b| Measure::length(a, b, view.tolerance);
    Ok(match *c {
        Constraint::Coincident { p, q } => length(dist(view.at(p)?, view.at(q)?), 0.0),
        Constraint::PointOnLine { p, line } => {
            let p = view.at(p)?;
            length(dist(p, foot(p, view.ends(line)?)), 0.0)
        }
        Constraint::PointOnCircle { p, on } => {
            let (centre, r) = view.round(on)?;
            length(dist(view.at(p)?, centre), r)
        }
        // A line is horizontal when its end points are at one height; the
        // offset is a length.
        Constraint::Horizontal(span) => {
            let (a, b) = view.two_points(span)?;
            length((b.1 - a.1).abs(), 0.0)
        }
        Constraint::Vertical(span) => {
            let (a, b) = view.two_points(span)?;
            length((b.0 - a.0).abs(), 0.0)
        }
        Constraint::Parallel { a, b } => {
            let phi = between(sub_line(view.ends(a)?), sub_line(view.ends(b)?));
            Measure::angle(phi, 0.0, off_zero(phi))
        }
        Constraint::Perpendicular { a, b } => {
            let phi = between(sub_line(view.ends(a)?), sub_line(view.ends(b)?));
            Measure::angle(phi, PI / 2.0, (phi - PI / 2.0).abs())
        }
        Constraint::EqualLength { a, b } => {
            let (a, b) = (view.ends(a)?, view.ends(b)?);
            length(dist(a.0, a.1), dist(b.0, b.1))
        }
        Constraint::EqualRadius { a, b } => length(view.round(a)?.1, view.round(b)?.1),
        Constraint::TangentLineCircle { line, round, sense } => {
            let (centre, r) = view.round(round)?;
            length(signed_distance(centre, view.ends(line)?), sense.sign() * r)
        }
        Constraint::TangentCircles { a, b, contact } => {
            let ((ca, ra), (cb, rb)) = (view.round(a)?, view.round(b)?);
            let wanted = match contact {
                Contact::External => ra + rb,
                Contact::Internal(sense) => sense.sign() * (ra - rb),
            };
            length(dist(ca, cb), wanted)
        }
        Constraint::Midpoint { p, line } => {
            let (a, b) = view.ends(line)?;
            let mid = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
            length(dist(view.at(p)?, mid), 0.0)
        }
        // `q` is the mirror image of `p`: reflect `p` through its foot on
        // the axis and see how far the image is from `q`.
        Constraint::Symmetric { p, q, axis } => {
            let (p, q) = (view.at(p)?, view.at(q)?);
            let f = foot(p, view.ends(axis)?);
            let image = (2.0 * f.0 - p.0, 2.0 * f.1 - p.1);
            length(dist(image, q), 0.0)
        }
        Constraint::Fix { p, x, y } => length(dist(view.at(p)?, (x, y)), 0.0),
        Constraint::Distance { p, q, value } => length(dist(view.at(p)?, view.at(q)?), value),
        Constraint::PointLineDistance {
            p,
            line,
            value,
            sense,
        } => length(
            signed_distance(view.at(p)?, view.ends(line)?),
            sense.sign() * value,
        ),
        Constraint::Radius { round, value } => length(view.round(round)?.1, value),
        // The angle from the first line to the second, taken mod π: the
        // second root (antiparallel) is the same pair of lines.
        Constraint::Angle { a, b, value } => {
            let phi = between(sub_line(view.ends(a)?), sub_line(view.ends(b)?));
            let gap = (phi - value).abs();
            Measure::angle(phi, value, gap.min(PI - gap))
        }
    })
}

/// A line's direction vector, start to end.
fn sub_line((a, b): (Xy, Xy)) -> Xy {
    sub(b, a)
}

/// Check `sketch` at `values`, which need not be the sketch's own
/// coordinates (a solver's result, a dragged frame).
///
/// Every constraint and every arc's intrinsic equation is measured; one that
/// is off by more than `laman::model::tolerance` is a [`Violation`]
/// naming the caller's id and tag. A line, circle or arc too small to
/// support its equations is listed as [`Degenerate`] whether or not any
/// constraint is off. A parameter missing from `values` is listed as
/// [`Missing`] and its constraints are skipped. A non-finite coordinate
/// makes every equation that reads it a violation. Never panics, never
/// errors.
///
/// ```
/// use laman::model::Sketch;
/// use laman_check::check;
///
/// let mut sk = Sketch::new();
/// let a = sk.point(0.0, 0.0)?;
/// let b = sk.point(1.0, 0.0)?;
/// let l = sk.line(a, b)?;
/// sk.horizontal(l)?;
/// assert!(check(&sk, &sk.coordinates()).is_clean());
/// # Ok::<(), laman::model::ModelError>(())
/// ```
pub fn check(sketch: &Sketch, values: &Coordinates) -> Report {
    let tolerances = Tolerances::new(sketch);
    let mut view = View {
        sketch,
        values,
        tolerance: 0.0,
    };
    let mut report = Report::default();
    for (id, c) in sketch.constraints() {
        view.tolerance = tolerances.constraint(id);
        match measure(&view, c) {
            Ok(m) if !m.holds() => report.violations.push(Violation {
                id: Source::Constraint(id),
                tag: sketch.constraint_tag_of(id).map(str::to_owned),
                kind: Subject::Constraint(c.kind()),
                measured: m.measured,
                wanted: m.wanted,
            }),
            Ok(_) => {}
            Err(gap) => note_missing(&mut report, gap),
        }
    }
    for (id, entity) in sketch.entities() {
        view.tolerance = tolerances.entity(id);
        match entity {
            Entity::Point { .. } => {}
            Entity::Line { .. } => {
                if let Some(line) = line_of(sketch, id) {
                    degenerate_line(&view, &mut report, line);
                }
            }
            Entity::Circle { .. } => match view.value(id, Coordinate::R) {
                Ok(r) if too_small(r, view.tolerance) => report.degenerate.push(degenerate(
                    sketch,
                    id,
                    EntityKind::Circle,
                    Degeneracy::ZeroRadius,
                    r,
                )),
                Ok(_) => {}
                Err(gap) => note_missing(&mut report, gap),
            },
            Entity::Arc { c, s, e } => arc(&view, &mut report, id, (*c, *s, *e)),
        }
    }
    report
}

/// Below the length tolerance; a `NaN` size is too small to trust.
fn too_small(length: f64, tolerance: f64) -> bool {
    length < tolerance || !length.is_finite()
}

fn line_of(sketch: &Sketch, id: EntityId) -> Option<Line> {
    sketch.line_handle(id).ok()
}

fn note_missing(report: &mut Report, gap: Missing) {
    if !report.missing.contains(&gap) {
        report.missing.push(gap);
    }
}

fn degenerate(
    sketch: &Sketch,
    id: EntityId,
    kind: EntityKind,
    what: Degeneracy,
    size: f64,
) -> Degenerate {
    Degenerate {
        id,
        tag: sketch.tag_of(id).map(str::to_owned),
        kind,
        what,
        size,
    }
}

fn degenerate_line(view: &View<'_>, report: &mut Report, line: Line) {
    match view.ends(line) {
        Ok((a, b)) => {
            let length = dist(a, b);
            if too_small(length, view.tolerance) {
                report.degenerate.push(degenerate(
                    view.sketch,
                    line.id(),
                    EntityKind::Line,
                    Degeneracy::ZeroLengthLine,
                    length,
                ));
            }
        }
        Err(gap) => note_missing(report, gap),
    }
}

/// An arc's intrinsic equation, `|e − c| = |s − c|`, and its degeneracies.
fn arc(view: &View<'_>, report: &mut Report, id: EntityId, (c, s, e): (Point, Point, Point)) {
    let (centre, start, end) = match (view.at(c), view.at(s), view.at(e)) {
        (Ok(c), Ok(s), Ok(e)) => (c, s, e),
        (c, s, e) => {
            for gap in [c, s, e].into_iter().filter_map(Result::err) {
                note_missing(report, gap);
            }
            return;
        }
    };
    let m = Measure::length(dist(centre, end), dist(centre, start), view.tolerance);
    if !m.holds() {
        report.violations.push(Violation {
            id: Source::Arc(id),
            tag: view.sketch.tag_of(id).map(str::to_owned),
            kind: Subject::ArcIntrinsic,
            measured: m.measured,
            wanted: m.wanted,
        });
    }
    let radius = dist(centre, start);
    if too_small(radius, view.tolerance) {
        report.degenerate.push(degenerate(
            view.sketch,
            id,
            EntityKind::Arc,
            Degeneracy::ZeroRadiusArc,
            radius,
        ));
    }
    let chord = dist(start, end);
    if too_small(chord, view.tolerance) {
        report.degenerate.push(degenerate(
            view.sketch,
            id,
            EntityKind::Arc,
            Degeneracy::EmptyArc,
            chord,
        ));
    }
}
