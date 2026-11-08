//! The independent checker, `laman.check`.

use crate::model::{Coordinates, Sketch, coordinate_name_of};
use crate::profile::PyProfiles;
use laman_check::{Degeneracy, ProfileViolation, Source, Subject};
use pyo3::prelude::*;

/// One equation that does not hold at the checked coordinates.
#[pyclass(frozen, get_all, name = "Violation", module = "laman.check")]
pub struct CheckViolation {
    /// The constraint's id, or `None` for an arc's intrinsic equation.
    pub constraint: Option<u32>,
    /// The arc's entity id for its intrinsic equation, else `None`.
    pub arc: Option<u32>,
    /// The caller's tag on it, if any.
    pub tag: Option<String>,
    /// The constraint's kind name, or `"arc_intrinsic"`.
    pub kind: String,
    /// The value measured: a length, or an angle in radians.
    pub measured: f64,
    /// The value it should have had, in the same unit.
    pub wanted: f64,
}

/// An entity that collapsed at the checked coordinates.
#[pyclass(frozen, get_all, name = "Degenerate", module = "laman.check")]
pub struct CheckDegenerate {
    /// The entity's id.
    pub id: u32,
    /// The caller's tag on it, if any.
    pub tag: Option<String>,
    /// Its kind name.
    pub kind: String,
    /// How it collapsed: `"zero_length_line"`, `"zero_radius"`,
    /// `"zero_radius_arc"` or `"empty_arc"`.
    pub what: String,
    /// The length that is too small.
    pub size: f64,
}

/// A parameter the coordinates did not hold.
#[pyclass(frozen, get_all, name = "Missing", module = "laman.check")]
pub struct CheckMissing {
    /// The entity's id.
    pub id: u32,
    /// Its parameter name, or `None` when the entity is not in the sketch.
    pub coordinate: Option<String>,
}

/// Everything off at the checked coordinates.
#[pyclass(frozen, name = "Report", module = "laman.check")]
pub struct CheckReport {
    pub inner: laman_check::Report,
}

#[pymethods]
impl CheckReport {
    /// Equations that do not hold.
    #[getter]
    fn violations(&self) -> Vec<CheckViolation> {
        self.inner
            .violations
            .iter()
            .map(|v| {
                let (constraint, arc) = match v.id {
                    Source::Constraint(c) => (Some(c.0), None),
                    Source::Arc(e) => (None, Some(e.0)),
                };
                let kind = match v.kind {
                    Subject::Constraint(k) => k.to_string(),
                    Subject::ArcIntrinsic => "arc_intrinsic".to_owned(),
                };
                CheckViolation {
                    constraint,
                    arc,
                    tag: v.tag.clone(),
                    kind,
                    measured: v.measured,
                    wanted: v.wanted,
                }
            })
            .collect()
    }

    /// Degenerate entities, in id order.
    #[getter]
    fn degenerate(&self) -> Vec<CheckDegenerate> {
        self.inner
            .degenerate
            .iter()
            .map(|d| CheckDegenerate {
                id: d.id.0,
                tag: d.tag.clone(),
                kind: d.kind.to_string(),
                what: match d.what {
                    Degeneracy::ZeroLengthLine => "zero_length_line",
                    Degeneracy::ZeroRadius => "zero_radius",
                    Degeneracy::ZeroRadiusArc => "zero_radius_arc",
                    Degeneracy::EmptyArc => "empty_arc",
                }
                .to_owned(),
                size: d.size,
            })
            .collect()
    }

    /// Parameters the coordinates lacked.
    #[getter]
    fn missing(&self) -> Vec<CheckMissing> {
        self.inner
            .missing
            .iter()
            .map(|m| CheckMissing {
                id: m.id.0,
                coordinate: m.coordinate.map(|c| coordinate_name_of(c).to_owned()),
            })
            .collect()
    }

    /// True when nothing is violated, degenerate or missing.
    fn is_clean(&self) -> bool {
        self.inner.is_clean()
    }

    fn __repr__(&self) -> String {
        format!(
            "Report(violations={}, degenerate={}, missing={})",
            self.inner.violations.len(),
            self.inner.degenerate.len(),
            self.inner.missing.len()
        )
    }
}

/// Re-evaluates every constraint of `sketch` at `coordinates`, sharing no
/// code with the solver.
#[pyfunction]
pub fn check_sketch(sketch: &Sketch, coordinates: &Coordinates) -> CheckReport {
    CheckReport {
        inner: laman_check::check(&sketch.inner, &coordinates.inner),
    }
}

/// One way a reported profile disagrees with the checker's own measure.
///
/// `kind` is one of `area`, `net_area`, `orientation`, `not_chained`,
/// `open_join`, `arc_off_radius`, `not_an_edge`, `repeated`, `uncovered`,
/// `containment`. Of the other fields, only those the kind carries are set.
#[pyclass(frozen, get_all, name = "ProfileViolation", module = "laman.check")]
pub struct CheckProfileViolation {
    /// What is wrong.
    pub kind: String,
    /// The first edge of the loop it is about (the profile's outer loop for
    /// `net_area`); `None` for a loop with no edge, and for the kinds about
    /// one edge.
    pub first: Option<u32>,
    /// The edge, for `not_an_edge`, `repeated` and `uncovered`.
    pub entity: Option<u32>,
    /// The checker's measure, for `area` and `net_area`.
    pub measured: Option<f64>,
    /// The reported value, for `area` and `net_area`.
    pub reported: Option<f64>,
}

impl CheckProfileViolation {
    fn about_loop(kind: &str, first: Option<laman::model::EntityId>) -> Self {
        Self {
            kind: kind.to_owned(),
            first: first.map(|e| e.0),
            entity: None,
            measured: None,
            reported: None,
        }
    }

    fn about_edge(kind: &str, entity: laman::model::EntityId) -> Self {
        Self {
            kind: kind.to_owned(),
            first: None,
            entity: Some(entity.0),
            measured: None,
            reported: None,
        }
    }

    fn from_rust(v: &ProfileViolation) -> Self {
        match v {
            ProfileViolation::Area {
                first,
                measured,
                reported,
            } => Self {
                measured: Some(*measured),
                reported: Some(*reported),
                ..Self::about_loop("area", *first)
            },
            ProfileViolation::NetArea {
                first,
                measured,
                reported,
            } => Self {
                measured: Some(*measured),
                reported: Some(*reported),
                ..Self::about_loop("net_area", *first)
            },
            ProfileViolation::Orientation { first } => Self::about_loop("orientation", *first),
            ProfileViolation::NotChained { first } => Self::about_loop("not_chained", *first),
            ProfileViolation::OpenJoin { first } => Self::about_loop("open_join", *first),
            ProfileViolation::ArcOffRadius { first } => Self::about_loop("arc_off_radius", *first),
            ProfileViolation::NotAnEdge { entity } => Self::about_edge("not_an_edge", *entity),
            ProfileViolation::Repeated { entity } => Self::about_edge("repeated", *entity),
            ProfileViolation::Uncovered { entity } => Self::about_edge("uncovered", *entity),
            ProfileViolation::Containment { first } => Self::about_loop("containment", *first),
        }
    }
}

#[pymethods]
impl CheckProfileViolation {
    fn __repr__(&self) -> String {
        let opt = |v: Option<u32>| v.map_or("None".to_owned(), |v| v.to_string());
        let optf = |v: Option<f64>| v.map_or("None".to_owned(), |v| format!("{v:?}"));
        format!(
            "ProfileViolation(kind={:?}, first={}, entity={}, measured={}, reported={})",
            self.kind,
            opt(self.first),
            opt(self.entity),
            optf(self.measured),
            optf(self.reported)
        )
    }
}

/// Everything off in a reported `Profiles` value.
#[pyclass(frozen, name = "ProfileReport", module = "laman.check")]
pub struct CheckProfileReport {
    pub inner: laman_check::ProfileReport,
}

#[pymethods]
impl CheckProfileReport {
    /// What disagrees with the checker's own measure, in report order.
    #[getter]
    fn violations(&self) -> Vec<CheckProfileViolation> {
        self.inner
            .violations
            .iter()
            .map(CheckProfileViolation::from_rust)
            .collect()
    }

    /// Parameters the coordinates lacked, or held non-finite.
    #[getter]
    fn missing(&self) -> Vec<CheckMissing> {
        self.inner
            .missing
            .iter()
            .map(|m| CheckMissing {
                id: m.id.0,
                coordinate: m.coordinate.map(|c| coordinate_name_of(c).to_owned()),
            })
            .collect()
    }

    /// True when nothing is violated or missing.
    fn is_clean(&self) -> bool {
        self.inner.is_clean()
    }

    fn __repr__(&self) -> String {
        format!(
            "ProfileReport(violations={}, missing={})",
            self.inner.violations.len(),
            self.inner.missing.len()
        )
    }
}

/// Re-measures a reported `Profiles` value at `coordinates`, sharing no code
/// with extraction. Any value is accepted, one built by hand included.
#[pyfunction]
pub fn check_profiles(
    sketch: &Sketch,
    coordinates: &Coordinates,
    profiles: &PyProfiles,
) -> CheckProfileReport {
    CheckProfileReport {
        inner: laman_check::check_profiles(&sketch.inner, &coordinates.inner, &profiles.inner),
    }
}
