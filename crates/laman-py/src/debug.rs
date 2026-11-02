//! Tooling, `laman.debug`: the digests of a run and the text and SVG dumps. Outside the API's stability
//! promise.

use crate::check::CheckReport;
use crate::model::{Coordinates, Sketch};
use crate::solve::Solution;
use laman_debug::digest::{of_drag_step, of_run, of_solution};
use laman_debug::dump;
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

/// The sketch at `coordinates` as text: every entity and constraint with its
/// id, tag and flags, then the checker's findings in `report`.
#[pyfunction]
pub fn text(sketch: &Sketch, coordinates: &Coordinates, report: &CheckReport) -> String {
    dump::text(&sketch.inner, &coordinates.inner, &report.inner)
}

/// The sketch at `coordinates` as an SVG document, entities and constraints
/// the checker reports in red.
#[pyfunction]
pub fn svg(sketch: &Sketch, coordinates: &Coordinates, report: &CheckReport) -> String {
    dump::svg(&sketch.inner, &coordinates.inner, &report.inner)
}

/// The sketch at the solution's coordinates as text, then the solution's
/// status, degrees of freedom and diagnostics.
#[pyfunction]
pub fn solution_text(sketch: &Sketch, solution: &Solution) -> String {
    dump::solution_text(&sketch.inner, &solution.inner)
}
