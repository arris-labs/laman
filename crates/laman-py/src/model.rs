//! The sketch, its coordinates and the serde form.

use crate::solve::{Solution, SolverOptions};
use laman::model::{Coordinate, EntityId};
use laman_debug::fixture::coordinate_name;
use laman_debug::json::{sketch_from_str, sketch_to_string};
use pyo3::exceptions::{PyKeyError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

/// The parameter named `x`, `y` or `r`.
pub fn coordinate_from_name(name: &str) -> PyResult<Coordinate> {
    match name {
        "x" => Ok(Coordinate::X),
        "y" => Ok(Coordinate::Y),
        "r" => Ok(Coordinate::R),
        other => Err(PyValueError::new_err(format!(
            "unknown coordinate {other:?}: expected \"x\", \"y\" or \"r\""
        ))),
    }
}

/// A parameter's name: `x`, `y` or `r`.
pub fn coordinate_name_of(c: Coordinate) -> &'static str {
    coordinate_name(c)
}

/// The value of every parameter of a sketch, keyed by `(entity id,
/// "x" | "y" | "r")`, in id order.
#[pyclass(frozen, eq, module = "laman")]
#[derive(Clone, PartialEq)]
pub struct Coordinates {
    pub inner: laman::model::Coordinates,
}

fn key(id: EntityId, c: Coordinate) -> (u32, String) {
    (id.0, coordinate_name(c).to_owned())
}

#[pymethods]
impl Coordinates {
    /// Builds coordinates from a mapping (or pairs) of `(id, name)` to value.
    #[new]
    #[pyo3(signature = (items=None))]
    fn new(items: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        let mut inner = laman::model::Coordinates::new();
        if let Some(items) = items {
            let pairs = match items.downcast::<PyDict>() {
                Ok(dict) => dict.items().into_any(),
                Err(_) => items.clone(),
            };
            for pair in pairs.try_iter()? {
                let ((id, name), value) = pair?.extract::<((u32, String), f64)>()?;
                inner.insert((EntityId(id), coordinate_from_name(&name)?), value);
            }
        }
        Ok(Self { inner })
    }

    fn __len__(&self) -> usize {
        self.inner.len()
    }

    fn __getitem__(&self, key: (u32, String)) -> PyResult<f64> {
        let c = coordinate_from_name(&key.1)?;
        self.inner
            .get(&(EntityId(key.0), c))
            .copied()
            .ok_or_else(|| PyKeyError::new_err(key))
    }

    fn __contains__(&self, key: (u32, String)) -> bool {
        coordinate_from_name(&key.1).is_ok_and(|c| self.inner.contains_key(&(EntityId(key.0), c)))
    }

    fn __iter__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, pyo3::types::PyIterator>> {
        self.keys(py)?.into_any().try_iter()
    }

    /// The `(id, name)` keys, in id order.
    fn keys<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        PyList::new(py, self.inner.keys().map(|&(id, c)| key(id, c)))
    }

    /// The values, in key order.
    fn values<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        PyList::new(py, self.inner.values().copied())
    }

    /// The `((id, name), value)` pairs, in key order.
    fn items<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        PyList::new(py, self.inner.iter().map(|(&(id, c), &v)| (key(id, c), v)))
    }

    fn __repr__(&self) -> String {
        format!("Coordinates(<{} parameters>)", self.inner.len())
    }
}

/// A sketch: entities and the constraints between them.
#[pyclass(module = "laman")]
pub struct Sketch {
    pub inner: laman::model::Sketch,
}

#[pymethods]
impl Sketch {
    /// An empty sketch.
    #[new]
    fn new() -> Self {
        Self {
            inner: laman::model::Sketch::new(),
        }
    }

    /// Loads a sketch from its JSON serde form. Raises `ValueError` for text
    /// that is not a sketch file or a sketch the model refuses.
    #[staticmethod]
    fn from_json(text: &str) -> PyResult<Self> {
        let inner = sketch_from_str(text).map_err(|e| PyValueError::new_err(e.to_string()))?;
        Ok(Self { inner })
    }

    /// The sketch in its JSON serde form.
    fn to_json(&self) -> String {
        sketch_to_string(&self.inner)
    }

    /// Counts every change to the sketch.
    #[getter]
    fn revision(&self) -> u64 {
        self.inner.revision()
    }

    /// The current value of every parameter.
    fn coordinates(&self) -> Coordinates {
        Coordinates {
            inner: self.inner.coordinates(),
        }
    }

    /// Solves from the current coordinates and changes nothing.
    #[pyo3(signature = (options=None))]
    fn solve(&self, options: Option<&SolverOptions>) -> Solution {
        let options = options.map_or_else(laman::solution::SolverOptions::default, |o| o.inner);
        let solution = self.inner.solve(&options);
        Solution::new(solution)
    }
}
