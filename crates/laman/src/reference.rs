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
//! let whole = laman::reference::solve(&sk, &options);
//! assert_eq!(whole.diagnostics.dof, sk.solve(&options).diagnostics.dof);
//! # Ok::<(), laman::model::ModelError>(())
//! ```

use crate::model::Sketch;
use crate::solution::{Solution, SolverOptions};
use crate::solve::Path;

/// [`Sketch::solve`] with every component analysed whole, never by parts:
/// the same guarantees and refusals.
pub fn solve(sketch: &Sketch, options: &SolverOptions) -> Solution {
    crate::solution::solve_on(sketch, options, Path::Whole)
}
