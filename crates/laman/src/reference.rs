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

use crate::model::tolerance::Source;
use crate::model::{ConstraintId, Coordinate, Coordinates, EntityId, Sketch};
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
/// analysis of an unsatisfied component re-solves its tests by the default
/// staged solve, as every analysis does (ADR-0005, second amendment).
pub fn diagnose(sketch: &Sketch, coordinates: &Coordinates, options: &SolverOptions) -> Solution {
    crate::solution::diagnose_on(sketch, coordinates, options, Path::Whole)
}

/// The parameters a change of `constraint` can move in a staged solve of
/// `sketch` (plans/m6-stages): those of the part holding its equations and
/// of every later part that reads one of them, transitively, in its
/// component's decomposition, which depends on the sketch's structure
/// alone. A component of one part is all its parameters; a constraint that
/// reads no free parameter, or is not in the sketch, moves none. In the
/// component's parameter order.
///
/// ```
/// use laman::model::{Coordinate, Sketch};
/// let mut sk = Sketch::new();
/// let o = sk.point_fixed(0.0, 0.0)?;
/// let q = sk.point(9.0, 2.0)?;
/// let width = sk.distance(o, q, 9.0)?;
/// let level = sk.horizontal((o, q))?;
/// // q.y is the horizontal's stage, which the width reads and never moves.
/// let moved = laman::reference::downstream(&sk, width);
/// assert_eq!(moved, vec![(q.id(), Coordinate::X)]);
/// let both = laman::reference::downstream(&sk, level);
/// assert_eq!(both, vec![(q.id(), Coordinate::X), (q.id(), Coordinate::Y)]);
/// # Ok::<(), laman::model::ModelError>(())
/// ```
pub fn downstream(sketch: &Sketch, constraint: ConstraintId) -> Vec<(EntityId, Coordinate)> {
    use std::collections::BTreeSet;
    let system = crate::system::System::new(sketch);
    let source = Source::Constraint(constraint);
    let Some(component) = system
        .components
        .iter()
        .find(|c| c.sources.contains(&source))
    else {
        return vec![];
    };
    let decomposition = crate::stage::decompose(&system, component);
    let own = sketch.coordinates();
    let mut reached: BTreeSet<(EntityId, Coordinate)> = BTreeSet::new();
    let mut started = false;
    for part in &decomposition.parts {
        let reads: BTreeSet<_> = part
            .rows
            .iter()
            .filter_map(|row| system.equations.get(*row))
            .flat_map(|eq| crate::stage::support_at(&system, eq, &own))
            .collect();
        let holds = part
            .rows
            .iter()
            .filter_map(|row| system.equations.get(*row))
            .any(|eq| eq.source == source);
        if holds || (started && reads.iter().any(|p| reached.contains(p))) {
            started = true;
            reached.extend(part.params.iter().copied());
        }
    }
    if decomposition.parts.len() < 2 {
        reached.extend(component.params.iter().copied());
    }
    component
        .params
        .iter()
        .filter(|p| reached.contains(p))
        .copied()
        .collect()
}
