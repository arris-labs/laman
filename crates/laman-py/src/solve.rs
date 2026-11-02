//! Solver options and the solution of a solve.

use crate::enums::{algorithm_from_py, algorithm_to_py, status_to_py};
use crate::model::Coordinates;
use pyo3::prelude::*;

/// How a solve runs: the algorithm and its hard caps. Defaults are Rust's.
#[pyclass(frozen, eq, module = "laman")]
#[derive(Clone, PartialEq)]
pub struct SolverOptions {
    pub inner: laman::solution::SolverOptions,
}

#[pymethods]
impl SolverOptions {
    #[new]
    #[pyo3(signature = (*, algorithm=None, max_iterations=None, max_evaluations=None, max_analysis_solves=None, max_bisections=None))]
    fn new(
        algorithm: Option<&Bound<'_, PyAny>>,
        max_iterations: Option<usize>,
        max_evaluations: Option<usize>,
        max_analysis_solves: Option<usize>,
        max_bisections: Option<usize>,
    ) -> PyResult<Self> {
        let mut inner = laman::solution::SolverOptions::default();
        if let Some(algorithm) = algorithm {
            inner.algorithm = algorithm_from_py(algorithm)?;
        }
        if let Some(n) = max_iterations {
            inner.max_iterations = n;
        }
        if let Some(n) = max_evaluations {
            inner.max_evaluations = n;
        }
        if let Some(n) = max_analysis_solves {
            inner.max_analysis_solves = n;
        }
        if let Some(n) = max_bisections {
            inner.max_bisections = n;
        }
        Ok(Self { inner })
    }

    /// The step each iteration proposes.
    #[getter]
    fn algorithm(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        algorithm_to_py(py, self.inner.algorithm)
    }

    /// Accepted and rejected steps in all, across components.
    #[getter]
    fn max_iterations(&self) -> usize {
        self.inner.max_iterations
    }

    /// Residual and Jacobian evaluations in all, across components.
    #[getter]
    fn max_evaluations(&self) -> usize {
        self.inner.max_evaluations
    }

    /// Re-solves the conflict analysis may spend in all.
    #[getter]
    fn max_analysis_solves(&self) -> usize {
        self.inner.max_analysis_solves
    }

    /// Halvings of a drag step that flips a branch.
    #[getter]
    fn max_bisections(&self) -> usize {
        self.inner.max_bisections
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let algorithm = algorithm_to_py(py, self.inner.algorithm)?;
        Ok(format!(
            "SolverOptions(algorithm={}, max_iterations={}, max_evaluations={}, max_analysis_solves={}, max_bisections={})",
            algorithm.bind(py).str()?,
            self.inner.max_iterations,
            self.inner.max_evaluations,
            self.inner.max_analysis_solves,
            self.inner.max_bisections
        ))
    }
}

/// What a solve established about the sketch.
#[pyclass(frozen, module = "laman")]
pub struct Diagnostics {
    dof: usize,
}

#[pymethods]
impl Diagnostics {
    /// The geometric degrees of freedom left.
    #[getter]
    fn dof(&self) -> usize {
        self.dof
    }

    fn __repr__(&self) -> String {
        format!("Diagnostics(dof={})", self.dof)
    }
}

/// The result of a solve: coordinates, a status and the diagnostics.
#[pyclass(frozen, module = "laman")]
pub struct Solution {
    pub inner: laman::solution::Solution,
}

impl Solution {
    pub fn new(inner: laman::solution::Solution) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl Solution {
    /// Every parameter of the sketch, fixed ones at their values.
    #[getter]
    fn coordinates(&self) -> Coordinates {
        Coordinates {
            inner: self.inner.coordinates.clone(),
        }
    }

    /// The verdict.
    #[getter]
    fn status(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        status_to_py(py, self.inner.status)
    }

    /// What the solve established.
    #[getter]
    fn diagnostics(&self) -> Diagnostics {
        Diagnostics {
            dof: self.inner.diagnostics.dof,
        }
    }

    /// The revision of the sketch this was solved from.
    #[getter]
    fn source_revision(&self) -> u64 {
        self.inner.source_revision()
    }

    fn __repr__(&self) -> String {
        format!(
            "Solution(status={}, dof={})",
            self.inner.status, self.inner.diagnostics.dof
        )
    }
}
