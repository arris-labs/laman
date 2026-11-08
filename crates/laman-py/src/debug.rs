//! Tooling, `laman.debug`: the digests of a run and the text and SVG dumps. Outside the API's stability
//! promise.

use crate::check::{CheckProfileReport, CheckReport};
use crate::model::{Coordinates, Sketch};
use crate::profile::PyProfiles;
use crate::solve::Solution;
use laman_debug::corpus;
use laman_debug::digest::{of_drag_step, of_profiles, of_run, of_solution, of_solved_profiles};
use laman_debug::dump;
use pyo3::exceptions::PyValueError;
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

/// The digest (hex) of a profile run: the sketch, the coordinates, the
/// profiles extracted at them and the profile check's report.
#[pyfunction]
pub fn digest_profiles(
    sketch: &Sketch,
    coordinates: &Coordinates,
    profiles: &PyProfiles,
    report: &CheckProfileReport,
) -> String {
    of_profiles(
        &sketch.inner,
        &coordinates.inner,
        &profiles.inner,
        &report.inner,
    )
    .to_string()
}

/// The digest (hex) of a solved profile run: what `digest_solution` covers,
/// then the profiles extracted at the solution and the profile check's report.
#[pyfunction]
pub fn digest_solved_profiles(
    sketch: &Sketch,
    solution: &Solution,
    report: &CheckReport,
    profiles: &PyProfiles,
    profile_report: &CheckProfileReport,
) -> String {
    of_solved_profiles(
        &sketch.inner,
        &solution.inner,
        &report.inner,
        &profiles.inner,
        &profile_report.inner,
    )
    .to_string()
}

/// The corpus's grounded plate with `rows * cols` slots, and its expected
/// coordinates (the sketch's own, overlaid with the closed-form ones).
/// Raises `ValueError` for zero rows or columns or a count past the model's
/// id range.
#[pyfunction]
pub fn slot_grid(rows: usize, cols: usize) -> PyResult<(Sketch, Coordinates)> {
    let case = corpus::slot_grid(rows, cols).map_err(|e| PyValueError::new_err(e.to_string()))?;
    let mut values = case.sketch.coordinates();
    for (key, value) in &case.expected.coordinates {
        values.insert(*key, *value);
    }
    Ok((Sketch { inner: case.sketch }, Coordinates { inner: values }))
}
