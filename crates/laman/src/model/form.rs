//! The serde form of a sketch (`docs/DATA-MODEL.md` §The serde form).
//!
//! A sketch serialises through [`SketchForm`], a plain description of the
//! file, and loads by replaying the builder over it, so a file is validated
//! exactly as a program that built the same sketch would be.

use std::collections::BTreeSet;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{
    Constraint, ConstraintId, Contact, Coordinate, Entity, EntityId, EntityKind, ModelError,
    Payload, Round, Sense, Sketch, Span,
};

/// The `format` member every file carries.
const FORMAT: &str = "laman.sketch";

/// The newest `version` this reader loads and the one it writes.
const VERSION: u64 = 1;

/// A sketch as the file describes it: the document a serde format reads and
/// writes, before it is validated.
///
/// Get one from [`Sketch::to_form`] and turn it back with
/// [`Sketch::from_form`], which is where a malformed file is refused. The
/// members are private; the shape is `DATA-MODEL.md` §The serde form.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SketchForm {
    format: String,
    version: u64,
    #[serde(default)]
    entities: Vec<EntityForm>,
    #[serde(default)]
    constraints: Vec<ConstraintForm>,
}

impl SketchForm {
    /// The `version` the file declares.
    ///
    /// ```
    /// use laman::model::Sketch;
    /// assert_eq!(Sketch::new().to_form().version(), 1);
    /// ```
    pub fn version(&self) -> u64 {
        self.version
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct EntityForm {
    id: u32,
    #[serde(flatten)]
    body: EntityBody,
    #[serde(default, skip_serializing_if = "is_false")]
    construction: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    fixed: Vec<FixedName>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    payload: Option<String>,
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum FixedName {
    X,
    Y,
    R,
}

impl From<FixedName> for Coordinate {
    fn from(f: FixedName) -> Self {
        match f {
            FixedName::X => Coordinate::X,
            FixedName::Y => Coordinate::Y,
            FixedName::R => Coordinate::R,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum EntityBody {
    Point { x: f64, y: f64 },
    Line { a: u32, b: u32 },
    Circle { c: u32, r: f64 },
    Arc { c: u32, s: u32, e: u32 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct ConstraintForm {
    id: u32,
    #[serde(flatten)]
    body: ConstraintBody,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    payload: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ConstraintBody {
    Coincident {
        a: u32,
        b: u32,
    },
    PointOnLine {
        point: u32,
        line: u32,
    },
    PointOnCircle {
        point: u32,
        circle: u32,
    },
    Horizontal(SpanForm),
    Vertical(SpanForm),
    Parallel {
        a: u32,
        b: u32,
    },
    Perpendicular {
        a: u32,
        b: u32,
    },
    EqualLength {
        a: u32,
        b: u32,
    },
    EqualRadius {
        a: u32,
        b: u32,
    },
    Tangent(TangentForm),
    Midpoint {
        point: u32,
        line: u32,
    },
    Symmetric {
        a: u32,
        b: u32,
        axis: u32,
    },
    Fix {
        point: u32,
        x: f64,
        y: f64,
    },
    Distance {
        a: u32,
        b: u32,
        value: f64,
    },
    PointLineDistance {
        point: u32,
        line: u32,
        value: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sense: Option<i64>,
    },
    Radius {
        circle: u32,
        value: f64,
    },
    Angle {
        a: u32,
        b: u32,
        value: f64,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
enum SpanForm {
    Line { line: u32 },
    Points { a: u32, b: u32 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
enum TangentForm {
    LineCircle {
        line: u32,
        circle: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sense: Option<i64>,
    },
    Circles {
        a: u32,
        b: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        contact: Option<ContactName>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sense: Option<i64>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ContactName {
    External,
    Internal,
}

fn sense_to_form(s: Sense) -> i64 {
    match s {
        Sense::Positive => 1,
        Sense::Negative => -1,
    }
}

fn sense_from_form(s: Option<i64>) -> Result<Option<Sense>, ModelError> {
    match s {
        None => Ok(None),
        Some(1) => Ok(Some(Sense::Positive)),
        Some(-1) => Ok(Some(Sense::Negative)),
        Some(other) => Err(ModelError::DimensionOutOfRange {
            what: "sense",
            // Exact: the file's integer, shown as the model's one number type.
            value: other as f64,
            wanted: "1 or -1",
        }),
    }
}

fn encode(p: &Payload) -> String {
    STANDARD.encode(&p.0)
}

fn decode(s: &str) -> Result<Payload, ModelError> {
    STANDARD
        .decode(s)
        .map(Payload)
        .map_err(|_| ModelError::InvalidPayload)
}

fn at_entity(id: u32) -> impl FnOnce(ModelError) -> ModelError {
    move |e| ModelError::Entity {
        id: EntityId(id),
        source: Box::new(e),
    }
}

fn at_constraint(id: u32) -> impl FnOnce(ModelError) -> ModelError {
    move |e| ModelError::Constraint {
        id: ConstraintId(id),
        source: Box::new(e),
    }
}

impl Sketch {
    /// The sketch as its file description, entities in id order (an
    /// entity's points before it, should ids have been chosen otherwise)
    /// and constraints in id order.
    ///
    /// ```
    /// use laman::model::Sketch;
    /// let mut sk = Sketch::new();
    /// sk.point(1.0, 2.0)?;
    /// let form = sk.to_form();
    /// assert_eq!(Sketch::from_form(form)?.entities().count(), 1);
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn to_form(&self) -> SketchForm {
        let mut entities = Vec::new();
        let mut done = BTreeSet::new();
        for (&id, entity) in &self.entities {
            for r in entity.refs() {
                if done.insert(r) {
                    if let Some(dep) = self.entities.get(&r) {
                        entities.push(self.entity_form(r, dep));
                    }
                }
            }
            if done.insert(id) {
                entities.push(self.entity_form(id, entity));
            }
        }
        let constraints = self
            .constraints
            .iter()
            .map(|(&id, c)| ConstraintForm {
                id: id.0,
                body: constraint_body(c),
                tag: self.constraint_tags.get(&id).cloned(),
                payload: self.constraint_payloads.get(&id).map(encode),
            })
            .collect();
        SketchForm {
            format: FORMAT.to_owned(),
            version: VERSION,
            entities,
            constraints,
        }
    }

    fn entity_form(&self, id: EntityId, entity: &Entity) -> EntityForm {
        let body = match entity {
            Entity::Point { x, y } => EntityBody::Point { x: *x, y: *y },
            Entity::Line { a, b } => EntityBody::Line {
                a: a.id().0,
                b: b.id().0,
            },
            Entity::Circle { c, r } => EntityBody::Circle { c: c.id().0, r: *r },
            Entity::Arc { c, s, e } => EntityBody::Arc {
                c: c.id().0,
                s: s.id().0,
                e: e.id().0,
            },
        };
        let fixed = [
            (Coordinate::X, FixedName::X),
            (Coordinate::Y, FixedName::Y),
            (Coordinate::R, FixedName::R),
        ]
        .into_iter()
        .filter(|(c, _)| self.is_fixed(id, *c))
        .map(|(_, n)| n)
        .collect();
        EntityForm {
            id: id.0,
            body,
            construction: self.construction.contains(&id),
            fixed,
            tag: self.tags.get(&id).cloned(),
            payload: self.payloads.get(&id).map(encode),
        }
    }

    /// Builds a sketch from its file description by replaying the builder,
    /// so a file is validated exactly as a program would be.
    ///
    /// Refuses a `format` that is not `laman.sketch` (`WrongFormat`), a
    /// `version` that is 0 or newer than this reader (`FormatVersion`), and
    /// anything the builder refuses, wrapped in the entity or constraint of
    /// the file it came from (`Entity`, `Constraint`): an unknown or
    /// wrong-kind reference, a repeated id, a non-finite value, a distance
    /// out of range, a payload that is not base64 (`InvalidPayload`). A
    /// missing sense is taken from the coordinates, as on `add`.
    ///
    /// ```
    /// use laman::model::{EntityId, ModelError, Sketch};
    /// let mut sk = Sketch::new();
    /// let (a, b) = (sk.point(0.0, 0.0)?, sk.point(3.0, 4.0)?);
    /// sk.distance(a, b, 5.0)?;
    /// let loaded = Sketch::from_form(sk.to_form())?;
    /// assert_eq!(loaded.constraints().count(), 1);
    /// assert!(matches!(
    ///     Sketch::from_form(Sketch::new().to_form()),
    ///     Ok(s) if s.entities().count() == 0
    /// ));
    /// # Ok::<(), ModelError>(())
    /// ```
    pub fn from_form(form: SketchForm) -> Result<Sketch, ModelError> {
        let form = migrate(form)?;
        let mut sk = Sketch::new();
        for e in &form.entities {
            sk.load_entity(e).map_err(at_entity(e.id))?;
        }
        for c in &form.constraints {
            sk.load_constraint(c).map_err(at_constraint(c.id))?;
        }
        Ok(sk)
    }

    fn load_entity(&mut self, e: &EntityForm) -> Result<(), ModelError> {
        let id = EntityId(e.id);
        match &e.body {
            EntityBody::Point { x, y } => {
                self.point_with_id(id, *x, *y)?;
            }
            EntityBody::Line { a, b } => {
                let (a, b) = (
                    self.point_handle(EntityId(*a))?,
                    self.point_handle(EntityId(*b))?,
                );
                self.line_with_id(id, a, b)?;
            }
            EntityBody::Circle { c, r } => {
                let c = self.point_handle(EntityId(*c))?;
                self.circle_with_id(id, c, *r)?;
            }
            EntityBody::Arc { c, s, e } => {
                let (c, s, e) = (
                    self.point_handle(EntityId(*c))?,
                    self.point_handle(EntityId(*s))?,
                    self.point_handle(EntityId(*e))?,
                );
                self.arc_with_id(id, c, s, e)?;
            }
        }
        if e.construction {
            self.set_construction(id, true)?;
        }
        for f in &e.fixed {
            self.set_fixed(id, (*f).into(), true)?;
        }
        if let Some(tag) = &e.tag {
            self.tag(id, tag)?;
        }
        if let Some(p) = &e.payload {
            self.payload(id, decode(p)?)?;
        }
        Ok(())
    }

    fn round_handle(&self, id: u32) -> Result<Round, ModelError> {
        let id = EntityId(id);
        match self.kind_of(id)? {
            EntityKind::Circle => Ok(Round::Circle(self.circle_handle(id)?)),
            EntityKind::Arc => Ok(Round::Arc(self.arc_handle(id)?)),
            found @ (EntityKind::Point | EntityKind::Line) => Err(ModelError::WrongKind {
                id,
                expected: EntityKind::Circle,
                found,
            }),
        }
    }

    fn span_from(&self, s: &SpanForm) -> Result<Span, ModelError> {
        Ok(match s {
            SpanForm::Line { line } => Span::Line(self.line_handle(EntityId(*line))?),
            SpanForm::Points { a, b } => Span::Points(
                self.point_handle(EntityId(*a))?,
                self.point_handle(EntityId(*b))?,
            ),
        })
    }

    fn load_constraint(&mut self, c: &ConstraintForm) -> Result<(), ModelError> {
        let id = ConstraintId(c.id);
        let constraint = self.constraint_from(&c.body)?;
        self.add_constraint_with_id(id, constraint)?;
        if let Some(tag) = &c.tag {
            self.tag_constraint(id, tag)?;
        }
        if let Some(p) = &c.payload {
            self.payload_constraint(id, decode(p)?)?;
        }
        Ok(())
    }

    fn constraint_from(&self, body: &ConstraintBody) -> Result<Constraint, ModelError> {
        let pt = |i: u32| self.point_handle(EntityId(i));
        let ln = |i: u32| self.line_handle(EntityId(i));
        Ok(match body {
            ConstraintBody::Coincident { a, b } => Constraint::Coincident {
                p: pt(*a)?,
                q: pt(*b)?,
            },
            ConstraintBody::PointOnLine { point, line } => Constraint::PointOnLine {
                p: pt(*point)?,
                line: ln(*line)?,
            },
            ConstraintBody::PointOnCircle { point, circle } => Constraint::PointOnCircle {
                p: pt(*point)?,
                on: self.round_handle(*circle)?,
            },
            ConstraintBody::Horizontal(s) => Constraint::Horizontal(self.span_from(s)?),
            ConstraintBody::Vertical(s) => Constraint::Vertical(self.span_from(s)?),
            ConstraintBody::Parallel { a, b } => Constraint::Parallel {
                a: ln(*a)?,
                b: ln(*b)?,
            },
            ConstraintBody::Perpendicular { a, b } => Constraint::Perpendicular {
                a: ln(*a)?,
                b: ln(*b)?,
            },
            ConstraintBody::EqualLength { a, b } => Constraint::EqualLength {
                a: ln(*a)?,
                b: ln(*b)?,
            },
            ConstraintBody::EqualRadius { a, b } => Constraint::EqualRadius {
                a: self.round_handle(*a)?,
                b: self.round_handle(*b)?,
            },
            ConstraintBody::Tangent(TangentForm::LineCircle {
                line,
                circle,
                sense,
            }) => {
                let (line, round) = (ln(*line)?, self.round_handle(*circle)?);
                let sense = match sense_from_form(*sense)? {
                    Some(s) => s,
                    None => self.side_of(line, self.centre_radius(round)?.0)?,
                };
                Constraint::TangentLineCircle { line, round, sense }
            }
            ConstraintBody::Tangent(TangentForm::Circles {
                a,
                b,
                contact,
                sense,
            }) => {
                let (a, b) = (self.round_handle(*a)?, self.round_handle(*b)?);
                let sense = sense_from_form(*sense)?;
                let contact = match (contact, sense) {
                    (Some(ContactName::External), _) => Contact::External,
                    (Some(ContactName::Internal), Some(s)) => Contact::Internal(s),
                    (Some(ContactName::Internal), None) => {
                        let ((_, ra), (_, rb)) = (self.centre_radius(a)?, self.centre_radius(b)?);
                        Contact::Internal(Sense::of(ra - rb))
                    }
                    (None, _) => self.contact_of(a, b)?,
                };
                Constraint::TangentCircles { a, b, contact }
            }
            ConstraintBody::Midpoint { point, line } => Constraint::Midpoint {
                p: pt(*point)?,
                line: ln(*line)?,
            },
            ConstraintBody::Symmetric { a, b, axis } => Constraint::Symmetric {
                p: pt(*a)?,
                q: pt(*b)?,
                axis: ln(*axis)?,
            },
            ConstraintBody::Fix { point, x, y } => Constraint::Fix {
                p: pt(*point)?,
                x: *x,
                y: *y,
            },
            ConstraintBody::Distance { a, b, value } => Constraint::Distance {
                p: pt(*a)?,
                q: pt(*b)?,
                value: *value,
            },
            ConstraintBody::PointLineDistance {
                point,
                line,
                value,
                sense,
            } => {
                let (p, line) = (pt(*point)?, ln(*line)?);
                let sense = match sense_from_form(*sense)? {
                    Some(s) => s,
                    None => self.side_of(line, self.xy(p)?)?,
                };
                Constraint::PointLineDistance {
                    p,
                    line,
                    value: *value,
                    sense,
                }
            }
            ConstraintBody::Radius { circle, value } => Constraint::Radius {
                round: self.round_handle(*circle)?,
                value: *value,
            },
            ConstraintBody::Angle { a, b, value } => Constraint::Angle {
                a: ln(*a)?,
                b: ln(*b)?,
                value: *value,
            },
        })
    }
}

/// The seam for older versions: each arm brings a file of that version to
/// the next one in code. Version 1 is the only one, so nothing migrates yet.
fn migrate(form: SketchForm) -> Result<SketchForm, ModelError> {
    if form.format != FORMAT {
        return Err(ModelError::WrongFormat { found: form.format });
    }
    match form.version {
        VERSION => Ok(form),
        found => Err(ModelError::FormatVersion {
            found,
            supported: VERSION,
        }),
    }
}

fn constraint_body(c: &Constraint) -> ConstraintBody {
    let span = |s: &Span| match s {
        Span::Line(l) => SpanForm::Line { line: l.id().0 },
        Span::Points(p, q) => SpanForm::Points {
            a: p.id().0,
            b: q.id().0,
        },
    };
    match c {
        Constraint::Coincident { p, q } => ConstraintBody::Coincident {
            a: p.id().0,
            b: q.id().0,
        },
        Constraint::PointOnLine { p, line } => ConstraintBody::PointOnLine {
            point: p.id().0,
            line: line.id().0,
        },
        Constraint::PointOnCircle { p, on } => ConstraintBody::PointOnCircle {
            point: p.id().0,
            circle: on.id().0,
        },
        Constraint::Horizontal(s) => ConstraintBody::Horizontal(span(s)),
        Constraint::Vertical(s) => ConstraintBody::Vertical(span(s)),
        Constraint::Parallel { a, b } => ConstraintBody::Parallel {
            a: a.id().0,
            b: b.id().0,
        },
        Constraint::Perpendicular { a, b } => ConstraintBody::Perpendicular {
            a: a.id().0,
            b: b.id().0,
        },
        Constraint::EqualLength { a, b } => ConstraintBody::EqualLength {
            a: a.id().0,
            b: b.id().0,
        },
        Constraint::EqualRadius { a, b } => ConstraintBody::EqualRadius {
            a: a.id().0,
            b: b.id().0,
        },
        Constraint::TangentLineCircle { line, round, sense } => {
            ConstraintBody::Tangent(TangentForm::LineCircle {
                line: line.id().0,
                circle: round.id().0,
                sense: Some(sense_to_form(*sense)),
            })
        }
        Constraint::TangentCircles { a, b, contact } => {
            let (contact, sense) = match contact {
                Contact::External => (ContactName::External, None),
                Contact::Internal(s) => (ContactName::Internal, Some(sense_to_form(*s))),
            };
            ConstraintBody::Tangent(TangentForm::Circles {
                a: a.id().0,
                b: b.id().0,
                contact: Some(contact),
                sense,
            })
        }
        Constraint::Midpoint { p, line } => ConstraintBody::Midpoint {
            point: p.id().0,
            line: line.id().0,
        },
        Constraint::Symmetric { p, q, axis } => ConstraintBody::Symmetric {
            a: p.id().0,
            b: q.id().0,
            axis: axis.id().0,
        },
        Constraint::Fix { p, x, y } => ConstraintBody::Fix {
            point: p.id().0,
            x: *x,
            y: *y,
        },
        Constraint::Distance { p, q, value } => ConstraintBody::Distance {
            a: p.id().0,
            b: q.id().0,
            value: *value,
        },
        Constraint::PointLineDistance {
            p,
            line,
            value,
            sense,
        } => ConstraintBody::PointLineDistance {
            point: p.id().0,
            line: line.id().0,
            value: *value,
            sense: Some(sense_to_form(*sense)),
        },
        Constraint::Radius { round, value } => ConstraintBody::Radius {
            circle: round.id().0,
            value: *value,
        },
        Constraint::Angle { a, b, value } => ConstraintBody::Angle {
            a: a.id().0,
            b: b.id().0,
            value: *value,
        },
    }
}

impl Serialize for Sketch {
    /// Writes the serde form (`DATA-MODEL.md` §The serde form).
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.to_form().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Sketch {
    /// Reads the serde form, validating as the builder does. The format's
    /// error carries the `ModelError`'s message, naming the id.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let form = SketchForm::deserialize(deserializer)?;
        Sketch::from_form(form).map_err(serde::de::Error::custom)
    }
}
