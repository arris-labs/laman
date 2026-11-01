//! `laman-py`: the Python binding, the package `laman` on PyPI.
//!
//! A thin 1:1 layer over the `laman` crate; nothing here computes. The
//! crate is `publish = false` on crates.io and empty on
//! `wasm32-unknown-unknown`, where pyo3 does not build.
#![forbid(unsafe_code)]
#![cfg(not(target_arch = "wasm32"))]

use pyo3::create_exception;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

create_exception!(
    laman,
    RigidityError,
    PyValueError,
    "A bar names a point the framework does not have, or a point twice."
);

/// The generic rigidity of a bar framework (`laman::Rigidity`).
#[pyclass(frozen, module = "laman", get_all)]
pub struct Rigidity {
    /// The number of points in the framework.
    pub points: usize,
    /// Indices of the independent bars, ascending.
    pub independent: Vec<usize>,
    /// Indices of the bars redundant given the bars before them, ascending.
    pub redundant: Vec<usize>,
    /// Remaining internal degrees of freedom.
    pub dof: usize,
}

#[pymethods]
impl Rigidity {
    /// Whether the framework is generically rigid (no internal freedom).
    fn is_rigid(&self) -> bool {
        self.dof == 0
    }

    /// Whether the framework is rigid with no redundant bar.
    fn is_minimally_rigid(&self) -> bool {
        self.dof == 0 && self.redundant.is_empty()
    }

    fn __repr__(&self) -> String {
        format!(
            "Rigidity(points={}, dof={}, independent={:?}, redundant={:?})",
            self.points, self.dof, self.independent, self.redundant
        )
    }
}

/// The generic rigidity of `points` points joined by `bars` (pairs of
/// point indices). Raises `RigidityError` for a bar naming a point out of
/// range or the same point twice.
#[pyfunction]
fn rigidity(points: usize, bars: Vec<(usize, usize)>) -> PyResult<Rigidity> {
    let r = laman::rigidity(points, &bars).map_err(|e| RigidityError::new_err(e.to_string()))?;
    Ok(Rigidity {
        points: r.points,
        independent: r.independent,
        redundant: r.redundant,
        dof: r.dof,
    })
}

/// The extension module `laman._laman`, re-exported by `laman/__init__.py`.
#[pymodule]
fn _laman(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(rigidity, m)?)?;
    m.add_class::<Rigidity>()?;
    m.add("RigidityError", m.py().get_type::<RigidityError>())?;
    Ok(())
}
