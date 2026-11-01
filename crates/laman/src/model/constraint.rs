//! Constraint descriptions and the sketch operations that add, validate and
//! remove them. A constraint here is data: its equations live in the
//! solver, its meaning in the checker (`docs/DATA-MODEL.md`).

use std::collections::BTreeSet;
use std::f64::consts::PI;
use std::fmt;

use super::{
    Arc, Circle, ConstraintId, Entity, EntityId, EntityKind, Line, ModelError, Payload, Point,
    Sketch,
};

/// Which of two roots a constraint's signed equation holds, chosen from the
/// drawing when the constraint is added (`σ` in `docs/DATA-MODEL.md`).
///
/// ```
/// use laman::model::Sense;
/// assert_eq!(Sense::Negative.sign(), -1.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Sense {
    /// `σ = +1`.
    Positive,
    /// `σ = −1`.
    Negative,
}

impl Sense {
    /// `+1.0` or `-1.0`.
    ///
    /// ```
    /// use laman::model::Sense;
    /// assert_eq!(Sense::Positive.sign(), 1.0);
    /// ```
    pub fn sign(self) -> f64 {
        match self {
            Sense::Positive => 1.0,
            Sense::Negative => -1.0,
        }
    }

    /// The side a signed quantity is on; zero, exactly, is `Positive`.
    fn of(value: f64) -> Sense {
        if value < 0.0 {
            Sense::Negative
        } else {
            Sense::Positive
        }
    }
}

/// A circle or an arc, where a constraint treats them alike through their
/// centre and radius.
///
/// ```
/// use laman::model::{Round, Sketch};
/// let mut sk = Sketch::new();
/// let c = sk.point(0.0, 0.0)?;
/// let circle = sk.circle(c, 1.0)?;
/// assert_eq!(Round::from(circle).id(), circle.id());
/// # Ok::<(), laman::model::ModelError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Round {
    /// A circle.
    Circle(Circle),
    /// An arc (its whole circle, for a constraint).
    Arc(Arc),
}

impl Round {
    /// The id of the circle or arc.
    ///
    /// ```
    /// use laman::model::{Round, Sketch};
    /// let mut sk = Sketch::new();
    /// let c = sk.point(0.0, 0.0)?;
    /// let circle = sk.circle(c, 1.0)?;
    /// assert_eq!(Round::Circle(circle).id(), circle.id());
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn id(self) -> EntityId {
        match self {
            Round::Circle(c) => c.id(),
            Round::Arc(a) => a.id(),
        }
    }

    /// `EntityKind::Circle` or `EntityKind::Arc`.
    ///
    /// ```
    /// use laman::model::{EntityKind, Round, Sketch};
    /// let mut sk = Sketch::new();
    /// let c = sk.point(0.0, 0.0)?;
    /// let circle = sk.circle(c, 1.0)?;
    /// assert_eq!(Round::from(circle).kind(), EntityKind::Circle);
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn kind(self) -> EntityKind {
        match self {
            Round::Circle(_) => EntityKind::Circle,
            Round::Arc(_) => EntityKind::Arc,
        }
    }
}

impl From<Circle> for Round {
    fn from(c: Circle) -> Self {
        Round::Circle(c)
    }
}

impl From<Arc> for Round {
    fn from(a: Arc) -> Self {
        Round::Arc(a)
    }
}

/// A direction given by a line, or by two points (the line through them),
/// for `horizontal` and `vertical`.
///
/// ```
/// use laman::model::{Sketch, Span};
/// let mut sk = Sketch::new();
/// let (a, b) = (sk.point(0.0, 0.0)?, sk.point(1.0, 0.1)?);
/// let by_points = Span::from((a, b));
/// assert_eq!(by_points, Span::Points(a, b));
/// # Ok::<(), laman::model::ModelError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Span {
    /// A line segment, start to end.
    Line(Line),
    /// Two distinct points, first to second.
    Points(Point, Point),
}

impl From<Line> for Span {
    fn from(l: Line) -> Self {
        Span::Line(l)
    }
}

impl From<(Point, Point)> for Span {
    fn from((p, q): (Point, Point)) -> Self {
        Span::Points(p, q)
    }
}

/// How two circles or arcs touch, with the branch the drawing was on.
///
/// ```
/// use laman::model::Contact;
/// assert_ne!(Contact::External, Contact::Internal(laman::model::Sense::Positive));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Contact {
    /// Outside each other: `|c_X − c_Y| = r_X + r_Y`.
    External,
    /// One inside the other: `|c_X − c_Y| = σ (r_X − r_Y)`, `σ` positive
    /// when the first is the larger.
    Internal(Sense),
}

/// The kind of a constraint, as the serde form and every diagnostic name
/// it. Both tangent forms are named `tangent`.
///
/// ```
/// use laman::model::ConstraintKind;
/// assert_eq!(ConstraintKind::PointOnLine.to_string(), "point_on_line");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConstraintKind {
    /// Two points are one.
    Coincident,
    /// A point lies on the infinite line.
    PointOnLine,
    /// A point lies on a circle, or on an arc's whole circle.
    PointOnCircle,
    /// A line or two points share a y.
    Horizontal,
    /// A line or two points share an x.
    Vertical,
    /// Two lines run the same way or opposite ways.
    Parallel,
    /// Two lines meet at a right angle.
    Perpendicular,
    /// Two lines have one length.
    EqualLength,
    /// Two circles or arcs have one radius.
    EqualRadius,
    /// A line touches a circle or arc.
    TangentLineCircle,
    /// Two circles or arcs touch.
    TangentCircles,
    /// A point is the middle of a line.
    Midpoint,
    /// Two points mirror each other in a line.
    Symmetric,
    /// A point is at given coordinates.
    Fix,
    /// Two points are a given distance apart.
    Distance,
    /// A point is a given distance from a line.
    PointLineDistance,
    /// A circle or arc has a given radius.
    Radius,
    /// Two lines make a given angle.
    Angle,
}

impl fmt::Display for ConstraintKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ConstraintKind::Coincident => "coincident",
            ConstraintKind::PointOnLine => "point_on_line",
            ConstraintKind::PointOnCircle => "point_on_circle",
            ConstraintKind::Horizontal => "horizontal",
            ConstraintKind::Vertical => "vertical",
            ConstraintKind::Parallel => "parallel",
            ConstraintKind::Perpendicular => "perpendicular",
            ConstraintKind::EqualLength => "equal_length",
            ConstraintKind::EqualRadius => "equal_radius",
            ConstraintKind::TangentLineCircle | ConstraintKind::TangentCircles => "tangent",
            ConstraintKind::Midpoint => "midpoint",
            ConstraintKind::Symmetric => "symmetric",
            ConstraintKind::Fix => "fix",
            ConstraintKind::Distance => "distance",
            ConstraintKind::PointLineDistance => "point_line_distance",
            ConstraintKind::Radius => "radius",
            ConstraintKind::Angle => "angle",
        })
    }
}

/// A constraint as a description: which entities, which value, which
/// branch. Its equations are in `docs/DATA-MODEL.md`.
///
/// Build one through the `Sketch` methods (`Sketch::distance` and the
/// like), which take the sense from the drawing. A description built by hand
/// goes through [`Sketch::add_constraint`], which validates it and stores
/// the sense it carries.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Constraint {
    /// Points `p` and `q` are one point.
    Coincident {
        /// The first point.
        p: Point,
        /// The second point.
        q: Point,
    },
    /// Point `p` lies on the infinite line through `line`.
    PointOnLine {
        /// The point.
        p: Point,
        /// The line.
        line: Line,
    },
    /// Point `p` lies on the circle, or the arc's whole circle.
    PointOnCircle {
        /// The point.
        p: Point,
        /// The circle or arc.
        on: Round,
    },
    /// The span has equal y at both ends.
    Horizontal(Span),
    /// The span has equal x at both ends.
    Vertical(Span),
    /// Lines `a` and `b` are parallel.
    Parallel {
        /// The first line.
        a: Line,
        /// The second line.
        b: Line,
    },
    /// Lines `a` and `b` are perpendicular.
    Perpendicular {
        /// The first line.
        a: Line,
        /// The second line.
        b: Line,
    },
    /// Lines `a` and `b` have one length.
    EqualLength {
        /// The first line.
        a: Line,
        /// The second line.
        b: Line,
    },
    /// Circles or arcs `a` and `b` have one radius.
    EqualRadius {
        /// The first circle or arc.
        a: Round,
        /// The second circle or arc.
        b: Round,
    },
    /// The line touches the circle or arc, the centre on the `sense` side.
    TangentLineCircle {
        /// The line.
        line: Line,
        /// The circle or arc.
        round: Round,
        /// The side of the line the centre is on.
        sense: Sense,
    },
    /// Circles or arcs `a` and `b` touch.
    TangentCircles {
        /// The first circle or arc.
        a: Round,
        /// The second circle or arc.
        b: Round,
        /// How they touch.
        contact: Contact,
    },
    /// Point `p` is the middle of `line`.
    Midpoint {
        /// The point.
        p: Point,
        /// The line.
        line: Line,
    },
    /// Points `p` and `q` mirror each other in `axis`.
    Symmetric {
        /// The first point.
        p: Point,
        /// The second point.
        q: Point,
        /// The axis of symmetry.
        axis: Line,
    },
    /// Point `p` is at `(x, y)`; unlike a fixed point it stays an unknown
    /// with two equations that a diagnostic can blame.
    Fix {
        /// The point.
        p: Point,
        /// The wanted x.
        x: f64,
        /// The wanted y.
        y: f64,
    },
    /// Points `p` and `q` are `value` apart, `value > 0`.
    Distance {
        /// The first point.
        p: Point,
        /// The second point.
        q: Point,
        /// The distance.
        value: f64,
    },
    /// Point `p` is `value` from the line, on the `sense` side, `value > 0`.
    PointLineDistance {
        /// The point.
        p: Point,
        /// The line.
        line: Line,
        /// The distance.
        value: f64,
        /// The side of the line the point is on.
        sense: Sense,
    },
    /// The circle or arc has radius `value > 0`.
    Radius {
        /// The circle or arc.
        round: Round,
        /// The radius.
        value: f64,
    },
    /// The angle from line `a` to line `b` is `value`, in `(0, π)`.
    Angle {
        /// The first line.
        a: Line,
        /// The second line.
        b: Line,
        /// The angle in radians.
        value: f64,
    },
}

impl Constraint {
    /// The constraint's kind.
    ///
    /// ```
    /// use laman::model::{ConstraintKind, Sketch};
    /// let mut sk = Sketch::new();
    /// let (p, q) = (sk.point(0.0, 0.0)?, sk.point(1.0, 0.0)?);
    /// let id = sk.coincident(p, q)?;
    /// assert_eq!(sk.constraint(id).map(|c| c.kind()), Some(ConstraintKind::Coincident));
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn kind(&self) -> ConstraintKind {
        match self {
            Constraint::Coincident { .. } => ConstraintKind::Coincident,
            Constraint::PointOnLine { .. } => ConstraintKind::PointOnLine,
            Constraint::PointOnCircle { .. } => ConstraintKind::PointOnCircle,
            Constraint::Horizontal(_) => ConstraintKind::Horizontal,
            Constraint::Vertical(_) => ConstraintKind::Vertical,
            Constraint::Parallel { .. } => ConstraintKind::Parallel,
            Constraint::Perpendicular { .. } => ConstraintKind::Perpendicular,
            Constraint::EqualLength { .. } => ConstraintKind::EqualLength,
            Constraint::EqualRadius { .. } => ConstraintKind::EqualRadius,
            Constraint::TangentLineCircle { .. } => ConstraintKind::TangentLineCircle,
            Constraint::TangentCircles { .. } => ConstraintKind::TangentCircles,
            Constraint::Midpoint { .. } => ConstraintKind::Midpoint,
            Constraint::Symmetric { .. } => ConstraintKind::Symmetric,
            Constraint::Fix { .. } => ConstraintKind::Fix,
            Constraint::Distance { .. } => ConstraintKind::Distance,
            Constraint::PointLineDistance { .. } => ConstraintKind::PointLineDistance,
            Constraint::Radius { .. } => ConstraintKind::Radius,
            Constraint::Angle { .. } => ConstraintKind::Angle,
        }
    }

    /// Every entity the constraint names, with the kind it must have, in
    /// field order.
    fn typed_refs(&self) -> Vec<(EntityId, EntityKind)> {
        use EntityKind::{Line as L, Point as P};
        let span = |s: &Span| match s {
            Span::Line(l) => vec![(l.id(), L)],
            Span::Points(p, q) => vec![(p.id(), P), (q.id(), P)],
        };
        let round = |r: &Round| (r.id(), r.kind());
        match self {
            Constraint::Coincident { p, q } => vec![(p.id(), P), (q.id(), P)],
            Constraint::PointOnLine { p, line } => vec![(p.id(), P), (line.id(), L)],
            Constraint::PointOnCircle { p, on } => vec![(p.id(), P), round(on)],
            Constraint::Horizontal(s) | Constraint::Vertical(s) => span(s),
            Constraint::Parallel { a, b }
            | Constraint::Perpendicular { a, b }
            | Constraint::EqualLength { a, b }
            | Constraint::Angle { a, b, .. } => vec![(a.id(), L), (b.id(), L)],
            Constraint::EqualRadius { a, b } | Constraint::TangentCircles { a, b, .. } => {
                vec![round(a), round(b)]
            }
            Constraint::TangentLineCircle { line, round: r, .. } => vec![(line.id(), L), round(r)],
            Constraint::Midpoint { p, line } | Constraint::PointLineDistance { p, line, .. } => {
                vec![(p.id(), P), (line.id(), L)]
            }
            Constraint::Symmetric { p, q, axis } => {
                vec![(p.id(), P), (q.id(), P), (axis.id(), L)]
            }
            Constraint::Fix { p, .. } => vec![(p.id(), P)],
            Constraint::Distance { p, q, .. } => vec![(p.id(), P), (q.id(), P)],
            Constraint::Radius { round: r, .. } => vec![round(r)],
        }
    }

    /// Every entity the constraint names, in field order.
    ///
    /// ```
    /// use laman::model::Sketch;
    /// let mut sk = Sketch::new();
    /// let (p, q) = (sk.point(0.0, 0.0)?, sk.point(1.0, 0.0)?);
    /// let id = sk.distance(p, q, 1.0)?;
    /// let named = sk.constraint(id).map(|c| c.entities());
    /// assert_eq!(named, Some(vec![p.id(), q.id()]));
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn entities(&self) -> Vec<EntityId> {
        self.typed_refs().into_iter().map(|(id, _)| id).collect()
    }

    /// The first entity named twice where the constraint needs distinct
    /// ones, with its kind.
    fn repeated(&self) -> Option<(EntityId, EntityKind)> {
        let pair = |a: EntityId, b: EntityId, kind| (a == b).then_some((a, kind));
        match self {
            Constraint::Coincident { p, q }
            | Constraint::Symmetric { p, q, .. }
            | Constraint::Distance { p, q, .. }
            | Constraint::Horizontal(Span::Points(p, q))
            | Constraint::Vertical(Span::Points(p, q)) => pair(p.id(), q.id(), EntityKind::Point),
            Constraint::Parallel { a, b }
            | Constraint::Perpendicular { a, b }
            | Constraint::EqualLength { a, b }
            | Constraint::Angle { a, b, .. } => pair(a.id(), b.id(), EntityKind::Line),
            Constraint::EqualRadius { a, b } | Constraint::TangentCircles { a, b, .. } => {
                pair(a.id(), b.id(), a.kind())
            }
            Constraint::PointOnLine { .. }
            | Constraint::PointOnCircle { .. }
            | Constraint::Horizontal(Span::Line(_))
            | Constraint::Vertical(Span::Line(_))
            | Constraint::TangentLineCircle { .. }
            | Constraint::Midpoint { .. }
            | Constraint::Fix { .. }
            | Constraint::PointLineDistance { .. }
            | Constraint::Radius { .. } => None,
        }
    }

    /// Refuses a value outside what the constraint allows.
    fn check_values(&self) -> Result<(), ModelError> {
        fn finite(what: &'static str, value: f64) -> Result<(), ModelError> {
            if value.is_finite() {
                Ok(())
            } else {
                Err(ModelError::NonFinite { what, value })
            }
        }
        fn positive(
            what: &'static str,
            value: f64,
            wanted: &'static str,
        ) -> Result<(), ModelError> {
            finite(what, value)?;
            if value > 0.0 {
                Ok(())
            } else {
                Err(ModelError::DimensionOutOfRange {
                    what,
                    value,
                    wanted,
                })
            }
        }
        match self {
            Constraint::Fix { x, y, .. } => {
                finite("x", *x)?;
                finite("y", *y)
            }
            Constraint::Distance { value, .. } => positive(
                "distance",
                *value,
                "distance > 0 (use coincident for a distance of 0)",
            ),
            Constraint::PointLineDistance { value, .. } => positive(
                "distance",
                *value,
                "distance > 0 (use point_on_line for a distance of 0)",
            ),
            Constraint::Radius { value, .. } => positive("radius", *value, "radius > 0"),
            Constraint::Angle { value, .. } => {
                finite("angle", *value)?;
                if *value > 0.0 && *value < PI {
                    Ok(())
                } else {
                    Err(ModelError::DimensionOutOfRange {
                        what: "angle",
                        value: *value,
                        wanted: "0 < angle < pi radians",
                    })
                }
            }
            Constraint::Coincident { .. }
            | Constraint::PointOnLine { .. }
            | Constraint::PointOnCircle { .. }
            | Constraint::Horizontal(_)
            | Constraint::Vertical(_)
            | Constraint::Parallel { .. }
            | Constraint::Perpendicular { .. }
            | Constraint::EqualLength { .. }
            | Constraint::EqualRadius { .. }
            | Constraint::TangentLineCircle { .. }
            | Constraint::TangentCircles { .. }
            | Constraint::Midpoint { .. }
            | Constraint::Symmetric { .. } => Ok(()),
        }
    }
}

type Xy = (f64, f64);

/// The distance between two points (`sqrt` is correctly rounded everywhere).
fn length(a: Xy, b: Xy) -> f64 {
    let (dx, dy) = (a.0 - b.0, a.1 - b.1);
    (dx * dx + dy * dy).sqrt()
}

impl Sketch {
    fn xy(&self, p: Point) -> Result<Xy, ModelError> {
        match self.entities.get(&p.id()) {
            Some(Entity::Point { x, y }) => Ok((*x, *y)),
            Some(other) => Err(ModelError::WrongKind {
                id: p.id(),
                expected: EntityKind::Point,
                found: other.kind(),
            }),
            None => Err(ModelError::UnknownId { id: p.id() }),
        }
    }

    fn ends(&self, l: Line) -> Result<(Xy, Xy), ModelError> {
        match self.entities.get(&l.id()) {
            Some(Entity::Line { a, b }) => Ok((self.xy(*a)?, self.xy(*b)?)),
            Some(other) => Err(ModelError::WrongKind {
                id: l.id(),
                expected: EntityKind::Line,
                found: other.kind(),
            }),
            None => Err(ModelError::UnknownId { id: l.id() }),
        }
    }

    /// Centre and radius of a circle, or of an arc's whole circle (`|s − c|`).
    fn centre_radius(&self, r: Round) -> Result<(Xy, f64), ModelError> {
        match (r, self.entities.get(&r.id())) {
            (Round::Circle(_), Some(Entity::Circle { c, r })) => Ok((self.xy(*c)?, *r)),
            (Round::Arc(_), Some(Entity::Arc { c, s, .. })) => {
                let (c, s) = (self.xy(*c)?, self.xy(*s)?);
                Ok((c, length(s, c)))
            }
            (_, Some(other)) => Err(ModelError::WrongKind {
                id: r.id(),
                expected: r.kind(),
                found: other.kind(),
            }),
            (_, None) => Err(ModelError::UnknownId { id: r.id() }),
        }
    }

    /// The side of `line` that `at` is on; exactly on it is `Positive`.
    pub(crate) fn side_of(&self, line: Line, at: Xy) -> Result<Sense, ModelError> {
        let (a, b) = self.ends(line)?;
        let cross = (b.0 - a.0) * (at.1 - a.1) - (b.1 - a.1) * (at.0 - a.0);
        Ok(Sense::of(cross))
    }

    /// How two circles or arcs touch in the current drawing: external when
    /// the centre distance is nearer `r_a + r_b` than `|r_a − r_b|`, a tie
    /// external; internal takes `σ` from which is larger, a tie `Positive`.
    pub(crate) fn contact_of(&self, a: Round, b: Round) -> Result<Contact, ModelError> {
        let ((ca, ra), (cb, rb)) = (self.centre_radius(a)?, self.centre_radius(b)?);
        let d = length(ca, cb);
        let to_external = (d - (ra + rb)).abs();
        let to_internal = (d - (ra - rb).abs()).abs();
        Ok(if to_external <= to_internal {
            Contact::External
        } else {
            Contact::Internal(Sense::of(ra - rb))
        })
    }

    /// Every constraint that names any of these entities, in id order.
    pub(crate) fn constraints_using(&self, ids: &BTreeSet<EntityId>) -> Vec<ConstraintId> {
        self.constraints
            .iter()
            .filter(|(_, c)| c.entities().iter().any(|e| ids.contains(e)))
            .map(|(id, _)| *id)
            .collect()
    }

    pub(crate) fn forget_constraint(&mut self, id: ConstraintId) {
        self.constraints.remove(&id);
        self.constraint_tags.remove(&id);
        self.constraint_payloads.remove(&id);
    }

    fn mint_constraint(&self) -> Result<ConstraintId, ModelError> {
        u32::try_from(self.next_constraint)
            .map(ConstraintId)
            .map_err(|_| ModelError::IdsExhausted)
    }

    fn kind_checked(&self, c: &Constraint) -> Result<(), ModelError> {
        for (id, expected) in c.typed_refs() {
            self.expect(id, expected)?;
        }
        if let Some((id, kind)) = c.repeated() {
            return Err(ModelError::RepeatedEntity { id, kind });
        }
        c.check_values()
    }

    /// Adds a constraint description under a minted id, storing the sense
    /// it carries.
    ///
    /// Refuses an entity not in the sketch (`UnknownId`), of the wrong kind
    /// (`WrongKind`) or named twice where two are needed (`RepeatedEntity`),
    /// a non-finite value (`NonFinite`) and a value out of range
    /// (`DimensionOutOfRange`: a distance or radius at most 0, an angle
    /// outside `(0, π)`). The methods that build a description from the
    /// drawing (`Sketch::tangent_line_circle` and the like) are the usual
    /// way in.
    ///
    /// ```
    /// use laman::model::{Constraint, Sketch};
    /// let mut sk = Sketch::new();
    /// let (p, q) = (sk.point(0.0, 0.0)?, sk.point(1.0, 0.0)?);
    /// let id = sk.add_constraint(Constraint::Distance { p, q, value: 2.0 })?;
    /// assert_eq!(id.0, 0);
    /// assert!(sk.add_constraint(Constraint::Distance { p, q, value: 0.0 }).is_err());
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn add_constraint(&mut self, c: Constraint) -> Result<ConstraintId, ModelError> {
        let id = self.mint_constraint()?;
        self.add_constraint_with_id(id, c)
    }

    /// Adds a constraint description under the caller's id; refuses like
    /// [`Sketch::add_constraint`] and an id in use (`DuplicateConstraintId`).
    ///
    /// ```
    /// use laman::model::{Constraint, ConstraintId, Sketch};
    /// let mut sk = Sketch::new();
    /// let (p, q) = (sk.point(0.0, 0.0)?, sk.point(1.0, 0.0)?);
    /// sk.add_constraint_with_id(ConstraintId(7), Constraint::Coincident { p, q })?;
    /// assert_eq!(sk.coincident(p, q)?, ConstraintId(8));
    /// assert!(sk
    ///     .add_constraint_with_id(ConstraintId(7), Constraint::Coincident { p, q })
    ///     .is_err());
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn add_constraint_with_id(
        &mut self,
        id: ConstraintId,
        c: Constraint,
    ) -> Result<ConstraintId, ModelError> {
        if self.constraints.contains_key(&id) {
            return Err(ModelError::DuplicateConstraintId { id });
        }
        self.kind_checked(&c)?;
        self.constraints.insert(id, c);
        self.next_constraint = self.next_constraint.max(u64::from(id.0) + 1);
        self.revision += 1;
        Ok(id)
    }

    /// The constraint with this id, if the sketch has it.
    ///
    /// ```
    /// use laman::model::{Constraint, Sketch};
    /// let mut sk = Sketch::new();
    /// let (p, q) = (sk.point(0.0, 0.0)?, sk.point(1.0, 0.0)?);
    /// let id = sk.coincident(p, q)?;
    /// assert_eq!(sk.constraint(id), Some(&Constraint::Coincident { p, q }));
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn constraint(&self, id: ConstraintId) -> Option<&Constraint> {
        self.constraints.get(&id)
    }

    /// Every constraint, in id order.
    ///
    /// ```
    /// use laman::model::Sketch;
    /// let mut sk = Sketch::new();
    /// let (p, q) = (sk.point(0.0, 0.0)?, sk.point(1.0, 0.0)?);
    /// sk.coincident(p, q)?;
    /// assert_eq!(sk.constraints().count(), 1);
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn constraints(&self) -> impl Iterator<Item = (ConstraintId, &Constraint)> {
        self.constraints.iter().map(|(id, c)| (*id, c))
    }

    /// Removes a constraint. Its id is never minted again.
    ///
    /// Refuses an unknown id (`UnknownConstraint`).
    ///
    /// ```
    /// use laman::model::Sketch;
    /// let mut sk = Sketch::new();
    /// let (p, q) = (sk.point(0.0, 0.0)?, sk.point(1.0, 0.0)?);
    /// let id = sk.coincident(p, q)?;
    /// sk.remove_constraint(id)?;
    /// assert!(sk.remove_constraint(id).is_err());
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn remove_constraint(&mut self, id: ConstraintId) -> Result<(), ModelError> {
        self.known(id)?;
        self.forget_constraint(id);
        self.revision += 1;
        Ok(())
    }

    fn known(&self, id: ConstraintId) -> Result<(), ModelError> {
        if self.constraints.contains_key(&id) {
            Ok(())
        } else {
            Err(ModelError::UnknownConstraint { id })
        }
    }

    /// Attaches a caller string to a constraint, returned in every
    /// diagnostic naming it. Replaces an earlier tag.
    ///
    /// Refuses an unknown id (`UnknownConstraint`).
    ///
    /// ```
    /// use laman::model::Sketch;
    /// let mut sk = Sketch::new();
    /// let (p, q) = (sk.point(0.0, 0.0)?, sk.point(1.0, 0.0)?);
    /// let id = sk.distance(p, q, 1.0)?;
    /// sk.tag_constraint(id, "base")?;
    /// assert_eq!(sk.constraint_tag_of(id), Some("base"));
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn tag_constraint(&mut self, id: ConstraintId, tag: &str) -> Result<(), ModelError> {
        self.known(id)?;
        self.constraint_tags.insert(id, tag.to_owned());
        self.revision += 1;
        Ok(())
    }

    /// The constraint's tag, if it has one.
    ///
    /// ```
    /// use laman::model::{ConstraintId, Sketch};
    /// assert_eq!(Sketch::new().constraint_tag_of(ConstraintId(0)), None);
    /// ```
    pub fn constraint_tag_of(&self, id: ConstraintId) -> Option<&str> {
        self.constraint_tags.get(&id).map(String::as_str)
    }

    /// Attaches an opaque caller value to a constraint, stored and returned
    /// untouched. Replaces an earlier payload.
    ///
    /// Refuses an unknown id (`UnknownConstraint`).
    ///
    /// ```
    /// use laman::model::{Payload, Sketch};
    /// let mut sk = Sketch::new();
    /// let (p, q) = (sk.point(0.0, 0.0)?, sk.point(1.0, 0.0)?);
    /// let id = sk.coincident(p, q)?;
    /// sk.payload_constraint(id, Payload(vec![1]))?;
    /// assert_eq!(sk.constraint_payload_of(id), Some(&Payload(vec![1])));
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn payload_constraint(
        &mut self,
        id: ConstraintId,
        payload: Payload,
    ) -> Result<(), ModelError> {
        self.known(id)?;
        self.constraint_payloads.insert(id, payload);
        self.revision += 1;
        Ok(())
    }

    /// The constraint's payload, if it has one.
    ///
    /// ```
    /// use laman::model::{ConstraintId, Sketch};
    /// assert_eq!(Sketch::new().constraint_payload_of(ConstraintId(0)), None);
    /// ```
    pub fn constraint_payload_of(&self, id: ConstraintId) -> Option<&Payload> {
        self.constraint_payloads.get(&id)
    }

    /// Points `p` and `q` are one point.
    ///
    /// Refuses like [`Sketch::add_constraint`]; `p` and `q` must differ.
    ///
    /// ```
    /// use laman::model::Sketch;
    /// let mut sk = Sketch::new();
    /// let (p, q) = (sk.point(0.0, 0.0)?, sk.point(1.0, 0.0)?);
    /// sk.coincident(p, q)?;
    /// assert!(sk.coincident(p, p).is_err());
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn coincident(&mut self, p: Point, q: Point) -> Result<ConstraintId, ModelError> {
        self.add_constraint(Constraint::Coincident { p, q })
    }

    /// Point `p` lies on the infinite line through `line`.
    ///
    /// Refuses like [`Sketch::add_constraint`].
    ///
    /// ```
    /// use laman::model::Sketch;
    /// let mut sk = Sketch::new();
    /// let (a, b, p) = (sk.point(0.0, 0.0)?, sk.point(1.0, 0.0)?, sk.point(5.0, 1.0)?);
    /// let l = sk.line(a, b)?;
    /// sk.point_on_line(p, l)?;
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn point_on_line(&mut self, p: Point, line: Line) -> Result<ConstraintId, ModelError> {
        self.add_constraint(Constraint::PointOnLine { p, line })
    }

    /// Point `p` lies on a circle, or on an arc's whole circle.
    ///
    /// Refuses like [`Sketch::add_constraint`].
    ///
    /// ```
    /// use laman::model::Sketch;
    /// let mut sk = Sketch::new();
    /// let (c, p) = (sk.point(0.0, 0.0)?, sk.point(1.0, 0.0)?);
    /// let circle = sk.circle(c, 1.0)?;
    /// sk.point_on_circle(p, circle)?;
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn point_on_circle(
        &mut self,
        p: Point,
        on: impl Into<Round>,
    ) -> Result<ConstraintId, ModelError> {
        self.add_constraint(Constraint::PointOnCircle { p, on: on.into() })
    }

    /// A line, or two points, share a y.
    ///
    /// Refuses like [`Sketch::add_constraint`]; two points must differ.
    ///
    /// ```
    /// use laman::model::Sketch;
    /// let mut sk = Sketch::new();
    /// let (a, b) = (sk.point(0.0, 0.0)?, sk.point(1.0, 0.1)?);
    /// let l = sk.line(a, b)?;
    /// sk.horizontal(l)?;
    /// sk.horizontal((a, b))?;
    /// assert!(sk.horizontal((a, a)).is_err());
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn horizontal(&mut self, span: impl Into<Span>) -> Result<ConstraintId, ModelError> {
        self.add_constraint(Constraint::Horizontal(span.into()))
    }

    /// A line, or two points, share an x; refuses like
    /// [`Sketch::horizontal`].
    ///
    /// ```
    /// use laman::model::Sketch;
    /// let mut sk = Sketch::new();
    /// let (a, b) = (sk.point(0.0, 0.0)?, sk.point(0.1, 1.0)?);
    /// sk.vertical((a, b))?;
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn vertical(&mut self, span: impl Into<Span>) -> Result<ConstraintId, ModelError> {
        self.add_constraint(Constraint::Vertical(span.into()))
    }

    /// Lines `a` and `b` are parallel.
    ///
    /// Refuses like [`Sketch::add_constraint`]; the lines must differ.
    ///
    /// ```
    /// use laman::model::Sketch;
    /// let mut sk = Sketch::new();
    /// let p: Vec<_> = (0..4).map(|i| sk.point(f64::from(i), 0.0)).collect::<Result<_, _>>()?;
    /// let (l1, l2) = (sk.line(p[0], p[1])?, sk.line(p[2], p[3])?);
    /// sk.parallel(l1, l2)?;
    /// assert!(sk.parallel(l1, l1).is_err());
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn parallel(&mut self, a: Line, b: Line) -> Result<ConstraintId, ModelError> {
        self.add_constraint(Constraint::Parallel { a, b })
    }

    /// Lines `a` and `b` are perpendicular; refuses like
    /// [`Sketch::parallel`].
    ///
    /// ```
    /// use laman::model::Sketch;
    /// let mut sk = Sketch::new();
    /// let p: Vec<_> = (0..4).map(|i| sk.point(f64::from(i), 0.0)).collect::<Result<_, _>>()?;
    /// let (l1, l2) = (sk.line(p[0], p[1])?, sk.line(p[2], p[3])?);
    /// sk.perpendicular(l1, l2)?;
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn perpendicular(&mut self, a: Line, b: Line) -> Result<ConstraintId, ModelError> {
        self.add_constraint(Constraint::Perpendicular { a, b })
    }

    /// Lines `a` and `b` have one length; refuses like [`Sketch::parallel`].
    ///
    /// ```
    /// use laman::model::Sketch;
    /// let mut sk = Sketch::new();
    /// let p: Vec<_> = (0..4).map(|i| sk.point(f64::from(i), 0.0)).collect::<Result<_, _>>()?;
    /// let (l1, l2) = (sk.line(p[0], p[1])?, sk.line(p[2], p[3])?);
    /// sk.equal_length(l1, l2)?;
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn equal_length(&mut self, a: Line, b: Line) -> Result<ConstraintId, ModelError> {
        self.add_constraint(Constraint::EqualLength { a, b })
    }

    /// Circles or arcs `a` and `b` have one radius.
    ///
    /// Refuses like [`Sketch::add_constraint`]; they must differ.
    ///
    /// ```
    /// use laman::model::Sketch;
    /// let mut sk = Sketch::new();
    /// let (c1, c2) = (sk.point(0.0, 0.0)?, sk.point(5.0, 0.0)?);
    /// let (x, y) = (sk.circle(c1, 1.0)?, sk.circle(c2, 2.0)?);
    /// sk.equal_radius(x, y)?;
    /// assert!(sk.equal_radius(x, x).is_err());
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn equal_radius(
        &mut self,
        a: impl Into<Round>,
        b: impl Into<Round>,
    ) -> Result<ConstraintId, ModelError> {
        self.add_constraint(Constraint::EqualRadius {
            a: a.into(),
            b: b.into(),
        })
    }

    /// `line` touches the circle or arc. The sense is the side of the line
    /// the centre is on in the current drawing; a centre exactly on the
    /// line is `Positive`.
    ///
    /// Refuses like [`Sketch::add_constraint`].
    ///
    /// ```
    /// use laman::model::{Constraint, Sense, Sketch};
    /// let mut sk = Sketch::new();
    /// let (a, b, c) = (sk.point(0.0, 0.0)?, sk.point(1.0, 0.0)?, sk.point(0.5, -2.0)?);
    /// let (l, circle) = (sk.line(a, b)?, sk.circle(c, 2.0)?);
    /// let id = sk.tangent_line_circle(l, circle)?;
    /// assert!(matches!(
    ///     sk.constraint(id),
    ///     Some(Constraint::TangentLineCircle { sense: Sense::Negative, .. })
    /// ));
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn tangent_line_circle(
        &mut self,
        line: Line,
        round: impl Into<Round>,
    ) -> Result<ConstraintId, ModelError> {
        let round = round.into();
        let (centre, _) = self.centre_radius(round)?;
        let sense = self.side_of(line, centre)?;
        self.add_constraint(Constraint::TangentLineCircle { line, round, sense })
    }

    /// Circles or arcs `a` and `b` touch. The contact kind and sense come
    /// from the current drawing: external when the centre distance is
    /// nearer `r_a + r_b` than `|r_a − r_b|` (a tie is external); internal
    /// is `Positive` when `a` is the larger (a tie `Positive`).
    ///
    /// Refuses like [`Sketch::add_constraint`]; they must differ.
    ///
    /// ```
    /// use laman::model::{Constraint, Contact, Sketch};
    /// let mut sk = Sketch::new();
    /// let (c1, c2) = (sk.point(0.0, 0.0)?, sk.point(3.0, 0.0)?);
    /// let (x, y) = (sk.circle(c1, 1.0)?, sk.circle(c2, 2.0)?);
    /// let id = sk.tangent_circles(x, y)?;
    /// assert!(matches!(
    ///     sk.constraint(id),
    ///     Some(Constraint::TangentCircles { contact: Contact::External, .. })
    /// ));
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn tangent_circles(
        &mut self,
        a: impl Into<Round>,
        b: impl Into<Round>,
    ) -> Result<ConstraintId, ModelError> {
        let (a, b) = (a.into(), b.into());
        let contact = self.contact_of(a, b)?;
        self.add_constraint(Constraint::TangentCircles { a, b, contact })
    }

    /// Point `p` is the middle of `line`.
    ///
    /// Refuses like [`Sketch::add_constraint`].
    ///
    /// ```
    /// use laman::model::Sketch;
    /// let mut sk = Sketch::new();
    /// let (a, b, m) = (sk.point(0.0, 0.0)?, sk.point(2.0, 0.0)?, sk.point(1.0, 0.0)?);
    /// let l = sk.line(a, b)?;
    /// sk.midpoint(m, l)?;
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn midpoint(&mut self, p: Point, line: Line) -> Result<ConstraintId, ModelError> {
        self.add_constraint(Constraint::Midpoint { p, line })
    }

    /// Points `p` and `q` mirror each other in `axis`.
    ///
    /// Refuses like [`Sketch::add_constraint`]; `p` and `q` must differ.
    ///
    /// ```
    /// use laman::model::Sketch;
    /// let mut sk = Sketch::new();
    /// let (a, b) = (sk.point(0.0, 0.0)?, sk.point(0.0, 1.0)?);
    /// let (p, q) = (sk.point(-1.0, 0.5)?, sk.point(1.0, 0.5)?);
    /// let axis = sk.line(a, b)?;
    /// sk.symmetric(p, q, axis)?;
    /// assert!(sk.symmetric(p, p, axis).is_err());
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn symmetric(
        &mut self,
        p: Point,
        q: Point,
        axis: Line,
    ) -> Result<ConstraintId, ModelError> {
        self.add_constraint(Constraint::Symmetric { p, q, axis })
    }

    /// Point `p` is at `(x, y)`, as an equation a diagnostic can blame
    /// (unlike `Sketch::point_fixed`).
    ///
    /// Refuses like [`Sketch::add_constraint`]; a non-finite `x` or `y` is
    /// `NonFinite`.
    ///
    /// ```
    /// use laman::model::Sketch;
    /// let mut sk = Sketch::new();
    /// let p = sk.point(0.0, 0.0)?;
    /// sk.fix(p, 3.0, 4.0)?;
    /// assert!(sk.fix(p, f64::NAN, 0.0).is_err());
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn fix(&mut self, p: Point, x: f64, y: f64) -> Result<ConstraintId, ModelError> {
        self.add_constraint(Constraint::Fix { p, x, y })
    }

    /// Points `p` and `q` are `value` apart.
    ///
    /// Refuses like [`Sketch::add_constraint`]; `p` and `q` must differ and
    /// `value` must be above 0 (`DimensionOutOfRange`, naming `coincident`
    /// for a distance of 0).
    ///
    /// ```
    /// use laman::model::Sketch;
    /// let mut sk = Sketch::new();
    /// let (p, q) = (sk.point(0.0, 0.0)?, sk.point(3.0, 4.0)?);
    /// sk.distance(p, q, 5.0)?;
    /// assert!(sk.distance(p, q, 0.0).is_err());
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn distance(&mut self, p: Point, q: Point, value: f64) -> Result<ConstraintId, ModelError> {
        self.add_constraint(Constraint::Distance { p, q, value })
    }

    /// Point `p` is `value` from `line`, on the side it is on in the current
    /// drawing; exactly on the line is `Positive`.
    ///
    /// Refuses like [`Sketch::add_constraint`]; `value` must be above 0
    /// (`DimensionOutOfRange`, naming `point_on_line` for 0).
    ///
    /// ```
    /// use laman::model::{Constraint, Sense, Sketch};
    /// let mut sk = Sketch::new();
    /// let (a, b, p) = (sk.point(0.0, 0.0)?, sk.point(1.0, 0.0)?, sk.point(0.5, 3.0)?);
    /// let l = sk.line(a, b)?;
    /// let id = sk.point_line_distance(p, l, 3.0)?;
    /// assert!(matches!(
    ///     sk.constraint(id),
    ///     Some(Constraint::PointLineDistance { sense: Sense::Positive, .. })
    /// ));
    /// assert!(sk.point_line_distance(p, l, 0.0).is_err());
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn point_line_distance(
        &mut self,
        p: Point,
        line: Line,
        value: f64,
    ) -> Result<ConstraintId, ModelError> {
        let sense = self.side_of(line, self.xy(p)?)?;
        self.add_constraint(Constraint::PointLineDistance {
            p,
            line,
            value,
            sense,
        })
    }

    /// The circle or arc has radius `value`.
    ///
    /// Refuses like [`Sketch::add_constraint`]; `value` must be above 0.
    ///
    /// ```
    /// use laman::model::Sketch;
    /// let mut sk = Sketch::new();
    /// let c = sk.point(0.0, 0.0)?;
    /// let circle = sk.circle(c, 1.0)?;
    /// sk.radius(circle, 2.5)?;
    /// assert!(sk.radius(circle, -1.0).is_err());
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn radius(
        &mut self,
        round: impl Into<Round>,
        value: f64,
    ) -> Result<ConstraintId, ModelError> {
        self.add_constraint(Constraint::Radius {
            round: round.into(),
            value,
        })
    }

    /// The angle from line `a` to line `b` is `value` radians.
    ///
    /// Refuses like [`Sketch::add_constraint`]; the lines must differ and
    /// `value` must lie in `(0, π)` (`DimensionOutOfRange`).
    ///
    /// ```
    /// use laman::model::Sketch;
    /// let mut sk = Sketch::new();
    /// let p: Vec<_> = (0..4).map(|i| sk.point(f64::from(i), 0.0)).collect::<Result<_, _>>()?;
    /// let (l1, l2) = (sk.line(p[0], p[1])?, sk.line(p[2], p[3])?);
    /// sk.angle(l1, l2, std::f64::consts::FRAC_PI_3)?;
    /// assert!(sk.angle(l1, l2, std::f64::consts::PI).is_err());
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn angle(&mut self, a: Line, b: Line, value: f64) -> Result<ConstraintId, ModelError> {
        self.add_constraint(Constraint::Angle { a, b, value })
    }
}
