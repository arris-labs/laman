//! `laman::model::ModelError` as Python exceptions (`laman/_errors.py`).

use laman::model::ModelError;
use pyo3::prelude::*;
use pyo3::types::PyDict;

/// The exception class for a variant, built with the variant's fields as
/// attributes. A variant this binding lacks (`ModelError` is
/// `#[non_exhaustive]`) reaches Python as the base `ModelError` carrying the
/// message.
#[allow(clippy::wildcard_enum_match_arm)]
pub fn model_error(py: Python<'_>, e: &ModelError) -> PyErr {
    build(py, e).unwrap_or_else(|err| err)
}

#[allow(clippy::wildcard_enum_match_arm)]
fn build(py: Python<'_>, e: &ModelError) -> PyResult<PyErr> {
    let fields = PyDict::new(py);
    let name = match e {
        ModelError::UnknownId { id } => {
            fields.set_item("id", id.0)?;
            "UnknownId"
        }
        ModelError::WrongKind {
            id,
            expected,
            found,
        } => {
            fields.set_item("id", id.0)?;
            fields.set_item("expected", expected.to_string())?;
            fields.set_item("found", found.to_string())?;
            "WrongKind"
        }
        ModelError::FixedPoint { id } => {
            fields.set_item("id", id.0)?;
            "FixedPoint"
        }
        ModelError::NonFinite { what, value } => {
            fields.set_item("what", *what)?;
            fields.set_item("value", *value)?;
            "NonFinite"
        }
        ModelError::DimensionOutOfRange {
            what,
            value,
            wanted,
        } => {
            fields.set_item("what", *what)?;
            fields.set_item("value", *value)?;
            fields.set_item("wanted", *wanted)?;
            "DimensionOutOfRange"
        }
        ModelError::DuplicateId { id } => {
            fields.set_item("id", id.0)?;
            "DuplicateId"
        }
        ModelError::IdsExhausted => "IdsExhausted",
        ModelError::RepeatedEntity { id, kind } => {
            fields.set_item("id", id.0)?;
            fields.set_item("kind", kind.to_string())?;
            "RepeatedEntity"
        }
        ModelError::UnknownConstraint { id } => {
            fields.set_item("id", id.0)?;
            "UnknownConstraint"
        }
        ModelError::DuplicateConstraintId { id } => {
            fields.set_item("id", id.0)?;
            "DuplicateConstraintId"
        }
        ModelError::NoSuchParameter { id, coordinate } => {
            fields.set_item("id", id.0)?;
            fields.set_item("coordinate", crate::model::coordinate_name_of(*coordinate))?;
            "NoSuchParameter"
        }
        ModelError::FormatVersion { found, supported } => {
            fields.set_item("found", *found)?;
            fields.set_item("supported", *supported)?;
            "FormatVersion"
        }
        ModelError::WrongFormat { found } => {
            fields.set_item("found", found)?;
            "WrongFormat"
        }
        ModelError::InvalidPayload => "InvalidPayload",
        ModelError::Entity { id, source } => {
            fields.set_item("id", id.0)?;
            fields.set_item("source", model_error(py, source).into_value(py))?;
            "EntityError"
        }
        ModelError::Constraint { id, source } => {
            fields.set_item("id", id.0)?;
            fields.set_item("source", model_error(py, source).into_value(py))?;
            "ConstraintError"
        }
        ModelError::Stale { solved, current } => {
            fields.set_item("solved", *solved)?;
            fields.set_item("current", *current)?;
            "Stale"
        }
        ModelError::ForeignSolution => "ForeignSolution",
        ModelError::IncompleteSolution { id, coordinate } => {
            fields.set_item("id", id.0)?;
            fields.set_item("coordinate", crate::model::coordinate_name_of(*coordinate))?;
            "IncompleteSolution"
        }
        ModelError::FixedMoved {
            id,
            coordinate,
            fixed,
            given,
        } => {
            fields.set_item("id", id.0)?;
            fields.set_item("coordinate", crate::model::coordinate_name_of(*coordinate))?;
            fields.set_item("fixed", *fixed)?;
            fields.set_item("given", *given)?;
            "FixedMoved"
        }
        ModelError::InUse {
            id,
            entities,
            constraints,
        } => {
            fields.set_item("id", id.0)?;
            fields.set_item("entities", entities.iter().map(|e| e.0).collect::<Vec<_>>())?;
            fields.set_item(
                "constraints",
                constraints.iter().map(|c| c.0).collect::<Vec<_>>(),
            )?;
            "InUse"
        }
        _ => "ModelError",
    };
    let class = py.import("laman._errors")?.getattr(name)?;
    let instance = class.call((e.to_string(),), Some(&fields))?;
    Ok(PyErr::from_value(instance))
}
