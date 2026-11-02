//! The sketch: ids, entities, and the rules that keep it well-formed.
//!
//! A [`Sketch`] owns every entity of one drawing. Every mutation validates
//! and returns a [`ModelError`] naming the caller's ids, so a malformed
//! sketch never exists. Storage is ordered by id, so every iteration is in
//! id order. See `docs/ARCHITECTURE.md` (the sketch and its ids) and
//! `docs/DATA-MODEL.md` (entities).
//!
//! ```
//! use laman::model::Sketch;
//!
//! let mut sk = Sketch::new();
//! let a = sk.point(0.0, 0.0)?;
//! let b = sk.point(3.0, 4.0)?;
//! let ab = sk.line(a, b)?;
//! sk.tag(ab.id(), "base")?;
//! assert_eq!(sk.tag_of(ab.id()), Some("base"));
//! # Ok::<(), laman::model::ModelError>(())
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

mod constraint;
#[cfg(feature = "serde")]
mod form;
pub mod tolerance;

pub use constraint::{Constraint, ConstraintKind, Contact, Round, Sense, Span};
#[cfg(feature = "serde")]
pub use form::SketchForm;

/// The identifier of an entity, stable for the life of its sketch.
///
/// The sketch mints ids itself, or a caller that persists its own passes
/// them (`Sketch::point_with_id` and the like). A removed id is never
/// reused by the sketch.
///
/// ```
/// use laman::model::EntityId;
/// assert_eq!(EntityId(7).to_string(), "entity 7");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntityId(pub u32);

/// The identifier of a constraint, stable for the life of its sketch.
///
/// ```
/// use laman::model::ConstraintId;
/// assert_eq!(ConstraintId(2).to_string(), "constraint 2");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConstraintId(pub u32);

impl fmt::Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "entity {}", self.0)
    }
}

impl fmt::Display for ConstraintId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "constraint {}", self.0)
    }
}

/// The kind of an entity.
///
/// ```
/// use laman::model::EntityKind;
/// assert_eq!(EntityKind::Arc.to_string(), "arc");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EntityKind {
    /// A point.
    Point,
    /// A line segment.
    Line,
    /// A circle.
    Circle,
    /// A circular arc.
    Arc,
}

impl fmt::Display for EntityKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            EntityKind::Point => "point",
            EntityKind::Line => "line",
            EntityKind::Circle => "circle",
            EntityKind::Arc => "arc",
        })
    }
}

macro_rules! handle {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(EntityId);

        impl $name {
            /// The id of the entity this handle names.
            pub fn id(self) -> EntityId {
                self.0
            }
        }
    };
}

handle!(
    /// A typed handle to a point of a sketch. Obtained from the sketch
    /// (`Sketch::point`, `Sketch::point_handle`), so a point handle names a
    /// point as far as the sketch it came from.
    Point
);
handle!(
    /// A typed handle to a line segment of a sketch.
    Line
);
handle!(
    /// A typed handle to a circle of a sketch.
    Circle
);
handle!(
    /// A typed handle to an arc of a sketch.
    Arc
);

/// One real parameter of an entity: a point's `x` and `y`, a circle's
/// radius `r`.
///
/// ```
/// use laman::model::Coordinate;
/// assert!(Coordinate::X < Coordinate::Y);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Coordinate {
    /// A point's x.
    X,
    /// A point's y.
    Y,
    /// A circle's radius.
    R,
}

/// The value of every parameter of a sketch, keyed by entity and
/// coordinate, in id order.
pub type Coordinates = BTreeMap<(EntityId, Coordinate), f64>;

/// An opaque caller value stored on an entity and returned untouched; laman
/// never reads it.
///
/// ```
/// use laman::model::Payload;
/// assert_eq!(Payload(vec![1, 2]).0, [1, 2]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Payload(pub Vec<u8>);

/// A geometric object of a sketch. Lines, circles and arcs refer to
/// points, so entities that share a point share its parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Entity {
    /// A point at `(x, y)`.
    Point {
        /// The x coordinate.
        x: f64,
        /// The y coordinate.
        y: f64,
    },
    /// A line segment from point `a` to point `b`.
    Line {
        /// The start point.
        a: Point,
        /// The end point.
        b: Point,
    },
    /// A circle about centre `c` of radius `r`.
    Circle {
        /// The centre point.
        c: Point,
        /// The radius, at least zero.
        r: f64,
    },
    /// An arc about centre `c`, counter-clockwise from `s` to `e`.
    Arc {
        /// The centre point.
        c: Point,
        /// The start point.
        s: Point,
        /// The end point.
        e: Point,
    },
}

impl Entity {
    /// The entity's kind.
    ///
    /// ```
    /// use laman::model::{EntityKind, Sketch};
    /// let mut sk = Sketch::new();
    /// let p = sk.point(1.0, 2.0)?;
    /// assert_eq!(sk.entity(p.id()).map(|e| e.kind()), Some(EntityKind::Point));
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn kind(&self) -> EntityKind {
        match self {
            Entity::Point { .. } => EntityKind::Point,
            Entity::Line { .. } => EntityKind::Line,
            Entity::Circle { .. } => EntityKind::Circle,
            Entity::Arc { .. } => EntityKind::Arc,
        }
    }

    /// The points this entity refers to, in field order.
    ///
    /// ```
    /// use laman::model::Sketch;
    /// let mut sk = Sketch::new();
    /// let (a, b) = (sk.point(0.0, 0.0)?, sk.point(1.0, 0.0)?);
    /// let l = sk.line(a, b)?;
    /// let refs = sk.entity(l.id()).map(|e| e.refs());
    /// assert_eq!(refs, Some(vec![a.id(), b.id()]));
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn refs(&self) -> Vec<EntityId> {
        match self {
            Entity::Point { .. } => Vec::new(),
            Entity::Line { a, b } => vec![a.id(), b.id()],
            Entity::Circle { c, .. } => vec![c.id()],
            Entity::Arc { c, s, e } => vec![c.id(), s.id(), e.id()],
        }
    }
}

/// Why a sketch operation was refused. A malformed input is an error; an
/// under- or over-constrained sketch never is.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum ModelError {
    /// An id names no entity of the sketch (never added, or removed).
    #[error("{id} is not in the sketch")]
    UnknownId {
        /// The id given.
        id: EntityId,
    },
    /// An entity is of another kind than the operation needs.
    #[error("{id} is a {found}, expected a {expected}")]
    WrongKind {
        /// The id given.
        id: EntityId,
        /// The kind the operation needs.
        expected: EntityKind,
        /// The kind the id names.
        found: EntityKind,
    },
    /// A drag was asked of a point with both coordinates fixed: nothing of
    /// it can follow a target.
    #[error("{id} is fixed in both coordinates and cannot be dragged")]
    FixedPoint {
        /// The point given.
        id: EntityId,
    },
    /// A coordinate or value is NaN or infinite.
    #[error("{what} is not finite: {value}")]
    NonFinite {
        /// What was being set.
        what: &'static str,
        /// The offending value.
        value: f64,
    },
    /// A value is outside what its parameter allows.
    #[error("{what} = {value} is out of range, wanted {wanted}")]
    DimensionOutOfRange {
        /// What was being set.
        what: &'static str,
        /// The offending value.
        value: f64,
        /// The range wanted, and what to use instead where one exists.
        wanted: &'static str,
    },
    /// An id the caller passed is already in use.
    #[error("{id} is already in the sketch")]
    DuplicateId {
        /// The id given.
        id: EntityId,
    },
    /// Every id the sketch can mint is used.
    #[error("the sketch has no ids left to mint")]
    IdsExhausted,
    /// An entity or constraint names the same entity twice where it needs
    /// distinct ones.
    #[error("{id} is named twice in one {kind}")]
    RepeatedEntity {
        /// The repeated entity.
        id: EntityId,
        /// For an entity being built (a line from a point to itself), the
        /// kind being built; for a constraint, the kind of the repeated
        /// entity.
        kind: EntityKind,
    },
    /// A constraint id names no constraint of the sketch.
    #[error("{id} is not in the sketch")]
    UnknownConstraint {
        /// The id given.
        id: ConstraintId,
    },
    /// A constraint id the caller passed is already in use.
    #[error("{id} is already in the sketch")]
    DuplicateConstraintId {
        /// The id given.
        id: ConstraintId,
    },
    /// The parameter does not exist on that entity.
    #[error("{id} has no {coordinate:?} parameter")]
    NoSuchParameter {
        /// The entity.
        id: EntityId,
        /// The coordinate asked for.
        coordinate: Coordinate,
    },
    /// A file's `version` is newer than this reader loads, or is not a
    /// version at all (0).
    #[error("sketch format version {found} is not loadable here (supported: 1 to {supported})")]
    FormatVersion {
        /// The version the file declares.
        found: u64,
        /// The newest version this reader loads.
        supported: u64,
    },
    /// A file's `format` is not `laman.sketch`.
    #[error("format {found:?} is not a laman sketch (wanted \"laman.sketch\")")]
    WrongFormat {
        /// The format the file declares.
        found: String,
    },
    /// A payload in a file is not valid base64.
    #[error("payload is not valid base64")]
    InvalidPayload,
    /// Loading a file failed at this entity; `source` says why.
    #[error("{id}: {source}")]
    Entity {
        /// The entity of the file being loaded.
        id: EntityId,
        /// What was wrong with it.
        #[source]
        source: Box<ModelError>,
    },
    /// Loading a file failed at this constraint; `source` says why.
    #[error("{id}: {source}")]
    Constraint {
        /// The constraint of the file being loaded.
        id: ConstraintId,
        /// What was wrong with it.
        #[source]
        source: Box<ModelError>,
    },
    /// A solution was solved for another revision of the sketch.
    #[error("the solution was solved at revision {solved}, the sketch is at {current}")]
    Stale {
        /// The revision the solution was solved from.
        solved: u64,
        /// The sketch's revision now.
        current: u64,
    },
    /// A solution carries this sketch's revision but was solved from other
    /// contents: another sketch, or this one edited and reverted.
    #[error("the solution was solved from another sketch than this one")]
    ForeignSolution,
    /// A solution holds no value for a parameter the sketch has.
    #[error("the solution has no value for {id} {coordinate:?}")]
    IncompleteSolution {
        /// The entity.
        id: EntityId,
        /// The parameter left out.
        coordinate: Coordinate,
    },
    /// A solution would move a fixed parameter.
    #[error("the solution moves fixed {id} {coordinate:?} from {fixed} to {given}")]
    FixedMoved {
        /// The entity.
        id: EntityId,
        /// The fixed parameter.
        coordinate: Coordinate,
        /// Its value in the sketch.
        fixed: f64,
        /// The value the solution gives it.
        given: f64,
    },
    /// An entity is still used and the caller did not ask for the cascade.
    #[error("{id} is still used by {} entities and {} constraints", .entities.len(), .constraints.len())]
    InUse {
        /// The entity that cannot be removed.
        id: EntityId,
        /// The entities that refer to it, in id order.
        entities: Vec<EntityId>,
        /// The constraints that use it, in id order.
        constraints: Vec<ConstraintId>,
    },
}

/// What a cascading removal took out of a sketch, in id order.
///
/// ```
/// use laman::model::Sketch;
/// let mut sk = Sketch::new();
/// let (a, b) = (sk.point(0.0, 0.0)?, sk.point(1.0, 0.0)?);
/// let l = sk.line(a, b)?;
/// let gone = sk.remove_with_constraints(a.id())?;
/// assert_eq!(gone.entities, vec![a.id(), l.id()]);
/// # Ok::<(), laman::model::ModelError>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Removed {
    /// The entities removed, including the one asked for.
    pub entities: Vec<EntityId>,
    /// The constraints removed.
    pub constraints: Vec<ConstraintId>,
}

/// The arena that owns every entity of one 2D drawing.
///
/// Every mutation validates, returns `Result<_, ModelError>` and increments
/// [`Sketch::revision`] on success; a refused call changes nothing.
#[derive(Debug, Clone, Default)]
pub struct Sketch {
    entities: BTreeMap<EntityId, Entity>,
    fixed: BTreeSet<(EntityId, Coordinate)>,
    construction: BTreeSet<EntityId>,
    tags: BTreeMap<EntityId, String>,
    payloads: BTreeMap<EntityId, Payload>,
    constraints: BTreeMap<ConstraintId, Constraint>,
    constraint_tags: BTreeMap<ConstraintId, String>,
    constraint_payloads: BTreeMap<ConstraintId, Payload>,
    next_entity: u64,
    next_constraint: u64,
    revision: u64,
}

fn finite(what: &'static str, value: f64) -> Result<(), ModelError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(ModelError::NonFinite { what, value })
    }
}

impl Sketch {
    /// An empty sketch at revision 0.
    ///
    /// ```
    /// use laman::model::Sketch;
    /// assert_eq!(Sketch::new().revision(), 0);
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// The number of mutations applied so far; it grows on every success.
    ///
    /// ```
    /// use laman::model::Sketch;
    /// let mut sk = Sketch::new();
    /// sk.point(0.0, 0.0)?;
    /// assert_eq!(sk.revision(), 1);
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// The entity with this id, if the sketch has it.
    ///
    /// ```
    /// use laman::model::{Entity, Sketch};
    /// let mut sk = Sketch::new();
    /// let p = sk.point(1.0, 2.0)?;
    /// assert_eq!(sk.entity(p.id()), Some(&Entity::Point { x: 1.0, y: 2.0 }));
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn entity(&self, id: EntityId) -> Option<&Entity> {
        self.entities.get(&id)
    }

    /// Every entity, in id order.
    ///
    /// ```
    /// use laman::model::Sketch;
    /// let mut sk = Sketch::new();
    /// sk.point(0.0, 0.0)?;
    /// sk.point(1.0, 0.0)?;
    /// assert_eq!(sk.entities().count(), 2);
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn entities(&self) -> impl Iterator<Item = (EntityId, &Entity)> {
        self.entities.iter().map(|(id, e)| (*id, e))
    }

    /// The value of every parameter, in id order.
    ///
    /// ```
    /// use laman::model::{Coordinate, Sketch};
    /// let mut sk = Sketch::new();
    /// let p = sk.point(1.0, 2.0)?;
    /// assert_eq!(sk.coordinates()[&(p.id(), Coordinate::Y)], 2.0);
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn coordinates(&self) -> Coordinates {
        let mut out = Coordinates::new();
        for (id, e) in &self.entities {
            match e {
                Entity::Point { x, y } => {
                    out.insert((*id, Coordinate::X), *x);
                    out.insert((*id, Coordinate::Y), *y);
                }
                Entity::Circle { r, .. } => {
                    out.insert((*id, Coordinate::R), *r);
                }
                Entity::Line { .. } | Entity::Arc { .. } => {}
            }
        }
        out
    }

    /// Overwrites every parameter `values` names (fixed ones included), for
    /// an analysis copy that solves from another start. Keys the sketch does
    /// not hold are ignored; the revision does not move, so a copy placed
    /// this way is never mistaken for a caller's edit.
    pub(crate) fn place(&mut self, values: &Coordinates) {
        for (id, e) in self.entities.iter_mut() {
            let get = |c| values.get(&(*id, c)).copied();
            match e {
                Entity::Point { x, y } => {
                    *x = get(Coordinate::X).unwrap_or(*x);
                    *y = get(Coordinate::Y).unwrap_or(*y);
                }
                Entity::Circle { r, .. } => *r = get(Coordinate::R).unwrap_or(*r),
                Entity::Line { .. } | Entity::Arc { .. } => {}
            }
        }
    }

    /// Replaces every parameter `values` names, for [`Sketch::apply`]: unlike
    /// [`Sketch::place`] this is the caller's edit, so the revision moves.
    pub(crate) fn replace_coordinates(&mut self, values: &Coordinates) {
        self.place(values);
        self.revision += 1;
    }

    /// A 64-bit FNV-1a digest of everything a solve reads: the entities with
    /// their coordinates, the fixed parameters and the constraints with their
    /// senses. Tags, payloads and construction flags are not read by a solve
    /// and are left out. Equal sketches give equal digests on every target
    /// (`Debug` of `f64` is the shortest round-trip form, deterministic).
    pub(crate) fn source_digest(&self) -> u64 {
        use std::fmt::Write as _;
        struct Fnv(u64);
        impl fmt::Write for Fnv {
            fn write_str(&mut self, s: &str) -> fmt::Result {
                for byte in s.bytes() {
                    self.0 = (self.0 ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
                }
                Ok(())
            }
        }
        let mut hash = Fnv(0xcbf2_9ce4_8422_2325);
        // Writing into a hasher cannot fail.
        let _ = write!(
            hash,
            "{:?}|{:?}|{:?}",
            self.entities, self.fixed, self.constraints
        );
        hash.0
    }

    fn kind_of(&self, id: EntityId) -> Result<EntityKind, ModelError> {
        self.entities
            .get(&id)
            .map(Entity::kind)
            .ok_or(ModelError::UnknownId { id })
    }

    fn expect(&self, id: EntityId, expected: EntityKind) -> Result<(), ModelError> {
        let found = self.kind_of(id)?;
        if found == expected {
            Ok(())
        } else {
            Err(ModelError::WrongKind {
                id,
                expected,
                found,
            })
        }
    }

    /// The point handle for a bare id, as a loader or binding needs.
    ///
    /// Refuses an unknown id (`UnknownId`) and an entity of another kind
    /// (`WrongKind`).
    ///
    /// ```
    /// use laman::model::{EntityId, Sketch};
    /// let mut sk = Sketch::new();
    /// let p = sk.point(0.0, 0.0)?;
    /// assert_eq!(sk.point_handle(p.id())?, p);
    /// assert!(sk.point_handle(EntityId(99)).is_err());
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn point_handle(&self, id: EntityId) -> Result<Point, ModelError> {
        self.expect(id, EntityKind::Point).map(|()| Point(id))
    }

    /// The line handle for a bare id; refuses like [`Sketch::point_handle`].
    ///
    /// ```
    /// use laman::model::Sketch;
    /// let mut sk = Sketch::new();
    /// let p = sk.point(0.0, 0.0)?;
    /// assert!(sk.line_handle(p.id()).is_err());
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn line_handle(&self, id: EntityId) -> Result<Line, ModelError> {
        self.expect(id, EntityKind::Line).map(|()| Line(id))
    }

    /// The circle handle for a bare id; refuses like [`Sketch::point_handle`].
    ///
    /// ```
    /// use laman::model::Sketch;
    /// let mut sk = Sketch::new();
    /// let p = sk.point(0.0, 0.0)?;
    /// assert!(sk.circle_handle(p.id()).is_err());
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn circle_handle(&self, id: EntityId) -> Result<Circle, ModelError> {
        self.expect(id, EntityKind::Circle).map(|()| Circle(id))
    }

    /// The arc handle for a bare id; refuses like [`Sketch::point_handle`].
    ///
    /// ```
    /// use laman::model::Sketch;
    /// let mut sk = Sketch::new();
    /// let p = sk.point(0.0, 0.0)?;
    /// assert!(sk.arc_handle(p.id()).is_err());
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn arc_handle(&self, id: EntityId) -> Result<Arc, ModelError> {
        self.expect(id, EntityKind::Arc).map(|()| Arc(id))
    }

    fn mint(&self) -> Result<EntityId, ModelError> {
        u32::try_from(self.next_entity)
            .map(EntityId)
            .map_err(|_| ModelError::IdsExhausted)
    }

    fn insert(&mut self, id: EntityId, entity: Entity) {
        self.entities.insert(id, entity);
        self.next_entity = self.next_entity.max(u64::from(id.0) + 1);
        self.revision += 1;
    }

    fn fresh(&self, id: EntityId) -> Result<(), ModelError> {
        if self.entities.contains_key(&id) {
            Err(ModelError::DuplicateId { id })
        } else {
            Ok(())
        }
    }

    /// Adds a point at `(x, y)` under a minted id.
    ///
    /// Refuses a non-finite coordinate (`NonFinite`).
    ///
    /// ```
    /// use laman::model::Sketch;
    /// let mut sk = Sketch::new();
    /// let p = sk.point(1.0, 2.0)?;
    /// assert_eq!(p.id().0, 0);
    /// assert!(sk.point(f64::NAN, 0.0).is_err());
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn point(&mut self, x: f64, y: f64) -> Result<Point, ModelError> {
        let id = self.mint()?;
        self.point_with_id(id, x, y)
    }

    /// Adds a point under the caller's id.
    ///
    /// Refuses a non-finite coordinate (`NonFinite`) and an id in use
    /// (`DuplicateId`).
    ///
    /// ```
    /// use laman::model::{EntityId, Sketch};
    /// let mut sk = Sketch::new();
    /// sk.point_with_id(EntityId(10), 0.0, 0.0)?;
    /// assert_eq!(sk.point(1.0, 1.0)?.id(), EntityId(11));
    /// assert!(sk.point_with_id(EntityId(10), 0.0, 0.0).is_err());
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn point_with_id(&mut self, id: EntityId, x: f64, y: f64) -> Result<Point, ModelError> {
        finite("x", x)?;
        finite("y", y)?;
        self.fresh(id)?;
        self.insert(id, Entity::Point { x, y });
        Ok(Point(id))
    }

    /// Adds a ground point: both coordinates fixed, never unknowns.
    ///
    /// Refuses like [`Sketch::point`].
    ///
    /// ```
    /// use laman::model::{Coordinate, Sketch};
    /// let mut sk = Sketch::new();
    /// let o = sk.point_fixed(0.0, 0.0)?;
    /// assert!(sk.is_fixed(o.id(), Coordinate::X));
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn point_fixed(&mut self, x: f64, y: f64) -> Result<Point, ModelError> {
        let p = self.point(x, y)?;
        self.fixed.insert((p.id(), Coordinate::X));
        self.fixed.insert((p.id(), Coordinate::Y));
        Ok(p)
    }

    /// Adds a line segment from `a` to `b` under a minted id.
    ///
    /// Refuses a point not in the sketch (`UnknownId`, `WrongKind`) and the
    /// same point twice (`RepeatedEntity`).
    ///
    /// ```
    /// use laman::model::Sketch;
    /// let mut sk = Sketch::new();
    /// let a = sk.point(0.0, 0.0)?;
    /// let b = sk.point(1.0, 0.0)?;
    /// sk.line(a, b)?;
    /// assert!(sk.line(a, a).is_err());
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn line(&mut self, a: Point, b: Point) -> Result<Line, ModelError> {
        let id = self.mint()?;
        self.line_with_id(id, a, b)
    }

    /// Adds a line segment under the caller's id; refuses like
    /// [`Sketch::line`] and an id in use (`DuplicateId`).
    ///
    /// ```
    /// use laman::model::{EntityId, Sketch};
    /// let mut sk = Sketch::new();
    /// let a = sk.point(0.0, 0.0)?;
    /// let b = sk.point(1.0, 0.0)?;
    /// let l = sk.line_with_id(EntityId(5), a, b)?;
    /// assert_eq!(l.id(), EntityId(5));
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn line_with_id(&mut self, id: EntityId, a: Point, b: Point) -> Result<Line, ModelError> {
        self.fresh(id)?;
        self.points_exist(&[a, b])?;
        self.distinct(&[a, b], EntityKind::Line)?;
        self.insert(id, Entity::Line { a, b });
        Ok(Line(id))
    }

    /// Adds a circle about `c` of radius `r` under a minted id.
    ///
    /// Refuses a point not in the sketch (`UnknownId`, `WrongKind`), a
    /// non-finite radius (`NonFinite`) and a negative one
    /// (`DimensionOutOfRange`).
    ///
    /// ```
    /// use laman::model::Sketch;
    /// let mut sk = Sketch::new();
    /// let c = sk.point(0.0, 0.0)?;
    /// sk.circle(c, 5.0)?;
    /// assert!(sk.circle(c, -1.0).is_err());
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn circle(&mut self, c: Point, r: f64) -> Result<Circle, ModelError> {
        let id = self.mint()?;
        self.circle_with_id(id, c, r)
    }

    /// Adds a circle under the caller's id; refuses like [`Sketch::circle`]
    /// and an id in use (`DuplicateId`).
    ///
    /// ```
    /// use laman::model::{EntityId, Sketch};
    /// let mut sk = Sketch::new();
    /// let c = sk.point(0.0, 0.0)?;
    /// sk.circle_with_id(EntityId(9), c, 1.0)?;
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn circle_with_id(&mut self, id: EntityId, c: Point, r: f64) -> Result<Circle, ModelError> {
        finite("radius", r)?;
        if r < 0.0 {
            return Err(ModelError::DimensionOutOfRange {
                what: "radius",
                value: r,
                wanted: "r >= 0",
            });
        }
        self.fresh(id)?;
        self.points_exist(&[c])?;
        self.insert(id, Entity::Circle { c, r });
        Ok(Circle(id))
    }

    /// Adds an arc about `c`, counter-clockwise from `s` to `e`, under a
    /// minted id. Its radius is implied: `|s - c|`.
    ///
    /// Refuses a point not in the sketch (`UnknownId`, `WrongKind`) and a
    /// point named twice (`RepeatedEntity`).
    ///
    /// ```
    /// use laman::model::Sketch;
    /// let mut sk = Sketch::new();
    /// let c = sk.point(0.0, 0.0)?;
    /// let s = sk.point(1.0, 0.0)?;
    /// let e = sk.point(0.0, 1.0)?;
    /// sk.arc(c, s, e)?;
    /// assert!(sk.arc(c, s, s).is_err());
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn arc(&mut self, c: Point, s: Point, e: Point) -> Result<Arc, ModelError> {
        let id = self.mint()?;
        self.arc_with_id(id, c, s, e)
    }

    /// Adds an arc under the caller's id; refuses like [`Sketch::arc`] and
    /// an id in use (`DuplicateId`).
    ///
    /// ```
    /// use laman::model::{EntityId, Sketch};
    /// let mut sk = Sketch::new();
    /// let c = sk.point(0.0, 0.0)?;
    /// let s = sk.point(1.0, 0.0)?;
    /// let e = sk.point(0.0, 1.0)?;
    /// sk.arc_with_id(EntityId(20), c, s, e)?;
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn arc_with_id(
        &mut self,
        id: EntityId,
        c: Point,
        s: Point,
        e: Point,
    ) -> Result<Arc, ModelError> {
        self.fresh(id)?;
        self.points_exist(&[c, s, e])?;
        self.distinct(&[c, s, e], EntityKind::Arc)?;
        self.insert(id, Entity::Arc { c, s, e });
        Ok(Arc(id))
    }

    fn points_exist(&self, points: &[Point]) -> Result<(), ModelError> {
        points
            .iter()
            .try_for_each(|p| self.expect(p.id(), EntityKind::Point))
    }

    fn distinct(&self, points: &[Point], kind: EntityKind) -> Result<(), ModelError> {
        for (i, p) in points.iter().enumerate() {
            if points[..i].contains(p) {
                return Err(ModelError::RepeatedEntity { id: p.id(), kind });
            }
        }
        Ok(())
    }

    /// Whether this parameter is fixed (ground). False for an unknown id or
    /// a parameter the entity does not have.
    ///
    /// ```
    /// use laman::model::{Coordinate, Sketch};
    /// let mut sk = Sketch::new();
    /// let p = sk.point(0.0, 0.0)?;
    /// assert!(!sk.is_fixed(p.id(), Coordinate::X));
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn is_fixed(&self, id: EntityId, coordinate: Coordinate) -> bool {
        self.fixed.contains(&(id, coordinate))
    }

    /// Fixes or frees one parameter.
    ///
    /// Refuses an unknown id (`UnknownId`) and a parameter the entity does
    /// not have, such as a line's `x` (`NoSuchParameter`).
    ///
    /// ```
    /// use laman::model::{Coordinate, Sketch};
    /// let mut sk = Sketch::new();
    /// let p = sk.point(0.0, 0.0)?;
    /// sk.set_fixed(p.id(), Coordinate::X, true)?;
    /// assert!(sk.is_fixed(p.id(), Coordinate::X));
    /// assert!(sk.set_fixed(p.id(), Coordinate::R, true).is_err());
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn set_fixed(
        &mut self,
        id: EntityId,
        coordinate: Coordinate,
        fixed: bool,
    ) -> Result<(), ModelError> {
        let has = match self.kind_of(id)? {
            EntityKind::Point => matches!(coordinate, Coordinate::X | Coordinate::Y),
            EntityKind::Circle => matches!(coordinate, Coordinate::R),
            EntityKind::Line | EntityKind::Arc => false,
        };
        if !has {
            return Err(ModelError::NoSuchParameter { id, coordinate });
        }
        if fixed {
            self.fixed.insert((id, coordinate));
        } else {
            self.fixed.remove(&(id, coordinate));
        }
        self.revision += 1;
        Ok(())
    }

    /// Flags an entity as construction (or clears the flag): it solves like
    /// any other and is left out of profiles.
    ///
    /// Refuses an unknown id (`UnknownId`).
    ///
    /// ```
    /// use laman::model::Sketch;
    /// let mut sk = Sketch::new();
    /// let p = sk.point(0.0, 0.0)?;
    /// sk.set_construction(p.id(), true)?;
    /// assert!(sk.is_construction(p.id()));
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn set_construction(&mut self, id: EntityId, construction: bool) -> Result<(), ModelError> {
        self.kind_of(id)?;
        if construction {
            self.construction.insert(id);
        } else {
            self.construction.remove(&id);
        }
        self.revision += 1;
        Ok(())
    }

    /// Whether the entity is flagged construction; false for an unknown id.
    ///
    /// ```
    /// use laman::model::Sketch;
    /// let mut sk = Sketch::new();
    /// let p = sk.point(0.0, 0.0)?;
    /// assert!(!sk.is_construction(p.id()));
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn is_construction(&self, id: EntityId) -> bool {
        self.construction.contains(&id)
    }

    /// Attaches a caller string, returned in every diagnostic naming the
    /// entity. Replaces an earlier tag.
    ///
    /// Refuses an unknown id (`UnknownId`).
    ///
    /// ```
    /// use laman::model::{EntityId, Sketch};
    /// let mut sk = Sketch::new();
    /// let p = sk.point(0.0, 0.0)?;
    /// sk.tag(p.id(), "origin")?;
    /// assert_eq!(sk.tag_of(p.id()), Some("origin"));
    /// assert!(sk.tag(EntityId(9), "x").is_err());
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn tag(&mut self, id: EntityId, tag: &str) -> Result<(), ModelError> {
        self.kind_of(id)?;
        self.tags.insert(id, tag.to_owned());
        self.revision += 1;
        Ok(())
    }

    /// The entity's tag, if it has one.
    ///
    /// ```
    /// use laman::model::Sketch;
    /// let mut sk = Sketch::new();
    /// let p = sk.point(0.0, 0.0)?;
    /// assert_eq!(sk.tag_of(p.id()), None);
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn tag_of(&self, id: EntityId) -> Option<&str> {
        self.tags.get(&id).map(String::as_str)
    }

    /// Attaches an opaque caller value, stored and returned untouched.
    /// Replaces an earlier payload.
    ///
    /// Refuses an unknown id (`UnknownId`).
    ///
    /// ```
    /// use laman::model::{Payload, Sketch};
    /// let mut sk = Sketch::new();
    /// let p = sk.point(0.0, 0.0)?;
    /// sk.payload(p.id(), Payload(vec![7]))?;
    /// assert_eq!(sk.payload_of(p.id()), Some(&Payload(vec![7])));
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn payload(&mut self, id: EntityId, payload: Payload) -> Result<(), ModelError> {
        self.kind_of(id)?;
        self.payloads.insert(id, payload);
        self.revision += 1;
        Ok(())
    }

    /// The entity's payload, if it has one.
    ///
    /// ```
    /// use laman::model::Sketch;
    /// let mut sk = Sketch::new();
    /// let p = sk.point(0.0, 0.0)?;
    /// assert_eq!(sk.payload_of(p.id()), None);
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn payload_of(&self, id: EntityId) -> Option<&Payload> {
        self.payloads.get(&id)
    }

    fn dependents(&self, id: EntityId) -> Vec<EntityId> {
        self.entities
            .iter()
            .filter(|(_, e)| e.refs().contains(&id))
            .map(|(other, _)| *other)
            .collect()
    }

    fn forget(&mut self, id: EntityId) {
        self.entities.remove(&id);
        self.construction.remove(&id);
        self.tags.remove(&id);
        self.payloads.remove(&id);
        self.fixed.retain(|(e, _)| *e != id);
    }

    /// Removes an entity nothing uses. Its id is never minted again.
    ///
    /// Refuses an unknown id (`UnknownId`) and an entity that others, or
    /// constraints, still use (`InUse`, naming them).
    ///
    /// ```
    /// use laman::model::{ModelError, Sketch};
    /// let mut sk = Sketch::new();
    /// let a = sk.point(0.0, 0.0)?;
    /// let b = sk.point(1.0, 0.0)?;
    /// let l = sk.line(a, b)?;
    /// assert!(matches!(sk.remove(a.id()), Err(ModelError::InUse { .. })));
    /// sk.remove(l.id())?;
    /// sk.remove(a.id())?;
    /// # Ok::<(), ModelError>(())
    /// ```
    pub fn remove(&mut self, id: EntityId) -> Result<(), ModelError> {
        self.kind_of(id)?;
        let entities = self.dependents(id);
        let constraints = self.constraints_using(&BTreeSet::from([id]));
        if !entities.is_empty() || !constraints.is_empty() {
            return Err(ModelError::InUse {
                id,
                entities,
                constraints,
            });
        }
        self.forget(id);
        self.revision += 1;
        Ok(())
    }

    /// Removes an entity and everything that depends on it, transitively
    /// (entities, then the constraints that name any of them), and returns
    /// what went.
    ///
    /// Refuses an unknown id (`UnknownId`).
    ///
    /// ```
    /// use laman::model::Sketch;
    /// let mut sk = Sketch::new();
    /// let a = sk.point(0.0, 0.0)?;
    /// let b = sk.point(1.0, 0.0)?;
    /// sk.line(a, b)?;
    /// assert_eq!(sk.remove_with_constraints(b.id())?.entities.len(), 2);
    /// assert_eq!(sk.entities().count(), 1);
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn remove_with_constraints(&mut self, id: EntityId) -> Result<Removed, ModelError> {
        self.kind_of(id)?;
        let mut gone = BTreeSet::from([id]);
        let mut frontier = vec![id];
        while let Some(next) = frontier.pop() {
            for d in self.dependents(next) {
                if gone.insert(d) {
                    frontier.push(d);
                }
            }
        }
        let constraints = self.constraints_using(&gone);
        for c in &constraints {
            self.forget_constraint(*c);
        }
        for g in &gone {
            self.forget(*g);
        }
        self.revision += 1;
        Ok(Removed {
            entities: gone.into_iter().collect(),
            constraints,
        })
    }
}
