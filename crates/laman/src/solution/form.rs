//! The serde form of a [`Solution`] (`docs/DATA-MODEL.md` §The serde form).
//!
//! Reading one gives a `Solution` and nothing more: it carries the
//! revision and digest of the sketch it claims to come from, and
//! [`crate::model::Sketch::apply`] validates it like any other, so a file cannot put
//! coordinates into a sketch it was not solved for.

use super::{
    Bisection, Collapse, ConstraintRef, Degenerate, Diagnostics, EntityFreedom, EntityRef, Flip,
    FlipSubject, Motion, Origin, Provenance, Solution, Status,
};
use crate::model::{ConstraintId, ConstraintKind, Coordinate, EntityId, EntityKind};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// The `format` member every file carries.
const FORMAT: &str = "laman.solution";

/// The newest `version` this reader loads and the one it writes.
const VERSION: u64 = 1;

#[derive(Serialize, Deserialize)]
struct SolutionForm {
    format: String,
    version: u64,
    source: SourceForm,
    status: Status,
    coordinates: Vec<CoordinateForm>,
    diagnostics: DiagnosticsForm,
}

#[derive(Serialize, Deserialize)]
struct SourceForm {
    revision: u64,
    /// 16 lower-case hex digits.
    digest: String,
}

#[derive(Serialize, Deserialize)]
struct CoordinateForm {
    id: u32,
    coordinate: CoordinateName,
    value: f64,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum CoordinateName {
    X,
    Y,
    R,
}

impl From<Coordinate> for CoordinateName {
    fn from(c: Coordinate) -> Self {
        match c {
            Coordinate::X => Self::X,
            Coordinate::Y => Self::Y,
            Coordinate::R => Self::R,
        }
    }
}

impl From<CoordinateName> for Coordinate {
    fn from(c: CoordinateName) -> Self {
        match c {
            CoordinateName::X => Self::X,
            CoordinateName::Y => Self::Y,
            CoordinateName::R => Self::R,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum EntityKindName {
    Point,
    Line,
    Circle,
    Arc,
}

impl From<EntityKind> for EntityKindName {
    fn from(k: EntityKind) -> Self {
        match k {
            EntityKind::Point => Self::Point,
            EntityKind::Line => Self::Line,
            EntityKind::Circle => Self::Circle,
            EntityKind::Arc => Self::Arc,
        }
    }
}

impl From<EntityKindName> for EntityKind {
    fn from(k: EntityKindName) -> Self {
        match k {
            EntityKindName::Point => Self::Point,
            EntityKindName::Line => Self::Line,
            EntityKindName::Circle => Self::Circle,
            EntityKindName::Arc => Self::Arc,
        }
    }
}

/// A constraint kind by name. Unlike the sketch form's `tangent`, which the
/// fields disambiguate, a diagnostic has no fields: the two tangents are
/// `tangent_line_circle` and `tangent_circles`, so a read gives back the
/// kind it was written from.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ConstraintKindName {
    Coincident,
    PointOnLine,
    PointOnCircle,
    Horizontal,
    Vertical,
    Parallel,
    Perpendicular,
    EqualLength,
    EqualRadius,
    TangentLineCircle,
    TangentCircles,
    Midpoint,
    Symmetric,
    Fix,
    Distance,
    PointLineDistance,
    Radius,
    Angle,
}

impl From<ConstraintKind> for ConstraintKindName {
    fn from(k: ConstraintKind) -> Self {
        match k {
            ConstraintKind::Coincident => Self::Coincident,
            ConstraintKind::PointOnLine => Self::PointOnLine,
            ConstraintKind::PointOnCircle => Self::PointOnCircle,
            ConstraintKind::Horizontal => Self::Horizontal,
            ConstraintKind::Vertical => Self::Vertical,
            ConstraintKind::Parallel => Self::Parallel,
            ConstraintKind::Perpendicular => Self::Perpendicular,
            ConstraintKind::EqualLength => Self::EqualLength,
            ConstraintKind::EqualRadius => Self::EqualRadius,
            ConstraintKind::TangentLineCircle => Self::TangentLineCircle,
            ConstraintKind::TangentCircles => Self::TangentCircles,
            ConstraintKind::Midpoint => Self::Midpoint,
            ConstraintKind::Symmetric => Self::Symmetric,
            ConstraintKind::Fix => Self::Fix,
            ConstraintKind::Distance => Self::Distance,
            ConstraintKind::PointLineDistance => Self::PointLineDistance,
            ConstraintKind::Radius => Self::Radius,
            ConstraintKind::Angle => Self::Angle,
        }
    }
}

impl From<ConstraintKindName> for ConstraintKind {
    fn from(k: ConstraintKindName) -> Self {
        match k {
            ConstraintKindName::Coincident => Self::Coincident,
            ConstraintKindName::PointOnLine => Self::PointOnLine,
            ConstraintKindName::PointOnCircle => Self::PointOnCircle,
            ConstraintKindName::Horizontal => Self::Horizontal,
            ConstraintKindName::Vertical => Self::Vertical,
            ConstraintKindName::Parallel => Self::Parallel,
            ConstraintKindName::Perpendicular => Self::Perpendicular,
            ConstraintKindName::EqualLength => Self::EqualLength,
            ConstraintKindName::EqualRadius => Self::EqualRadius,
            ConstraintKindName::TangentLineCircle => Self::TangentLineCircle,
            ConstraintKindName::TangentCircles => Self::TangentCircles,
            ConstraintKindName::Midpoint => Self::Midpoint,
            ConstraintKindName::Symmetric => Self::Symmetric,
            ConstraintKindName::Fix => Self::Fix,
            ConstraintKindName::Distance => Self::Distance,
            ConstraintKindName::PointLineDistance => Self::PointLineDistance,
            ConstraintKindName::Radius => Self::Radius,
            ConstraintKindName::Angle => Self::Angle,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct EntityRefForm {
    id: u32,
    kind: EntityKindName,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tag: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct ConstraintRefForm {
    id: u32,
    kind: ConstraintKindName,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tag: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "from", rename_all = "snake_case")]
enum OriginForm {
    Constraint(ConstraintRefForm),
    Arc(EntityRefForm),
}

#[derive(Serialize, Deserialize)]
struct EntityFreedomForm {
    #[serde(flatten)]
    entity: EntityRefForm,
    dof: usize,
}

#[derive(Serialize, Deserialize)]
struct MotionForm {
    id: u32,
    coordinate: CoordinateName,
    amount: f64,
}

#[derive(Serialize, Deserialize)]
struct DegenerateForm {
    #[serde(flatten)]
    entity: EntityRefForm,
    collapse: Collapse,
    /// `null` for a size that is not finite (read back as NaN).
    size: Option<f64>,
    equations: Vec<OriginForm>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "of", rename_all = "snake_case")]
enum FlipSubjectForm {
    Entity(EntityRefForm),
    Constraint(ConstraintRefForm),
}

#[derive(Serialize, Deserialize)]
struct FlipForm {
    #[serde(flatten)]
    subject: FlipSubjectForm,
    /// `null` for a quantity that is not finite (read back as NaN).
    was: Option<f64>,
    now: Option<f64>,
}

#[derive(Serialize, Deserialize)]
struct DiagnosticsForm {
    dof: usize,
    entities: Vec<EntityFreedomForm>,
    free_directions: Vec<Vec<MotionForm>>,
    redundant: Vec<ConstraintRefForm>,
    conflicts: Vec<Vec<ConstraintRefForm>>,
    degenerate: Vec<DegenerateForm>,
    ill_conditioned: Vec<OriginForm>,
    /// Absent when empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    flips: Vec<FlipForm>,
    /// Absent for a static solve and an unbisected frame.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bisection: Option<BisectionForm>,
}

#[derive(Serialize, Deserialize)]
struct BisectionForm {
    reached: [f64; 2],
    halvings: usize,
}

impl From<&EntityRef> for EntityRefForm {
    fn from(e: &EntityRef) -> Self {
        Self {
            id: e.id.0,
            kind: e.kind.into(),
            tag: e.tag.clone(),
        }
    }
}

impl From<EntityRefForm> for EntityRef {
    fn from(e: EntityRefForm) -> Self {
        Self {
            id: EntityId(e.id),
            kind: e.kind.into(),
            tag: e.tag,
        }
    }
}

impl From<&ConstraintRef> for ConstraintRefForm {
    fn from(c: &ConstraintRef) -> Self {
        Self {
            id: c.id.0,
            kind: c.kind.into(),
            tag: c.tag.clone(),
        }
    }
}

impl From<ConstraintRefForm> for ConstraintRef {
    fn from(c: ConstraintRefForm) -> Self {
        Self {
            id: ConstraintId(c.id),
            kind: c.kind.into(),
            tag: c.tag,
        }
    }
}

impl From<&Origin> for OriginForm {
    fn from(o: &Origin) -> Self {
        match o {
            Origin::Constraint(c) => Self::Constraint(c.into()),
            Origin::Arc(e) => Self::Arc(e.into()),
        }
    }
}

impl From<OriginForm> for Origin {
    fn from(o: OriginForm) -> Self {
        match o {
            OriginForm::Constraint(c) => Self::Constraint(c.into()),
            OriginForm::Arc(e) => Self::Arc(e.into()),
        }
    }
}

fn form_of(s: &Solution) -> SolutionForm {
    let d = &s.diagnostics;
    SolutionForm {
        format: FORMAT.to_owned(),
        version: VERSION,
        source: SourceForm {
            revision: s.source.revision,
            digest: format!("{:016x}", s.source.digest),
        },
        status: s.status,
        coordinates: s
            .coordinates
            .iter()
            .map(|(&(id, c), &value)| CoordinateForm {
                id: id.0,
                coordinate: c.into(),
                value,
            })
            .collect(),
        diagnostics: DiagnosticsForm {
            dof: d.dof,
            entities: d
                .entities
                .iter()
                .map(|e| EntityFreedomForm {
                    entity: (&e.entity).into(),
                    dof: e.dof,
                })
                .collect(),
            free_directions: d
                .free_directions
                .iter()
                .map(|v| {
                    v.iter()
                        .map(|m| MotionForm {
                            id: m.entity.0,
                            coordinate: m.coordinate.into(),
                            amount: m.amount,
                        })
                        .collect()
                })
                .collect(),
            redundant: d.redundant.iter().map(Into::into).collect(),
            conflicts: d
                .conflicts
                .iter()
                .map(|set| set.iter().map(Into::into).collect())
                .collect(),
            degenerate: d
                .degenerate
                .iter()
                .map(|g| DegenerateForm {
                    entity: (&g.entity).into(),
                    collapse: g.collapse,
                    size: g.size.is_finite().then_some(g.size),
                    equations: g.equations.iter().map(Into::into).collect(),
                })
                .collect(),
            ill_conditioned: d.ill_conditioned.iter().map(Into::into).collect(),
            flips: d
                .flips
                .iter()
                .map(|f| FlipForm {
                    subject: match &f.subject {
                        FlipSubject::Entity(e) => FlipSubjectForm::Entity(e.into()),
                        FlipSubject::Constraint(c) => FlipSubjectForm::Constraint(c.into()),
                    },
                    was: f.was.is_finite().then_some(f.was),
                    now: f.now.is_finite().then_some(f.now),
                })
                .collect(),
            bisection: d.bisection.as_ref().map(|b| BisectionForm {
                reached: b.reached,
                halvings: b.halvings,
            }),
        },
    }
}

fn solution_of(form: SolutionForm) -> Result<Solution, String> {
    if form.format != FORMAT {
        return Err(format!(
            "format {:?} is not a laman solution (wanted {FORMAT:?})",
            form.format
        ));
    }
    if form.version != VERSION {
        return Err(format!(
            "solution format version {} is not loadable here (supported: {VERSION})",
            form.version
        ));
    }
    let digest = (form.source.digest.len() == 16)
        .then(|| u64::from_str_radix(&form.source.digest, 16).ok())
        .flatten()
        .ok_or_else(|| {
            format!(
                "source digest {:?} is not 16 hex digits",
                form.source.digest
            )
        })?;
    let mut coordinates = crate::model::Coordinates::new();
    for c in form.coordinates {
        let key = (EntityId(c.id), c.coordinate.into());
        if coordinates.insert(key, c.value).is_some() {
            return Err(format!("{} {:?} is given twice", key.0, key.1));
        }
    }
    let d = form.diagnostics;
    Ok(Solution {
        coordinates,
        status: form.status,
        diagnostics: Diagnostics {
            dof: d.dof,
            entities: d
                .entities
                .into_iter()
                .map(|e| EntityFreedom {
                    entity: e.entity.into(),
                    dof: e.dof,
                })
                .collect(),
            free_directions: d
                .free_directions
                .into_iter()
                .map(|v| {
                    v.into_iter()
                        .map(|m| Motion {
                            entity: EntityId(m.id),
                            coordinate: m.coordinate.into(),
                            amount: m.amount,
                        })
                        .collect()
                })
                .collect(),
            redundant: d.redundant.into_iter().map(Into::into).collect(),
            conflicts: d
                .conflicts
                .into_iter()
                .map(|set| set.into_iter().map(Into::into).collect())
                .collect(),
            degenerate: d
                .degenerate
                .into_iter()
                .map(|g| Degenerate {
                    entity: g.entity.into(),
                    collapse: g.collapse,
                    size: g.size.unwrap_or(f64::NAN),
                    equations: g.equations.into_iter().map(Into::into).collect(),
                })
                .collect(),
            ill_conditioned: d.ill_conditioned.into_iter().map(Into::into).collect(),
            flips: d
                .flips
                .into_iter()
                .map(|f| Flip {
                    subject: match f.subject {
                        FlipSubjectForm::Entity(e) => FlipSubject::Entity(e.into()),
                        FlipSubjectForm::Constraint(c) => FlipSubject::Constraint(c.into()),
                    },
                    was: f.was.unwrap_or(f64::NAN),
                    now: f.now.unwrap_or(f64::NAN),
                })
                .collect(),
            bisection: d.bisection.map(|b| Bisection {
                reached: b.reached,
                halvings: b.halvings,
            }),
        },
        source: Provenance {
            revision: form.source.revision,
            digest,
        },
    })
}

impl Serialize for Solution {
    /// Writes the serde form, coordinates in id order.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        form_of(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Solution {
    /// Reads the serde form. A wrong `format`, an unsupported `version`, a
    /// malformed digest or a coordinate given twice is refused; whether the
    /// solution fits a sketch is [`crate::model::Sketch::apply`]'s to decide.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let form = SolutionForm::deserialize(deserializer)?;
        solution_of(form).map_err(serde::de::Error::custom)
    }
}
