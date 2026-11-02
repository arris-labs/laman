//! The sketch, its coordinates and the serde form.

use crate::errors::model_error;
use crate::solve::{Solution, SolverOptions};
use laman::model::{Coordinate, EntityId};
use laman_debug::fixture::coordinate_name;
use laman_debug::json::{sketch_from_str, sketch_to_string};
use pyo3::exceptions::{PyKeyError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList};

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
    fn solve(&self, py: Python<'_>, options: Option<&SolverOptions>) -> PyResult<Solution> {
        let options = options.map_or_else(laman::solution::SolverOptions::default, |o| o.inner);
        Solution::new(py, self.inner.solve(&options))
    }

    /// Writes a solution's coordinates into the sketch, moving its revision.
    /// Raises a `ModelError` (`Stale`, `ForeignSolution`, ...) for a
    /// solution that was not solved from this sketch as it is now.
    fn apply(&mut self, py: Python<'_>, solution: &Solution) -> PyResult<()> {
        self.inner
            .apply(&solution.inner)
            .map_err(|e| model_error(py, &e))
    }

    /// Opens a drag session on the point `point` (an id): a copy of the
    /// sketch is dragged, this one untouched until `apply`.
    #[pyo3(signature = (point, options=None))]
    fn drag(
        &self,
        py: Python<'_>,
        point: u32,
        options: Option<&SolverOptions>,
    ) -> PyResult<DragSession> {
        let options = options.map_or_else(laman::solution::SolverOptions::default, |o| o.inner);
        let session = self
            .inner
            .drag(EntityId(point), &options)
            .map_err(|e| model_error(py, &e))?;
        Ok(DragSession {
            inner: Some(session),
        })
    }

    /// The tag on an entity, or `None`.
    fn tag_of(&self, id: u32) -> Option<String> {
        self.inner.tag_of(EntityId(id)).map(str::to_owned)
    }

    /// The payload on an entity, or `None`.
    fn payload_of<'py>(&self, py: Python<'py>, id: u32) -> Option<Bound<'py, PyBytes>> {
        self.inner
            .payload_of(EntityId(id))
            .map(|p| PyBytes::new(py, &p.0))
    }

    /// The tag on a constraint, or `None`.
    fn constraint_tag_of(&self, id: u32) -> Option<String> {
        self.inner
            .constraint_tag_of(laman::model::ConstraintId(id))
            .map(str::to_owned)
    }

    /// The payload on a constraint, or `None`.
    fn constraint_payload_of<'py>(&self, py: Python<'py>, id: u32) -> Option<Bound<'py, PyBytes>> {
        self.inner
            .constraint_payload_of(laman::model::ConstraintId(id))
            .map(|p| PyBytes::new(py, &p.0))
    }
}

/// A point being dragged through a series of targets, one frame per `to`.
///
/// `finish` ends it; any call after that raises `ValueError`, since Rust
/// moves the session and Python cannot.
#[pyclass(module = "laman")]
pub struct DragSession {
    inner: Option<laman::drag::DragSession>,
}

fn finished() -> PyErr {
    PyValueError::new_err("the drag session is finished")
}

#[pymethods]
impl DragSession {
    /// One frame: solves with the point pulled toward `(x, y)`, from the
    /// previous step. Always returns a `Solution`.
    fn to(&mut self, py: Python<'_>, x: f64, y: f64) -> PyResult<Solution> {
        let session = self.inner.as_mut().ok_or_else(finished)?;
        Solution::new(py, session.to([x, y]))
    }

    /// Ends the session and returns its last solution.
    fn finish(&mut self, py: Python<'_>) -> PyResult<Solution> {
        let session = self.inner.take().ok_or_else(finished)?;
        Solution::new(py, session.finish())
    }

    fn __repr__(&self) -> String {
        if self.inner.is_some() {
            "DragSession(open)".to_owned()
        } else {
            "DragSession(finished)".to_owned()
        }
    }
}
