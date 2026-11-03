//! The whole-component path, for laman's own tests to compare the staged one
//! with (plans/m6-stages). Behind the non-default `reference` feature,
//! hidden from the docs, outside the stability promise and the binding: it
//! may change or go in any release.
//!
//! ```
//! use laman::model::Sketch;
//! use laman::solution::SolverOptions;
//! let mut sk = Sketch::new();
//! let o = sk.point_fixed(0.0, 0.0)?;
//! let p = sk.point(3.0, 1.0)?;
//! sk.distance(o, p, 5.0)?;
//! let options = SolverOptions::default();
//! let staged = sk.solve(&options);
//! let whole = laman::reference::solve(&sk, &options);
//! assert_eq!(whole.diagnostics.dof, staged.diagnostics.dof);
//! let there = laman::reference::diagnose(&sk, &staged.coordinates, &options);
//! assert_eq!(there.coordinates, staged.coordinates);
//! # Ok::<(), laman::model::ModelError>(())
//! ```

use crate::model::{Coordinates, Sketch};
use crate::solution::{Solution, SolverOptions};
use crate::solve::Path;

/// [`Sketch::solve`] with every component solved and analysed whole, never
/// by parts: the same guarantees and refusals.
pub fn solve(sketch: &Sketch, options: &SolverOptions) -> Solution {
    crate::solution::solve_on(sketch, options, Path::Whole)
}

/// The whole-component diagnosis of `sketch` at `coordinates` (a solve's,
/// every parameter named), as a solve that ended there would report it,
/// with no step taken: the coordinates come back unchanged. The conflict
/// analysis of an unsatisfied component still re-solves it, whole.
pub fn diagnose(sketch: &Sketch, coordinates: &Coordinates, options: &SolverOptions) -> Solution {
    crate::solution::diagnose_on(sketch, coordinates, options, Path::Whole)
}
