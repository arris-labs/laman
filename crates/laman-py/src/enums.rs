//! The two enumerations of the API live in Python (`laman/_types.py`, plain
//! `enum.Enum`s); these helpers read and return their members by name.

use laman::solution::{Algorithm, Status};
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;

fn class<'py>(py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    py.import("laman._types")?.getattr(name)
}

/// The Python member of `Status` for a Rust status.
pub fn status_to_py(py: Python<'_>, status: Status) -> PyResult<Py<PyAny>> {
    let name = match status {
        Status::FullyConstrained => "FullyConstrained",
        Status::UnderConstrained => "UnderConstrained",
        Status::Unsatisfiable => "Unsatisfiable",
        Status::NotConverged => "NotConverged",
    };
    Ok(class(py, "Status")?.getattr(name)?.unbind())
}

/// The Python member of `Algorithm` for a Rust algorithm.
// `Algorithm` is `#[non_exhaustive]`: a variant added to `laman` reaches
// Python as an error here, not a silent default.
#[allow(clippy::wildcard_enum_match_arm)]
pub fn algorithm_to_py(py: Python<'_>, algorithm: Algorithm) -> PyResult<Py<PyAny>> {
    let name = match algorithm {
        Algorithm::LevenbergMarquardt => "LevenbergMarquardt",
        Algorithm::DogLeg => "DogLeg",
        _ => return Err(PyValueError::new_err("an algorithm this binding lacks")),
    };
    Ok(class(py, "Algorithm")?.getattr(name)?.unbind())
}

/// The Rust algorithm for a Python `Algorithm` member.
pub fn algorithm_from_py(value: &Bound<'_, PyAny>) -> PyResult<Algorithm> {
    let cls = class(value.py(), "Algorithm")?;
    if !value.is_instance(&cls)? {
        return Err(PyTypeError::new_err("algorithm must be a laman.Algorithm"));
    }
    match value.getattr("name")?.extract::<String>()?.as_str() {
        "LevenbergMarquardt" => Ok(Algorithm::LevenbergMarquardt),
        "DogLeg" => Ok(Algorithm::DogLeg),
        other => Err(PyValueError::new_err(format!("unknown algorithm {other}"))),
    }
}
