//! Staged against whole: what a solve reports by stages (`Sketch::solve`)
//! against the whole-component reference (plans/m6-stages). The staged
//! solve lands on other coordinates than the whole one, within acceptance,
//! so its diagnostics are compared with the whole diagnosis at the staged
//! coordinates (`laman::reference::diagnose`): status, DoF and per-entity
//! DoF equal exactly, and so do the redundant and conflict sets; the free
//! directions span the same space, measured by their orthogonal projectors.
//! The conflict sets equal the whole solve's too (`laman::reference::solve`),
//! where the whole path's sets are unsatisfiable: see `assert_same_conflicts`.
#![allow(clippy::unwrap_used, dead_code)]

use laman::model::{Coordinate, Coordinates, EntityId, Sketch};
use laman::solution::{Solution, SolverOptions, Status};
use std::collections::BTreeMap;

/// How far the staged and whole projectors onto the free directions may
/// differ, entry by entry: the bases differ (part-wise against whole QRs)
/// and so does their rounding, of order one ulp per sum over the sizes the
/// tests reach; a direction one path has and the other lacks differs by
/// order one.
pub const PROJECTOR: f64 = 1e-9;

/// A parameter: an entity's coordinate.
type Param = (EntityId, Coordinate);

/// The orthogonal projector `Σ v vᵀ` of a solution's free directions.
fn projector(solution: &Solution) -> BTreeMap<(Param, Param), f64> {
    let mut out = BTreeMap::new();
    for v in &solution.diagnostics.free_directions {
        for a in v {
            for b in v {
                *out.entry(((a.entity, a.coordinate), (b.entity, b.coordinate)))
                    .or_insert(0.0) += a.amount * b.amount;
            }
        }
    }
    out
}

/// The largest entry of `staged`'s projector minus `whole`'s.
fn projector_gap(staged: &Solution, whole: &Solution) -> f64 {
    let (p, q) = (projector(staged), projector(whole));
    p.keys()
        .chain(q.keys())
        .map(|k| (p.get(k).copied().unwrap_or(0.0) - q.get(k).copied().unwrap_or(0.0)).abs())
        .fold(0.0, f64::max)
}

/// Whether `solution`'s conflict analysis was inconclusive: `NotConverged`
/// with no conflict set named.
fn inconclusive(solution: &Solution) -> bool {
    solution.status == Status::NotConverged && solution.diagnostics.conflicts.is_empty()
}

/// Holds the staged conflict sets equal to the whole path's. Both
/// analyses re-solve every filter test by the default staged solve
/// (ADR-0005, second amendment), so a whole-path set that holds together
/// (`regression/whole-false-certificate`) is no longer excused. An analysis
/// that is `inconclusive` on either side gives no answer to compare; the
/// staged one is a finding (`docs/BACKLOG.md`
/// `staged-conflict-inconclusive`). Nothing staged is excused by it: the
/// staged sets keep their own guarantees, which the callers' properties
/// hold (each set names the contradiction, a member violated).
fn assert_same_conflicts(name: &str, what: &str, staged: &Solution, whole: &Solution) {
    let (s, w) = (&staged.diagnostics.conflicts, &whole.diagnostics.conflicts);
    // One analysis inconclusive names nothing
    // (`framework-conflict-inconclusive`'s family; the staged DogLeg side
    // is the finding `staged-conflict-inconclusive`): there is no pair of
    // answers to compare, and the property's own checks still hold the
    // inconclusive side to never calling the sketch satisfied.
    let one_inconclusive = inconclusive(whole) || inconclusive(staged);
    assert!(
        s == w || one_inconclusive,
        "{name}: {what}: staged {s:?}, whole {w:?}"
    );
}

/// Solves `sketch` staged, diagnoses its coordinates whole and holds the
/// two to equality, and the conflict sets to the whole solve's
/// (`assert_same_conflicts`); returns the
/// staged solution. A failure names `name`.
pub fn assert_same_freedom(name: &str, sketch: &Sketch, options: &SolverOptions) -> Solution {
    let staged = sketch.solve(options);
    let solved = laman::reference::solve(sketch, options);
    assert_same_conflicts(name, "conflicts of the whole solve", &staged, &solved);
    let whole = laman::reference::diagnose(sketch, &staged.coordinates, options);
    // An inconclusive analysis on one side reports `NotConverged` where the
    // other proves a set (`assert_same_conflicts`).
    let one_inconclusive = (inconclusive(&staged) && whole.status == Status::Unsatisfiable)
        || (inconclusive(&whole) && staged.status == Status::Unsatisfiable);
    if !one_inconclusive {
        assert_eq!(staged.status, whole.status, "{name}: status");
    }
    assert_eq!(staged.diagnostics.dof, whole.diagnostics.dof, "{name}: DoF");
    let per_entity = |s: &Solution| -> Vec<(EntityId, usize)> {
        s.diagnostics
            .entities
            .iter()
            .map(|e| (e.entity.id, e.dof))
            .collect()
    };
    assert_eq!(
        per_entity(&staged),
        per_entity(&whole),
        "{name}: per-entity DoF"
    );
    assert_eq!(
        staged.diagnostics.redundant, whole.diagnostics.redundant,
        "{name}: redundant"
    );
    assert_same_conflicts(name, "conflicts", &staged, &whole);
    assert_eq!(
        staged.diagnostics.free_directions.len(),
        whole.diagnostics.free_directions.len(),
        "{name}: directions"
    );
    let gap = projector_gap(&staged, &whole);
    assert!(gap <= PROJECTOR, "{name}: projectors differ by {gap}");
    staged
}

/// `sketch` moved to `values` (the sketch's own value where `values` names
/// none) through the public API: a solution of it, its coordinates
/// replaced, applied. Values a caller cannot apply (a fixed parameter
/// moved) panic, as a test's setup failing.
pub fn placed(sketch: &Sketch, values: &Coordinates) -> Sketch {
    let mut out = sketch.clone();
    let mut solution = out.solve(&SolverOptions::default());
    solution.coordinates = out.coordinates();
    for (key, value) in values {
        solution.coordinates.insert(*key, *value);
    }
    out.apply(&solution).unwrap();
    out
}
