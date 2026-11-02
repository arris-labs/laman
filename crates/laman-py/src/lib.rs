//! `laman-py`: the Python binding, the package `laman` on PyPI.
//!
//! A thin 1:1 layer over the `laman` crate; nothing here computes. The
//! crate is `publish = false` on crates.io and empty on
//! `wasm32-unknown-unknown`, where pyo3 does not build.
#![forbid(unsafe_code)]
#![cfg(not(target_arch = "wasm32"))]

mod build;
mod check;
mod debug;
mod enums;
mod errors;
mod model;
mod solve;

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
    m.add_class::<model::Sketch>()?;
    m.add_class::<model::Coordinates>()?;
    m.add_class::<solve::SolverOptions>()?;
    m.add_class::<solve::Solution>()?;
    m.add_class::<model::DragSession>()?;
    m.add_class::<build::Point>()?;
    m.add_class::<build::Line>()?;
    m.add_class::<build::Circle>()?;
    m.add_class::<build::Arc>()?;
    m.add_class::<build::Removed>()?;
    m.add_class::<solve::Diagnostics>()?;
    m.add_class::<solve::EntityRef>()?;
    m.add_class::<solve::ConstraintRef>()?;
    m.add_class::<solve::EntityFreedom>()?;
    m.add_class::<solve::Motion>()?;
    m.add_class::<solve::Degenerate>()?;
    m.add_class::<solve::FlipSubject>()?;
    m.add_class::<solve::Flip>()?;
    // `laman.check` and `laman.debug` re-export these under their own names.
    m.add("CheckReport", m.py().get_type::<check::CheckReport>())?;
    m.add("CheckViolation", m.py().get_type::<check::CheckViolation>())?;
    m.add(
        "CheckDegenerate",
        m.py().get_type::<check::CheckDegenerate>(),
    )?;
    m.add("CheckMissing", m.py().get_type::<check::CheckMissing>())?;
    m.add_function(wrap_pyfunction!(check::check_sketch, m)?)?;
    m.add_function(wrap_pyfunction!(debug::digest_run, m)?)?;
    m.add_function(wrap_pyfunction!(debug::digest_solution, m)?)?;
    m.add_function(wrap_pyfunction!(debug::digest_drag_step, m)?)?;
    m.add("RigidityError", m.py().get_type::<RigidityError>())?;
    Ok(())
}
