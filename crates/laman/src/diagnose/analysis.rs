//! Status, redundant and conflicting sets and degeneracy at a solve's
//! coordinates (ADR-0005).
//!
//! **Redundant**: in reverse source order, a satisfied constraint whose
//! removal (contacts and scales recompiled) leaves the geometric DoF
//! unchanged is named and stays removed, so of two duplicates the later is
//! named. **Conflicting**: a deletion filter in source order over one failed
//! component's constraints. A subset is unsatisfiable when a re-solve of it
//! ends at a stationary least-squares point with residual above tolerance (a
//! certificate, below); satisfiable when a re-solve converges; anything else
//! is inconclusive, and so is the whole component. Each re-solve is the
//! default solve (staged, Levenberg–Marquardt) under the caller's caps. The
//! filter keeps a member only on a converged re-solve without it, its
//! removal witness, so the set is inclusion-minimal: no member can be
//! removed. The set's own unsatisfiability rests on the certificate, which
//! is first-order and local (ADR-0005).
//! On the staged path only a constraint with an equation in the head of its
//! component is tested; the other parts hold no redundancy (ADR-0009).
//! Intrinsic equations and ground are part of their entities: never removed,
//! never named.
use super::{Freedom, StagedRanks, dof_only, freedom_on, head_constraints};
use crate::model::tolerance::{Component, STATIONARY, Source, Tolerances};
use crate::model::{Constraint, ConstraintId, Coordinate, Coordinates, Entity, EntityId, Sketch};
use crate::solve::{Algorithm, Options, Outcome, Path, satisfied, solve};
use crate::system::System;
use std::collections::BTreeSet;

/// Re-solves one analysis may spend in all, across the conflict search of
/// every failed component (redundancy and degeneracy cost none). A filter
/// spends one per constraint of the component and one for the rest after
/// each set, two when a test's first start does not converge: the C1
/// conflicting twins spend 8 (triangle), 19 (belt) and 34 (plate) under both
/// engines. 256 holds both starts of every test of a 127-constraint component; past
/// it the analysis is inconclusive, never a conflict (ADR-0005). C2's sizes
/// measure it again.
pub(crate) const MAX_ANALYSIS_SOLVES: usize = 256;

/// The verdict on a solve (DATA-MODEL.md §Status). Redundancy, flips and
/// degeneracy ride on any status and never change it.
///
/// ```
/// use laman::solution::Status;
/// assert_eq!(Status::FullyConstrained.to_string(), "FullyConstrained");
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Status {
    /// Every required residual is within tolerance and the DoF is 0.
    FullyConstrained,
    /// Every required residual is within tolerance and the DoF is above 0.
    UnderConstrained,
    /// A conflict set exists; the coordinates are the least-squares best effort.
    Unsatisfiable,
    /// A cap was hit with residuals above tolerance and no conflict proven.
    NotConverged,
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Status::FullyConstrained => "FullyConstrained",
            Status::UnderConstrained => "UnderConstrained",
            Status::Unsatisfiable => "Unsatisfiable",
            Status::NotConverged => "NotConverged",
        })
    }
}

/// How an entity collapsed at the coordinates (the checker's four, measured
/// here independently with the shared length policy).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Collapse {
    /// A line shorter than the length tolerance.
    ZeroLengthLine,
    /// A circle of radius below the length tolerance.
    ZeroRadius,
    /// An arc whose end-points lie at its centre.
    ZeroRadiusArc,
    /// An arc whose end meets its start.
    EmptyArc,
}

/// A degenerate entity and every equation that reads it, which it makes
/// ill-conditioned: the constraints naming it and, for an arc, its own
/// intrinsic equation.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Degenerate {
    pub entity: EntityId,
    pub collapse: Collapse,
    pub size: f64,
    pub equations: Vec<Source>,
}

/// Everything `analyse` establishes.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Diagnosis {
    pub status: Status,
    pub freedom: Freedom,
    pub redundant: Vec<ConstraintId>,
    /// Inclusion-minimal conflict sets, each in source order, component by
    /// component in compiled order.
    pub conflicts: Vec<Vec<ConstraintId>>,
    pub degenerate: Vec<Degenerate>,
    /// Equations evaluated at an undefined point (`residual`'s degenerate
    /// flag) or non-finitely, whatever their entities' sizes.
    pub ill_conditioned: Vec<Source>,
    /// Re-solves spent, at most `Options::max_analysis_solves`.
    pub solves: usize,
}

/// What a re-solve of a subset of constraints established.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Verdict {
    Satisfiable,
    Unsatisfiable,
    Inconclusive,
}

/// The diagnosis of `sketch` at `outcome`'s coordinates, with re-solves
/// under `options`'s caps (`test`).
#[cfg(test)]
pub(crate) fn analyse(sketch: &Sketch, outcome: &Outcome, options: &Options) -> Diagnosis {
    analyse_kept(sketch, outcome, options, None)
}

/// [`analyse`] with the decompositions a caller kept for the sketch's
/// structure (`DragSession`).
pub(crate) fn analyse_kept(
    sketch: &Sketch,
    outcome: &Outcome,
    options: &Options,
    kept: Option<&crate::stage::Decompositions>,
) -> Diagnosis {
    let values = &outcome.coordinates;
    let mut system = System::new(sketch);
    system.decompositions = kept;
    let mut solves = 0;
    let mut conflicts = Vec::new();
    let mut failed = false;
    let mut held: BTreeSet<ConstraintId> = BTreeSet::new();
    for component in &system.components {
        let members = constraints_of(component);
        if component.sources.is_empty() || satisfied(&system, component, values) {
            held.extend(members);
            continue;
        }
        failed = true;
        // The caller's solve shares one budget across components, so a
        // component it never reached is re-solved alone before it is judged.
        if certified(&system, component, values)
            || test(sketch, values, &members, options, &mut solves) == Verdict::Unsatisfiable
        {
            conflicts.extend(conflict_sets(
                sketch,
                values,
                &members,
                options,
                &mut solves,
            ));
        }
    }
    let freedom = freedom_on(&system, values, options.path, &outcome.staged);
    let status = if !conflicts.is_empty() {
        Status::Unsatisfiable
    } else if failed {
        Status::NotConverged
    } else if freedom.dof == 0 {
        Status::FullyConstrained
    } else {
        Status::UnderConstrained
    };
    let redundant = redundant(
        &system,
        values,
        &held,
        freedom.dof,
        options.path,
        &outcome.staged,
    );
    let (degenerate, ill_conditioned) = degeneracy(sketch, &system, values);
    Diagnosis {
        status,
        freedom,
        redundant,
        conflicts,
        degenerate,
        ill_conditioned,
        solves,
    }
}

fn constraints_of(component: &Component) -> Vec<ConstraintId> {
    component
        .sources
        .iter()
        .filter_map(|s| match s {
            Source::Constraint(id) => Some(*id),
            Source::Arc(_) => None,
        })
        .collect()
}

/// The certificate of ADR-0005: an unsatisfied component whose residual `f`
/// is, to [`STATIONARY`], a left null vector of its scaled Jacobian — no
/// first-order motion reduces `|f|`. Measured on unit rows as rank is
/// (ADR-0003): with `w_i = f_i |J_i|`, `|Σ w_i J_i / |J_i|| = |Jᵀf| ≤
/// STATIONARY |w|`. A component
/// with no free parameter is certified exactly by its unsatisfied value.
/// A radius at 0 that `Jᵀf` presses below it reads the projected gradient,
/// its component dropped (ADR-0010). An invalid evaluation (degenerate,
/// non-finite, off the stored contact side) certifies nothing.
fn certified(system: &System<'_>, component: &Component, values: &Coordinates) -> bool {
    let e = system.evaluate(component, values);
    if !e.valid || satisfied(system, component, values) {
        return false;
    }
    if component.params.is_empty() {
        return true;
    }
    let n = component.params.len();
    let mut gradient = vec![0.0; n];
    let mut weight = 0.0;
    for (row, f) in e.jacobian.iter().zip(&e.values) {
        let length = row.iter().fold(0.0, |s, x| s + x * x).sqrt();
        weight += (f * length) * (f * length);
        for (g, j) in gradient.iter_mut().zip(row) {
            *g += j * f;
        }
    }
    // At a radius on its bound the first-order motion is the projected
    // gradient's (ADR-0010): a component pressing a radius below 0 cannot
    // follow it, so that component is dropped.
    for (g, p) in gradient.iter_mut().zip(&component.params) {
        let held = p.1 == Coordinate::R && values.get(p).is_some_and(|r| *r <= 0.0);
        if held && *g > 0.0 {
            *g = 0.0;
        }
    }
    let stationary = gradient.iter().fold(0.0, |s, x| s + x * x).sqrt();
    let weight = weight.sqrt();
    weight > 0.0
        && weight.is_finite()
        && stationary.is_finite()
        && stationary <= STATIONARY * weight
}

/// The inclusion-minimal sets of one failed, unsatisfiable component, found by
/// deletion filtering; after each set its greatest member is set aside and
/// the rest filtered again, until the rest solves, a test is inconclusive or
/// the budget runs out. An inconclusive filter contributes no set.
fn conflict_sets(
    sketch: &Sketch,
    values: &Coordinates,
    members: &[ConstraintId],
    options: &Options,
    solves: &mut usize,
) -> Vec<Vec<ConstraintId>> {
    let mut sets = Vec::new();
    let mut pool: Vec<ConstraintId> = members.to_vec();
    loop {
        let mut set = pool.clone();
        for c in &pool {
            let without: Vec<_> = set.iter().copied().filter(|k| k != c).collect();
            match test(sketch, values, &without, options, solves) {
                Verdict::Unsatisfiable => set = without,
                Verdict::Satisfiable => {}
                Verdict::Inconclusive => return sets,
            }
        }
        let Some(last) = set.last().copied() else {
            return sets;
        };
        sets.push(set);
        pool.retain(|k| *k != last);
        if test(sketch, values, &pool, options, solves) != Verdict::Unsatisfiable {
            return sets;
        }
    }
}

/// Re-solves `sketch` with only the constraints `keep` from every start: the
/// caller's best effort `values`, then the sketch's own coordinates. Any
/// converged start proves the subset satisfiable; it is unsatisfiable only
/// when every start was tried, none converged and one was certified. A
/// certificate is local, so a start that stops at a stationary point of a
/// satisfiable subset must not settle the verdict while another start can
/// still converge (ADR-0005, amendment). A budget spent before the last
/// start is inconclusive.
///
/// Every re-solve is the default solve, staged and by Levenberg–Marquardt,
/// whatever `options.path` and `options.algorithm` the analysis runs on; the
/// caller's caps stand. Staged (ADR-0005, second amendment): a staged solve
/// that does not converge by parts falls back to the whole solve from the
/// same start (ADR-0009), so it converges wherever the whole re-solve does
/// and ends at the whole re-solve's coordinates where neither converges. A
/// whole-path analysis then certifies nothing the staged re-solve disproves
/// (`regression/whole-false-certificate`). By Levenberg–Marquardt (third
/// amendment): an unsatisfiable subset with as many equations as it reads
/// parameters has its least-squares point where the Jacobian is singular,
/// and there the dog leg's steps, in the span of the gradient and the
/// Gauss–Newton step, crawl and end above the certificate at the caps
/// (`regression/staged-conflict-inconclusive`). Only the caller's own solve,
/// the best effort a test starts from, follows `options.algorithm`.
fn test(
    sketch: &Sketch,
    values: &Coordinates,
    keep: &[ConstraintId],
    options: &Options,
    solves: &mut usize,
) -> Verdict {
    let own = sketch.coordinates();
    let starts: &[&Coordinates] = if own == *values {
        &[values]
    } else {
        &[values, &own]
    };
    let mut copy = sketch.clone();
    let others: Vec<ConstraintId> = copy
        .constraints()
        .map(|(id, _)| id)
        .filter(|id| !keep.contains(id))
        .collect();
    for id in others {
        // Present by construction: the ids were just read from the copy.
        let _ = copy.remove_constraint(id);
    }
    let default = Options {
        path: Path::Staged,
        algorithm: Algorithm::LevenbergMarquardt,
        ..*options
    };
    let mut certificate = false;
    for start in starts {
        if *solves >= options.max_analysis_solves {
            return Verdict::Inconclusive;
        }
        *solves += 1;
        copy.place(start);
        let out = solve(&copy, default);
        if out.converged {
            return Verdict::Satisfiable;
        }
        let system = System::new(&copy);
        certificate |= system
            .components
            .iter()
            .any(|c| certified(&system, c, &out.coordinates));
    }
    if certificate {
        Verdict::Unsatisfiable
    } else {
        Verdict::Inconclusive
    }
}

/// Satisfied constraints of satisfied components, in reverse source order,
/// whose removal keeps `dof`. Each one named stays removed for the next.
fn redundant(
    system: &System<'_>,
    values: &Coordinates,
    held: &BTreeSet<ConstraintId>,
    dof: usize,
    path: Path,
    ranks: &StagedRanks,
) -> Vec<ConstraintId> {
    let mut working = system.sketch.clone();
    working.place(values);
    let mut out = Vec::new();
    // Only a head constraint can be redundant (ADR-0009): the others are
    // skipped without a factorization. The first list is read on the
    // analysis's own system, whose components `held` and `dof` were judged
    // on, from the factorizations the freedom made; naming one recompiles
    // `working`, so the list is read again there.
    #[cfg(test)]
    let first = if super::three_pass::active() {
        head_constraints(&System::new(&working), values, path, ranks)
    } else {
        head_constraints(system, values, path, ranks)
    };
    #[cfg(not(test))]
    let first = head_constraints(system, values, path, ranks);
    let mut tested = first;
    for id in held.iter().rev() {
        if !tested.contains(id) {
            continue;
        }
        let mut less = working.clone();
        if less.remove_constraint(*id).is_err() {
            continue;
        }
        if dof_only(&less, values, path) == dof {
            out.push(*id);
            working = less;
            tested = head_constraints(
                &System::new(&working),
                values,
                path,
                &StagedRanks::default(),
            );
        }
    }
    out.sort();
    out
}

fn degeneracy(
    sketch: &Sketch,
    system: &System<'_>,
    values: &Coordinates,
) -> (Vec<Degenerate>, Vec<Source>) {
    let tolerances = Tolerances::at(sketch, values);
    let xy = |id: EntityId| {
        let get = |c| values.get(&(id, c)).copied().unwrap_or(f64::NAN);
        [get(Coordinate::X), get(Coordinate::Y)]
    };
    let gap = |a: [f64; 2], b: [f64; 2]| libm::hypot(b[0] - a[0], b[1] - a[1]);
    let mut degenerate = Vec::new();
    for (id, entity) in sketch.entities() {
        let small = |size: f64| size < tolerances.entity(id) || !size.is_finite();
        let found = match *entity {
            Entity::Point { .. } => vec![],
            Entity::Line { a, b } => vec![(Collapse::ZeroLengthLine, gap(xy(a.id()), xy(b.id())))],
            Entity::Circle { .. } => vec![(
                Collapse::ZeroRadius,
                values
                    .get(&(id, Coordinate::R))
                    .copied()
                    .unwrap_or(f64::NAN),
            )],
            Entity::Arc { c, s, e } => vec![
                (Collapse::ZeroRadiusArc, gap(xy(c.id()), xy(s.id()))),
                (Collapse::EmptyArc, gap(xy(s.id()), xy(e.id()))),
            ],
        };
        for (collapse, size) in found.into_iter().filter(|(_, size)| small(*size)) {
            let mut equations: Vec<Source> = sketch
                .constraints()
                .filter(|(_, c)| Constraint::entities(c).contains(&id))
                .map(|(c, _)| Source::Constraint(c))
                .collect();
            if matches!(entity, Entity::Arc { .. }) {
                equations.insert(0, Source::Arc(id));
            }
            degenerate.push(Degenerate {
                entity: id,
                collapse,
                size,
                equations,
            });
        }
    }
    let mut ill = Vec::new();
    for eq in &system.equations {
        let original = match eq.source {
            Source::Arc(id) => crate::residual::intrinsic(sketch, values, id),
            Source::Constraint(id) => crate::residual::equations(sketch, values, id)
                .and_then(|e| e.as_slice().get(eq.index).copied()),
        };
        let bad = original.is_none_or(|o| {
            o.is_degenerate()
                || !o.value().is_finite()
                || o.terms().iter().any(|(_, g)| !g.is_finite())
        });
        if bad && ill.last() != Some(&eq.source) {
            ill.push(eq.source);
        }
    }
    (degenerate, ill)
}

#[cfg(test)]
mod tests;
