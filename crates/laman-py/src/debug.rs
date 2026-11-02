//! Tooling, `laman.debug`: the digests of a run. Outside the API's stability
//! promise.

use crate::check::CheckReport;
use crate::model::Sketch;
use crate::solve::Solution;
use laman_debug::digest::{of_drag_step, of_run, of_solution};
use pyo3::prelude::*;

/// The digest (hex) of a sketch and the checker's report at its expected
/// coordinates.
#[pyfunction]
pub fn digest_run(sketch: &Sketch, report: &CheckReport) -> String {
    of_run(&sketch.inner, &report.inner).to_string()
}

/// The digest (hex) of a solve: the sketch, the solution and the report at
/// its coordinates.
#[pyfunction]
pub fn digest_solution(sketch: &Sketch, solution: &Solution, report: &CheckReport) -> String {
    of_solution(&sketch.inner, &solution.inner, &report.inner).to_string()
}

/// The digest (hex) of one drag step: its 1-based `index`, its `target`, the
/// step's solution and the report at it.
#[pyfunction]
pub fn digest_drag_step(
    sketch: &Sketch,
    index: usize,
    target: (f64, f64),
    solution: &Solution,
    report: &CheckReport,
) -> String {
    of_drag_step(
        &sketch.inner,
        index,
        [target.0, target.1],
        &solution.inner,
        &report.inner,
    )
    .to_string()
}
