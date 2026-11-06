//! Bounded Levenberg–Marquardt and Powell's dog leg on one shared driver.
//! Public statuses and diagnostics land separately.
//!
//! Both follow Madsen, Nielsen & Tingleff, *Methods for Non-Linear Least
//! Squares Problems* (2004): identity-damped normal equations for LM (§3.2),
//! the dog leg between the Cauchy and Gauss–Newton steps for DogLeg (§3.3).
//! The driver owns component order, normalization, trials, acceptance and
//! budgets; an algorithm only proposes a step and updates its own control.
//! Defaults are measured in docs/ARCHITECTURE.md, “LM engine” and “DogLeg
//! engine”; no reference solver code is used.
use crate::model::tolerance::{Component, Frame, LENGTH_FLOOR, Source, Splitter, Tolerances};
use crate::model::{Constraint, Coordinate, Coordinates, Sketch};
use crate::system::{Evaluation, Goal, System};
use nalgebra::{DMatrix, DVector};

// Dimensionless damping and hard global budgets, measured in ARCHITECTURE.md. Damping
// regularizes the step, never geometric acceptance. No retry escapes these caps.
// Polishing target: coordinate errors accumulate across coupled equations.
// One tenth of geometric acceptance resolves the slotted-plate stopping regression.
const SOLVE_TARGET: f64 = 0.1;
const INITIAL_DAMPING: f64 = 1e-3;
const MIN_DAMPING: f64 = 1e-15;
const ACCEPT_DIVISOR: f64 = 3.0;
const REJECT_MULTIPLIER: f64 = 10.0;
const MAX_ITERATIONS: usize = 128;
const MAX_EVALUATIONS: usize = 256;

// DogLeg trust region, in normalized coordinates (one unit is the component's
// frozen scale), measured in ARCHITECTURE.md “DogLeg engine”. These are step
// controls, never acceptance thresholds.
/// First radius: the whole component may move by its own size in one step.
const INITIAL_RADIUS: f64 = 1.0;
/// Below this a normalized step is under the rounding of an order-one
/// coordinate (2.2e-16 times a few): the region has collapsed, stagnation.
const MIN_RADIUS: f64 = 1e-15;
/// Gain ratios (actual / predicted decrease) of MNT §2.4: above GOOD the
/// linear model is trusted further, below POOR the region shrinks.
const GOOD_RATIO: f64 = 0.75;
const POOR_RATIO: f64 = 0.25;
/// Growth to a multiple of the accepted step's length, and halving (MNT §3.3).
const RADIUS_GROWTH: f64 = 3.0;
const RADIUS_SHRINK: f64 = 0.5;
/// Relative singular-value cutoff of the Gauss–Newton leg: directions with
/// sigma <= GN_CUTOFF * sigma_max count as absent, so the leg neither moves
/// along free directions nor chases the inconsistent part of dependent
/// equations. A step control, not a rank decision (DoF uses ADR-0003's QR).
/// Measured in ARCHITECTURE.md “DogLeg engine”.
const GN_CUTOFF: f64 = 1e-10;
/// A goal is stationary when its projected gradient `|Nᵀ J_gᵀ g|`, the
/// distance its point could still close to first order, is within the
/// polishing target of the length acceptance plus this fraction of the
/// residual `|g|`: the rounding of the null basis and of its projection,
/// some thousands of ulps of an order-one sum, which matters only for a
/// target thousands of component sizes away. Measured, unreachable goals
/// end with `|p|` from 2e-16 to 8e-13 of the scale, against an acceptance
/// of 1e-12 (ARCHITECTURE.md “Priorities”).
const GOAL_ROUNDING: f64 = 1e-12;
/// The longest goal step, in normalized coordinates: the component's own
/// size, as DogLeg's first radius. Near a singular configuration the reduced
/// model proposes steps hundreds of sizes long, whose corrections land
/// anywhere.
const GOAL_RADIUS: f64 = 1.0;
/// Damping raises of one goal iteration before it gives up: tenfold each
/// (`REJECT_MULTIPLIER`), so 20 take the first damping 1e20 times higher,
/// where the step is steepest descent shorter than any rounding.
const GOAL_RAISES: usize = 20;
/// Accepted goal steps in a row whose objective did not fall (a rise
/// within the correction's noise, or none at all) before the pursuit stops,
/// not converged. Near stationarity such a step is how the iteration gets
/// there: measured over the test suite, at most 2 in a row before a step
/// that falls or the stationary exit. A longer run is a stall, such as a
/// step that rounds away against a far origin and leaves the objective
/// unchanged, which would otherwise be accepted until the budget is spent.
const LEVEL_STEPS: usize = 3;
/// The forward-difference step of the required curvature along a unit null
/// vector, in normalized coordinates: sqrt of machine epsilon, the step that
/// balances truncation against rounding for a first difference of an
/// analytic derivative (Nocedal & Wright, *Numerical Optimization*, §8.1).
const CURVATURE_STEP: f64 = 1.490_116_119_384_765_6e-8;
/// LAPACK's bidiagonal SVD (xBDSQR) allows 6 k^2 inner QR steps for k
/// singular values; the same hard bound, so a factorization cannot hang.
const SVD_STEPS_PER_VALUE_SQUARED: usize = 6;

/// Which step an iteration proposes (ARCHITECTURE.md §Solve).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Algorithm {
    /// Levenberg–Marquardt: identity-damped normal equations. The default.
    LevenbergMarquardt,
    /// Powell's dog leg between the steepest-descent and Gauss–Newton steps.
    DogLeg,
}

/// How a component is solved and analysed: part by part over its
/// decomposition ([`crate::stage`]), or whole. Both give the same DoF and the
/// same span of free directions; the staged path falls back to the whole one
/// wherever a part does not converge or its part ranks do not add up
/// (plans/m6-stages).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Path {
    /// Head, stages and remainder, each factorized on its own columns.
    #[default]
    Staged,
    /// The whole component at once: what the staged path is compared with
    /// (`laman::reference`). Only that feature and the tests ask for it; the
    /// staged path falls back to the same factorization without it.
    #[cfg_attr(not(any(test, feature = "reference")), allow(dead_code))]
    Whole,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Options {
    pub algorithm: Algorithm,
    pub max_iterations: usize,
    pub max_evaluations: usize,
    pub initial_damping: f64,
    pub initial_radius: f64,
    /// Re-solves the conflict analysis may spend in all (`diagnose::analysis`);
    /// exhausting it is inconclusive, never a conflict.
    pub max_analysis_solves: usize,
    /// How the diagnosis analyses each component.
    pub path: Path,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            algorithm: Algorithm::LevenbergMarquardt,
            max_iterations: MAX_ITERATIONS,
            max_evaluations: MAX_EVALUATIONS,
            initial_damping: INITIAL_DAMPING,
            initial_radius: INITIAL_RADIUS,
            max_analysis_solves: crate::diagnose::MAX_ANALYSIS_SOLVES,
            path: Path::Staged,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Outcome {
    pub coordinates: Coordinates,
    pub converged: bool,
    pub iterations: usize,
    pub evaluations: usize,
    pub rejected: usize,
    /// The caps are spent in these, in whole-component iterations and
    /// evaluations: one for a step or evaluation of a whole component, the
    /// share of its parameters for one of a part of it (`Scope::weight`). A
    /// component solved whole spends one each, so the caps mean what they
    /// did; a staged one spends what its parts' work comes to, so 400 small
    /// parts do not exhaust a cap sized for one component.
    pub iteration_work: f64,
    pub evaluation_work: f64,
    /// The staged factorizations the solve made accepting its staged
    /// components, for the diagnosis at the same coordinates to read.
    pub staged: crate::diagnose::StagedRanks,
    /// The components' decompositions the solve cut itself, when the caller
    /// kept none that applied: the diagnosis of the same sketch reads them
    /// instead of cutting again ([`crate::solution::conclude`]).
    pub cut: Option<crate::stage::Decompositions>,
}

impl Outcome {
    pub(crate) fn start(coordinates: Coordinates) -> Self {
        Self {
            coordinates,
            converged: true,
            iterations: 0,
            evaluations: 0,
            rejected: 0,
            iteration_work: 0.0,
            evaluation_work: 0.0,
            staged: crate::diagnose::StagedRanks::default(),
            cut: None,
        }
    }

    fn iterations_left(&self, options: &Options) -> bool {
        self.iteration_work < options.max_iterations as f64
    }

    fn evaluations_left(&self, options: &Options) -> bool {
        self.evaluation_work < options.max_evaluations as f64
    }

    fn spend_iteration(&mut self, weight: f64) {
        self.iterations += 1;
        self.iteration_work += weight;
    }

    fn spend_evaluation(&mut self, weight: f64) {
        self.evaluations += 1;
        self.evaluation_work += weight;
    }
}

/// An algorithm's per-component step control: LM's damping or DogLeg's radius.
#[derive(Clone, Copy, Debug)]
enum Control {
    Damping(f64),
    Radius(f64),
}

/// A proposed step and the decrease of the objective its linear model predicts.
struct Proposal {
    step: Vec<f64>,
    predicted: f64,
}

impl Control {
    fn new(options: &Options) -> Option<Self> {
        let (control, value) = match options.algorithm {
            Algorithm::LevenbergMarquardt => (
                Control::Damping(options.initial_damping),
                options.initial_damping,
            ),
            Algorithm::DogLeg => (
                Control::Radius(options.initial_radius),
                options.initial_radius,
            ),
        };
        (value.is_finite() && value > 0.0).then_some(control)
    }

    /// The step at `e` with `bound`'s active radii held (ADR-0010): their
    /// columns are dropped from the model, and the step is cut where it would
    /// carry a radius below 0. The prediction is that of the step taken.
    fn propose(&self, e: &Evaluation, bound: &Bound<'_>) -> Option<Proposal> {
        let active = bound.active(e);
        let model = bound.release(e, &active);
        let model = model.as_ref().unwrap_or(e);
        let mut step = match *self {
            Control::Damping(damping) => damped_step(model, damping)?,
            Control::Radius(radius) => dog_leg(model, radius)?,
        };
        // A column the model lacks takes no step: LM's is zero already,
        // DogLeg's SVD leaves rounding there.
        for i in &active {
            if let Some(d) = step.get_mut(*i) {
                *d = 0.0;
            }
        }
        bound.cut(&mut step);
        let predicted = predicted_decrease(e, &step);
        Some(Proposal { step, predicted })
    }

    /// After an accepted step with gain ratio `gain` and length `length`.
    fn accepted(&mut self, gain: f64, length: f64) {
        match self {
            Control::Damping(d) => *d = (*d / ACCEPT_DIVISOR).max(MIN_DAMPING),
            Control::Radius(r) => {
                if gain > GOOD_RATIO {
                    *r = r.max(RADIUS_GROWTH * length);
                } else if gain < POOR_RATIO {
                    *r *= RADIUS_SHRINK;
                }
            }
        }
    }

    /// After a rejected step; false when no further step can be meaningful.
    fn rejected(&mut self) -> bool {
        match self {
            Control::Damping(d) => {
                *d *= REJECT_MULTIPLIER;
                d.is_finite()
            }
            Control::Radius(r) => {
                *r *= RADIUS_SHRINK;
                *r >= MIN_RADIUS
            }
        }
    }
}

pub(crate) fn solve(sketch: &Sketch, options: Options) -> Outcome {
    solve_toward(sketch, &[], options)
}

/// [`solve`] with `goals` (ARCHITECTURE.md §Solve, Priorities): each
/// component's required equations are solved first, exactly as without
/// goals; then a component with goals pursues them lexicographically by
/// [`pursue`]. Without goals this is [`solve`], bit for bit.
pub(crate) fn solve_toward(sketch: &Sketch, goals: &[Goal], options: Options) -> Outcome {
    solve_kept(sketch, goals, options, None)
}

/// [`solve_toward`] with the decompositions a caller kept for the sketch's
/// structure (`DragSession`), which only saves cutting them again.
pub(crate) fn solve_kept(
    sketch: &Sketch,
    goals: &[Goal],
    options: Options,
    kept: Option<&crate::stage::Decompositions>,
) -> Outcome {
    solve_under(sketch, goals, options, kept, &Tolerances::new(sketch))
}

/// [`solve_kept`] under `frozen`, which must be [`Tolerances::new`] of
/// `sketch`: the frame's one measure of the sketch at its stored coordinates.
pub(crate) fn solve_under(
    sketch: &Sketch,
    goals: &[Goal],
    options: Options,
    kept: Option<&crate::stage::Decompositions>,
    frozen: &Tolerances,
) -> Outcome {
    let mut system = System::with_goals_under(sketch, goals, frozen);
    system.decompositions = kept;
    let mut out = Outcome::start(sketch.coordinates());
    for component in &system.components {
        let goals = system.goals_of(component);
        if component.sources.is_empty() && goals.is_empty() {
            continue;
        }
        let staged = match options.path {
            Path::Staged => solve_staged(&system, component, &goals, &options, &mut out),
            Path::Whole => None,
        };
        let converged = staged.unwrap_or_else(|| {
            let whole = Scope::whole(component);
            let mut converged = descend(&system, whole, &options, &mut out);
            if converged && !goals.is_empty() {
                converged = pursue(&system, whole, &goals, &options, &mut out);
            }
            converged
        });
        out.converged &= converged;
    }
    out.cut = system.cut.take();
    out
}

/// What a descent moves and reads, and the tolerance it meets: `part`'s
/// parameters and equations (every other parameter a constant), held to
/// `whole`'s acceptance. A component solved whole is both.
#[derive(Clone, Copy)]
struct Scope<'c> {
    part: &'c Component,
    whole: &'c Component,
    /// What the acceptance is measured by: `whole`, or for a staged part a
    /// component with `whole`'s scale and origin whose frame measured
    /// everything outside the part once ([`Scope::measured`]).
    accepts: &'c Component,
    /// Where a staged part was last solved: the component's start, from which
    /// a stage moved only by its earlier parts is continued ([`continued`]).
    solved: Option<&'c Coordinates>,
}

impl<'c> Scope<'c> {
    fn whole(component: &'c Component) -> Self {
        Self {
            part: component,
            whole: component,
            accepts: component,
            solved: None,
        }
    }

    /// The component that measures `whole`'s acceptance during a solve of
    /// `part` from `values`: the part moves no parameter outside itself, so
    /// the rest of the frame is measured once here and not at every
    /// evaluation. It holds no parameters or sources, only the scale, origin
    /// and frame the acceptance reads.
    fn measured(whole: &Component, frame: Frame) -> Component {
        Component {
            params: vec![],
            sources: vec![],
            scale: whole.scale,
            origin: whole.origin,
            frame: std::sync::Arc::new(frame),
            reads: Default::default(),
        }
    }

    /// The share of the component's parameters this scope moves: what one
    /// of its steps or evaluations spends of a cap, in whole-component ones.
    /// Exactly one for a component solved whole.
    fn weight(&self) -> f64 {
        let (part, whole) = (self.part.params.len(), self.whole.params.len());
        if part >= whole {
            1.0
        } else {
            part as f64 / whole as f64
        }
    }
}

/// The staged solve of one component (plans/m6-stages): its parts
/// ([`crate::stage`]) descended in dependency order, head, stages,
/// remainder, each with the earlier parts' parameters as constants, under
/// the shared budgets; then the goals on the last part's parameters pursued
/// there. A goal on an earlier part's parameter is stationary: the required
/// null space lies in the last part's columns. A satisfied part takes no
/// step, so an edit moves only the parts that read it.
///
/// `None` when the component is solved whole instead, with `out` restored
/// to the component's start, coordinates and budget counters both: a
/// component of one part, a part that does not converge, or a result the
/// diagnosis would not stage ([`crate::diagnose::stageable`]: a part before
/// the last column-deficient, or a row reading a later part). The whole solve
/// then spends at most the budgets the staged one had, so a component costs
/// at most twice its whole work, and a failed component keeps the whole
/// path's bits and best effort.
fn solve_staged(
    system: &System<'_>,
    component: &Component,
    goals: &[Goal],
    options: &Options,
    out: &mut Outcome,
) -> Option<bool> {
    let parts = parts_of(system, component)?;
    let last = parts.last()?;
    let start = (
        out.coordinates.clone(),
        out.iterations,
        out.evaluations,
        out.rejected,
        out.iteration_work,
        out.evaluation_work,
    );
    let mut splitter = Splitter::new(
        &component.frame,
        &parts.iter().map(|p| &p.params[..]).collect::<Vec<_>>(),
    );
    let descended = parts.iter().enumerate().all(|(k, part)| {
        part.sources.is_empty() || {
            let accepts = Scope::measured(component, splitter.split(k, &out.coordinates));
            let scope = Scope {
                part,
                whole: component,
                accepts: &accepts,
                solved: Some(&start.0),
            };
            let done = descend(system, scope, options, out);
            splitter.moved(k);
            done
        }
    });
    if !descended || !crate::diagnose::stageable(system, component, &out.coordinates, &out.staged) {
        (
            out.coordinates,
            out.iterations,
            out.evaluations,
            out.rejected,
            out.iteration_work,
            out.evaluation_work,
        ) = start;
        return None;
    }
    let goals: Vec<Goal> = goals
        .iter()
        .filter(|g| last.params.contains(&g.param))
        .copied()
        .collect();
    if goals.is_empty() {
        return Some(true);
    }
    let accepts = Scope::measured(component, splitter.split(parts.len() - 1, &out.coordinates));
    let scope = Scope {
        part: last,
        whole: component,
        accepts: &accepts,
        solved: None,
    };
    Some(pursue(system, scope, &goals, options, out))
}

/// `component`'s parts as components of their own, in dependency order:
/// each part's parameters and sources in the component's order, with the
/// component's frozen scale and origin. `None` for a component of one part,
/// which is solved whole.
pub(crate) fn parts_of(system: &System<'_>, component: &Component) -> Option<Vec<Component>> {
    let decomposition = crate::stage::decompose(system, component);
    if decomposition.parts.len() < 2 {
        return None;
    }
    Some(
        decomposition
            .parts
            .iter()
            .map(|part| {
                let mut part = Component {
                    params: part.params.clone(),
                    sources: part.sources.clone(),
                    scale: component.scale,
                    origin: component.origin,
                    frame: component.frame.clone(),
                    reads: Default::default(),
                };
                part.reads = crate::model::tolerance::component_reads(system.sketch, &part);
                part
            })
            .collect(),
    )
}

/// [`parts_of`] by filtering the component's parameters and sources for each
/// part: the reference the one-pass version is held equal to.
#[cfg(test)]
fn parts_of_filtered(system: &System<'_>, component: &Component) -> Option<Vec<Component>> {
    use std::collections::BTreeSet;
    let decomposition = crate::stage::decompose(system, component);
    if decomposition.parts.len() < 2 {
        return None;
    }
    let parts = decomposition
        .parts
        .iter()
        .map(|part| {
            let params: BTreeSet<_> = part.params.iter().copied().collect();
            let sources: BTreeSet<_> = part
                .rows
                .iter()
                .filter_map(|row| system.equations.get(*row).map(|e| e.source))
                .collect();
            let mut part = Component {
                params: component
                    .params
                    .iter()
                    .filter(|p| params.contains(p))
                    .copied()
                    .collect(),
                sources: component
                    .sources
                    .iter()
                    .filter(|s| sources.contains(s))
                    .copied()
                    .collect(),
                scale: component.scale,
                origin: component.origin,
                frame: component.frame.clone(),
                reads: Default::default(),
            };
            part.reads = crate::model::tolerance::component_reads(system.sketch, &part);
            part
        })
        .collect();
    Some(parts)
}

/// The required descent of one scope from `out.coordinates`, under the
/// shared budgets: true when its required equations meet acceptance. A
/// satisfied start takes no step and keeps every bit, signed zeros included.
fn descend(system: &System<'_>, scope: Scope<'_>, options: &Options, out: &mut Outcome) -> bool {
    let component = scope.part;
    let weight = scope.weight();
    if !out.evaluations_left(options) {
        return false;
    }
    out.spend_evaluation(weight);
    let mut current = system.evaluate_part(component, scope.accepts, &out.coordinates, 1.0);
    current.satisfied &= vector_acceptance(system, scope, &out.coordinates, 1.0);
    if current.satisfied {
        return true;
    }
    if let Some(next) = scope
        .solved
        .and_then(|solved| continued(system, scope, solved, options, out))
    {
        if !out.evaluations_left(options) {
            return false;
        }
        out.spend_evaluation(weight);
        let mut moved = system.evaluate_part(component, scope.accepts, &next, SOLVE_TARGET);
        moved.satisfied &= vector_acceptance(system, scope, &next, SOLVE_TARGET);
        if moved.valid && objective(&moved).is_finite() {
            out.coordinates = next;
            current = moved;
            if current.satisfied {
                return true;
            }
        }
    }
    let Some(mut position) = normalized(component, &out.coordinates) else {
        return false;
    };
    if !current.valid || component.params.is_empty() {
        return false;
    }
    let mut cost = objective(&current);
    let Some(mut control) = Control::new(options).filter(|_| cost.is_finite()) else {
        return false;
    };
    let radii = radii_of(component);
    // A part spends a share of a cap, so it is also held to the cap alone:
    // no descent takes more steps than a whole component may.
    let mut steps = 0;
    while steps < options.max_iterations
        && out.iterations_left(options)
        && out.evaluations_left(options)
    {
        steps += 1;
        out.spend_iteration(weight);
        let bound = Bound {
            radii: &radii,
            position: &position,
        };
        let proposal = control.propose(&current, &bound);
        let candidate = proposal
            .as_ref()
            .and_then(|p| trial_values(component, &out.coordinates, &position, &p.step));
        // An attempted trial, including a non-finite one, consumes one
        // evaluation allowance. Linear factorization failures consume both.
        out.spend_evaluation(weight);
        if let (Some((moved, next_position)), Some(p)) = (candidate, proposal.as_ref()) {
            // The trial is evaluated in place and put back when refused: the
            // component's parameters are the only entries it changes, so no
            // copy of the whole sketch's coordinates is made.
            let before: Vec<Option<f64>> = component
                .params
                .iter()
                .map(|key| out.coordinates.get(key).copied())
                .collect();
            for (key, value) in component.params.iter().zip(&moved) {
                out.coordinates.insert(*key, *value);
            }
            let mut next =
                system.evaluate_part(component, scope.accepts, &out.coordinates, SOLVE_TARGET);
            next.satisfied &= vector_acceptance(system, scope, &out.coordinates, SOLVE_TARGET);
            let next_cost = objective(&next);
            if next.valid && next_cost.is_finite() && (next_cost < cost || next.satisfied) {
                // A non-positive prediction (rounding) is a poor model.
                let gain = if p.predicted > 0.0 {
                    (cost - next_cost) / p.predicted
                } else {
                    0.0
                };
                control.accepted(gain, norm(&p.step));
                position = next_position;
                current = next;
                cost = next_cost;
                if current.satisfied {
                    break;
                }
                continue;
            }
            for (key, old) in component.params.iter().zip(before) {
                match old {
                    Some(v) => {
                        out.coordinates.insert(*key, v);
                    }
                    None => {
                        out.coordinates.remove(key);
                    }
                }
            }
        }
        out.rejected += 1;
        if !control.rejected() {
            break;
        }
        // A zero step above tolerance is stagnation, never convergence.
        if proposal.is_some_and(|p| p.step.iter().all(|x| *x == 0.0)) {
            break;
        }
    }
    current.satisfied
}

/// The first-order continuation of a part that held at `solved` and is moved
/// only by its earlier parts: with `x` its parameters and `u` the earlier
/// ones, the Gauss–Newton step of its equations linearized at `solved` with
/// `u` at its new value, `δ = −J_x⁺ (F + J_u Δu)` (the implicit function
/// theorem's tangent; Allgower & Georg, *Introduction to Numerical
/// Continuation Methods*, 1990, the Euler predictor). A geometric equation
/// does not change under a translation of all it reads, so a translated
/// parent carries the part exactly, and a rotation by `θ` errs by `θ²` times the part's reach: the
/// part stays on the branch it held, where a descent from where it stood,
/// left behind by a parent that moved further than the part's own size,
/// can cross to the mirror root (an edit rotating a long tail,
/// `triangle_strip_250`). `None` when the part did not hold at `solved`
/// (a fresh solve or an edited part, descended as it stands), when no
/// earlier parameter it reads moved, or when the step is not finite. One
/// evaluation of the shared budget.
fn continued(
    system: &System<'_>,
    scope: Scope<'_>,
    solved: &Coordinates,
    options: &Options,
    out: &mut Outcome,
) -> Option<Coordinates> {
    let part = scope.part;
    if !out.evaluations_left(options) {
        return None;
    }
    out.spend_evaluation(scope.weight());
    // At `solved`, the component's start: `scope.accepts` was split at the
    // coordinates the earlier parts have moved to, so it would measure the
    // part's own entities at `solved` and everything outside them elsewhere.
    // The whole frame measured at `solved` is one set, once per part.
    let at_start = Scope {
        accepts: scope.whole,
        ..scope
    };
    let (rows, satisfied, _) = system.rows(part, at_start.accepts, solved, 1.0);
    if !satisfied || !vector_acceptance(system, at_start, solved, 1.0) {
        return None;
    }
    let scale = part.scale.max(LENGTH_FLOOR);
    let mut shifted = false;
    let mut values = Vec::with_capacity(rows.len());
    let mut jacobian = Vec::with_capacity(rows.len());
    for row in &rows {
        let mut value = row.value;
        for (p, term) in &row.terms {
            if part.params.contains(p) {
                continue;
            }
            let (Some(now), Some(then)) = (out.coordinates.get(p), solved.get(p)) else {
                continue;
            };
            if now.to_bits() != then.to_bits() {
                value += term * row.factor * ((now - then) / scale);
                shifted = true;
            }
        }
        values.push(value);
        jacobian.push(
            part.params
                .iter()
                .map(|p| row.terms.get(p).copied().unwrap_or(0.0) * row.factor)
                .collect(),
        );
    }
    if !shifted {
        return None;
    }
    let linear = Evaluation {
        values,
        jacobian,
        satisfied: false,
        valid: true,
    };
    let step = gauss_newton(&linear, GN_CUTOFF)?;
    let position = normalized(part, &out.coordinates)?;
    trial(part, &out.coordinates, &position, &step).map(|(coordinates, _)| coordinates)
}

/// The goal phase of one component whose required equations hold at
/// `out.coordinates`: a lexicographic least-squares descent on the goals,
/// Levenberg–Marquardt on the reduced problem (sequential quadratic
/// programming on the required equations; Nocedal & Wright, *Numerical
/// Optimization*, 2006, §18.1).
///
/// Each iteration takes the null basis `N` of the required Jacobian
/// (ADR-0003's QR, [`crate::diagnose::null_space`]), the goal's projected
/// gradient `p = Nᵀ J_gᵀ g` and its reduced Hessian `H` ([`Pursuit::hessian`]),
/// and the step `N z`, `(H + μ I) z = −p`: of least norm, since `N` is
/// orthonormal and `z` has no part outside it. `μ` is LM's dimensionless
/// damping times `max(1, max diag H)`, from `INITIAL_DAMPING`, divided by
/// `ACCEPT_DIVISOR` on an accepted step and multiplied by
/// `REJECT_MULTIPLIER` on a rejected one or a failed factorization, at most
/// [`GOAL_RAISES`] times an iteration. A component without required
/// equations takes the minimum-norm Gauss–Newton step: its goal model is
/// exact. The step is cut to
/// [`GOAL_RADIUS`], corrected back onto the required equations by
/// [`descend`] and accepted when that converges and the goal's objective
/// does not rise beyond the correction's noise ([`Pursuit::decreased`]),
/// at most [`LEVEL_STEPS`] times in a row without falling. Every probe
/// costs one iteration and one evaluation of the shared budgets, its
/// correction its own.
///
/// Converged when every goal is met within the length acceptance (its
/// polishing target after a step, as in [`descend`]), or the goal is
/// stationary in the null space: `|p|` within that acceptance plus
/// [`GOAL_ROUNDING`] `· |g|` (an unreachable target, such as fixture 4's y). A goal that cannot be
/// decreased, one that stalls for more than [`LEVEL_STEPS`] steps, or a
/// spent budget, is not converged; the coordinates are then
/// the last iterate whose required equations held, so a goal never leaves a
/// required residual over acceptance.
fn pursue(
    system: &System<'_>,
    scope: Scope<'_>,
    goals: &[Goal],
    options: &Options,
    out: &mut Outcome,
) -> bool {
    let component = scope.part;
    let index: Option<Vec<usize>> = goals
        .iter()
        .map(|g| component.params.iter().position(|p| *p == g.param))
        .collect();
    let Some(index) = index else {
        return false;
    };
    let pursuit = Pursuit {
        system,
        scope,
        component,
        goals,
        index,
        options,
        scale: component.scale.max(LENGTH_FLOOR),
    };
    let exact = component.sources.is_empty();
    let mut damping = options.initial_damping;
    if !(damping.is_finite() && damping > 0.0) {
        return false;
    }
    // As in `descend`: a goal met at the start takes no step; one an
    // iteration reached must meet the polishing target.
    let mut target = 1.0;
    let mut level = 0;
    loop {
        let Some(g) = pursuit.residuals(&out.coordinates) else {
            return false;
        };
        let acceptance =
            target * system.acceptance(scope.accepts, &out.coordinates) / pursuit.scale;
        target = SOLVE_TARGET;
        if g.iter().all(|r| r.abs() <= acceptance) {
            return true;
        }
        let (_, basis) = crate::diagnose::null_space(system, component, &out.coordinates);
        // J_g N: a goal row is the unit row of its parameter in normalized
        // coordinates, so its product with N is that parameter's entries.
        let reduced: Vec<Vec<f64>> = pursuit
            .index
            .iter()
            .map(|i| basis.iter().map(|v| v[*i]).collect())
            .collect();
        let projected: Vec<f64> = (0..basis.len())
            .map(|k| {
                reduced
                    .iter()
                    .zip(&g)
                    .fold(0.0, |sum, (row, r)| sum + row[k] * r)
            })
            .collect();
        if basis.is_empty() || norm(&projected) <= acceptance + GOAL_ROUNDING * norm(&g) {
            return true;
        }
        let Some(position) = normalized(component, &out.coordinates) else {
            return false;
        };
        let hessian = pursuit.hessian(out, &position, &basis, &reduced, &g);
        let magnitude = (0..basis.len())
            .map(|i| hessian[(i, i)].abs())
            .fold(1.0, f64::max);
        let rhs = DVector::<f64>::from_iterator(basis.len(), projected.iter().map(|x| -x));
        let start = out.coordinates.clone();
        let mut accepted = false;
        for _ in 0..=GOAL_RAISES {
            let z = if exact {
                // Without required equations the goal is linear: its
                // minimum-norm Gauss–Newton step is exact.
                let model = Evaluation {
                    values: g.clone(),
                    jacobian: reduced.clone(),
                    satisfied: false,
                    valid: true,
                };
                gauss_newton(&model, GN_CUTOFF).map(DVector::from_vec)
            } else {
                let mut damped = hessian.clone();
                for i in 0..basis.len() {
                    damped[(i, i)] += damping * magnitude;
                }
                damped
                    .cholesky()
                    .map(|f| f.solve(&rhs))
                    .filter(|z| z.iter().all(|x| x.is_finite()))
            };
            if let Some(z) = z {
                let mut step: Vec<f64> = (0..component.params.len())
                    .map(|c| {
                        basis
                            .iter()
                            .zip(z.iter())
                            .fold(0.0, |sum, (v, zk)| sum + zk * v[c])
                    })
                    .collect();
                let reach = norm(&step);
                if !exact && reach > GOAL_RADIUS {
                    for x in &mut step {
                        *x *= GOAL_RADIUS / reach;
                    }
                }
                let Some(probe) = pursuit.probe(out, &start, &position, &step) else {
                    break;
                };
                let verdict = probe.and_then(|c| pursuit.decreased(&start, &g, &c).map(|f| (c, f)));
                if let Some((coordinates, fell)) = verdict {
                    level = if fell { 0 } else { level + 1 };
                    if level > LEVEL_STEPS {
                        // A stall: the last iterate stays, its required
                        // equations held.
                        return false;
                    }
                    out.coordinates = coordinates;
                    damping = (damping / ACCEPT_DIVISOR).max(MIN_DAMPING);
                    accepted = true;
                    break;
                }
                out.rejected += 1;
            }
            if exact {
                // The exact step does not change with the damping.
                break;
            }
            damping *= REJECT_MULTIPLIER;
            if !damping.is_finite() {
                break;
            }
        }
        if !accepted {
            out.coordinates = start;
            return false;
        }
    }
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).fold(0.0, |sum, (x, y)| sum + x * y)
}

/// What [`pursue`] holds fixed while it iterates.
struct Pursuit<'s, 'a> {
    system: &'s System<'a>,
    scope: Scope<'s>,
    /// `scope.part`: what the goals move in.
    component: &'s Component,
    goals: &'s [Goal],
    /// Each goal's parameter's index in `component.params`.
    index: Vec<usize>,
    options: &'s Options,
    scale: f64,
}

impl Pursuit<'_, '_> {
    /// The goals' residuals at `values` in normalized units; None when one
    /// is unreadable or non-finite.
    fn residuals(&self, values: &Coordinates) -> Option<Vec<f64>> {
        self.goals
            .iter()
            .map(|g| {
                let r = (values.get(&g.param)? - g.target) / self.scale;
                r.is_finite().then_some(r)
            })
            .collect()
    }

    /// The reduced Hessian of the goal objective (halved) in the null basis
    /// `N`: `H = (J_g N)ᵀ (J_g N) + Nᵀ W N`, `W = Σ λ_i ∇²c_i` the required
    /// equations' curvature weighted by their least-squares multipliers
    /// `λ = −(J_rᵀ)⁺ J_gᵀ g`. `W N_k` is a forward difference of the analytic
    /// required Jacobian along each null vector, one evaluation each, at
    /// [`CURVATURE_STEP`]. That curvature is what the Gauss–Newton model in
    /// the null space misses: a linkage reaching for a target beyond it bends
    /// and straightens in turn without it, and a point on a circle reaching
    /// for a target inside undershoots by the ratio of their radii. When the
    /// curvature cannot be read (no required equation, an unreadable
    /// evaluation, a spent budget), `H` is the Gauss–Newton part alone.
    fn hessian(
        &self,
        out: &mut Outcome,
        position: &[f64],
        basis: &[Vec<f64>],
        reduced: &[Vec<f64>],
        g: &[f64],
    ) -> DMatrix<f64> {
        let k = basis.len();
        let mut hessian = DMatrix::<f64>::from_fn(k, k, |a, b| {
            reduced.iter().fold(0.0, |sum, row| sum + row[a] * row[b])
        });
        if let Some(curvature) = self.curvature(out, position, basis, g) {
            for a in 0..k {
                for b in 0..k {
                    // Symmetrized: the forward difference is not exactly so.
                    hessian[(a, b)] +=
                        0.5 * (dot(&basis[a], &curvature[b]) + dot(&basis[b], &curvature[a]));
                }
            }
        }
        hessian
    }

    /// `W N_k` for each null vector, or None (see [`Pursuit::hessian`]).
    fn curvature(
        &self,
        out: &mut Outcome,
        position: &[f64],
        basis: &[Vec<f64>],
        g: &[f64],
    ) -> Option<Vec<Vec<f64>>> {
        if self.component.sources.is_empty() {
            return None;
        }
        let n = self.component.params.len();
        let here = &out.coordinates;
        let jr = self.system.evaluate(self.component, here);
        if !jr.valid {
            return None;
        }
        // J_gᵀ g over the parameters: each goal's residual at its parameter.
        let mut pulled = vec![0.0; n];
        for (i, r) in self.index.iter().zip(g) {
            pulled[*i] += r;
        }
        let m = jr.jacobian.len();
        let transposed = Evaluation {
            values: pulled,
            jacobian: (0..n)
                .map(|c| jr.jacobian.iter().map(|row| row[c]).collect())
                .collect(),
            satisfied: false,
            valid: true,
        };
        let lambda = gauss_newton(&transposed, GN_CUTOFF)?;
        let mut curvature = Vec::with_capacity(basis.len());
        for v in basis {
            if !out.evaluations_left(self.options) {
                return None;
            }
            out.spend_evaluation(self.scope.weight());
            let probe: Vec<f64> = v.iter().map(|x| x * CURVATURE_STEP).collect();
            let (moved, _) = trial(self.component, &out.coordinates, position, &probe)?;
            let jm = self.system.evaluate(self.component, &moved);
            if !jm.valid || jm.jacobian.len() != m {
                return None;
            }
            // (J_r(x + h v) − J_r(x))ᵀ λ / h, summed in equation order.
            let w: Vec<f64> = (0..n)
                .map(|c| {
                    (0..m).fold(0.0, |sum, i| {
                        sum + (jm.jacobian[i][c] - jr.jacobian[i][c]) * lambda[i]
                    }) / CURVATURE_STEP
                })
                .collect();
            if !w.iter().all(|x| x.is_finite()) {
                return None;
            }
            curvature.push(w);
        }
        Some(curvature)
    }

    /// The coordinates after the trial `start + step` and its correction:
    /// `None` when the budgets are spent, `Some(None)` when the trial or its
    /// correction failed. `out.coordinates` is left at `start`.
    #[allow(clippy::option_option)] // three outcomes, each read by the caller
    fn probe(
        &self,
        out: &mut Outcome,
        start: &Coordinates,
        position: &[f64],
        step: &[f64],
    ) -> Option<Option<Coordinates>> {
        if !out.iterations_left(self.options) || !out.evaluations_left(self.options) {
            return None;
        }
        out.spend_iteration(self.scope.weight());
        out.spend_evaluation(self.scope.weight());
        let mut reached = None;
        if let Some((coordinates, _)) = trial(self.component, start, position, step) {
            out.coordinates = coordinates;
            if descend(self.system, self.scope, self.options, out) {
                reached = Some(out.coordinates.clone());
            }
        }
        out.coordinates = start.clone();
        Some(reached)
    }

    /// Whether the goal objective at `next` is not above its value at
    /// `start`, whose residuals are `g`, by more than the required equations'
    /// acceptance can move it: `Σ (r'_i − r_i)(r'_i + r_i) < a Σ |r'_i + r_i|`,
    /// `a` the length acceptance in normalized units. `None` refuses the
    /// step; `Some(fell)` accepts it and says whether the objective fell
    /// (the sum below zero) or only stayed within that noise, which
    /// [`pursue`] admits at most [`LEVEL_STEPS`] times in a row. Each difference is
    /// taken from the coordinates themselves: the product form keeps a
    /// decrease of a goal's reachable part visible beside a large
    /// unreachable part, where a difference of two sums of squares rounds it
    /// away. The margin is the correction's own noise: a corrected point sits
    /// anywhere within acceptance of the required equations, and near a
    /// stationary point the true decrease is smaller than that; refusing such
    /// a step would stall the reduced Newton iteration short of stationarity.
    fn decreased(&self, start: &Coordinates, g: &[f64], next: &Coordinates) -> Option<bool> {
        let acceptance = self.system.acceptance(self.scope.accepts, next) / self.scale;
        let change = self
            .goals
            .iter()
            .zip(g)
            .try_fold((0.0, 0.0), |(sum, margin), (goal, r)| {
                let before = start.get(&goal.param)?;
                let after = next.get(&goal.param)?;
                let delta = (after - before) / self.scale;
                let r_next = (after - goal.target) / self.scale;
                Some((sum + delta * (r_next + r), margin + (r_next + r).abs()))
            });
        change
            .filter(|(c, margin)| *c < acceptance * margin)
            .map(|(c, _)| c < 0.0)
    }
}

/// Whether every equation of `component` holds at `values` with geometric
/// acceptance: the test that lets a satisfied input take no step, and the one
/// the analysis reads.
pub(crate) fn satisfied(system: &System<'_>, component: &Component, values: &Coordinates) -> bool {
    let (rows, flagged, valid) = system.rows(component, component, values, 1.0);
    satisfied_over(system, component, values, &rows, flagged && valid)
}

/// [`satisfied`] over rows already read by `system.rows(component,
/// component, values, 1.0)`, `holds` being the conjunction of the flags it
/// returned with them (plans/m8-dense-rank step 2): one row pass for a
/// caller that also needs the rows.
pub(crate) fn satisfied_over(
    system: &System<'_>,
    component: &Component,
    values: &Coordinates,
    rows: &[crate::system::Row],
    holds: bool,
) -> bool {
    sparse_satisfied(component, rows, holds)
        && vector_acceptance(system, Scope::whole(component), values, 1.0)
}

/// `system.evaluate(component, values).satisfied` read from the sparse rows,
/// without the dense Jacobian (plans/m8-frame-floor step 3): the rows'
/// flags (`holds`), and every row's value and every entry finite. A dense
/// entry is a term in the component's columns times the row's factor, and
/// the entries it does not read are +0, so only those terms need testing.
fn sparse_satisfied(component: &Component, rows: &[crate::system::Row], holds: bool) -> bool {
    let columns: std::collections::BTreeSet<_> = component.params.iter().collect();
    holds
        && rows.iter().all(|row| {
            row.value.is_finite()
                && row
                    .terms
                    .iter()
                    .all(|(p, term)| !columns.contains(p) || (term * row.factor).is_finite())
        })
}

/// The dense reference of [`sparse_satisfied`].
#[cfg(test)]
pub(crate) fn dense_satisfied(
    system: &System<'_>,
    component: &Component,
    values: &Coordinates,
) -> bool {
    system.evaluate(component, values).satisfied
}

// A vector condition has one geometric length tolerance. In symmetry, the
// reflected-point error is hypot(2 * midpoint-to-axis, along-axis separation).
// This complements scalar equation acceptance without importing the checker.
fn vector_acceptance(
    system: &System<'_>,
    scope: Scope<'_>,
    values: &Coordinates,
    target: f64,
) -> bool {
    let sketch = system.sketch;
    // Measured only for a two-equation constraint: the measure reads the
    // whole component, and most parts have none.
    let mut acceptance = None;
    scope.part.sources.iter().all(|source| {
        let Source::Constraint(id) = source else {
            return true;
        };
        let Some(equations) = crate::residual::equations(sketch, values, *id) else {
            return false;
        };
        let [a, b] = equations.as_slice() else {
            return true;
        };
        let weight = if matches!(sketch.constraint(*id), Some(Constraint::Symmetric { .. })) {
            2.0
        } else {
            1.0
        };
        let acceptance =
            *acceptance.get_or_insert_with(|| system.acceptance(scope.accepts, values));
        libm::hypot(weight * a.value(), b.value()) <= target * acceptance
    })
}

fn objective(e: &Evaluation) -> f64 {
    e.values.iter().fold(0.0, |sum, v| sum + v * v)
}

fn normalized(component: &Component, values: &Coordinates) -> Option<Vec<f64>> {
    let scale = component.scale.max(LENGTH_FLOOR);
    component
        .params
        .iter()
        .map(|p| {
            let origin = match p.1 {
                Coordinate::X => component.origin[0],
                Coordinate::Y => component.origin[1],
                Coordinate::R => 0.0,
            };
            let value = (values.get(p)? - origin) / scale;
            value.is_finite().then_some(value)
        })
        .collect()
}

/// The indices of `component`'s radius parameters: the only ones with a
/// bound (`r ≥ 0`, ADR-0010). In normalized coordinates a radius has origin
/// 0, so the bound is `x ≥ 0` there too.
fn radii_of(component: &Component) -> Vec<usize> {
    component
        .params
        .iter()
        .enumerate()
        .filter(|(_, p)| p.1 == Coordinate::R)
        .map(|(i, _)| i)
        .collect()
}

/// The bound `r ≥ 0` at one iterate (ADR-0010): a projected step with an
/// active set (Bertsekas, *Nonlinear Programming*, 2nd ed., 1999, §2.3, the
/// two-metric projection). A radius is **active** when it sits on its bound
/// and the objective's gradient `Jᵀf` presses it below; its column leaves the
/// step's model, so the other parameters take the step that model gives
/// without it, and every radius the step would carry below 0 is cut at 0.
struct Bound<'a> {
    radii: &'a [usize],
    position: &'a [f64],
}

impl Bound<'_> {
    /// The active radii at `e`: on the bound, pressed below it.
    fn active(&self, e: &Evaluation) -> Vec<usize> {
        self.radii
            .iter()
            .copied()
            .filter(|i| self.position.get(*i).is_some_and(|x| *x <= 0.0) && pressed(e, *i) > 0.0)
            .collect()
    }

    /// `e` with the `active` radii's columns zeroed, or `None` when none is
    /// active, so an unbounded iterate is proposed from `e` itself, bit for bit.
    fn release(&self, e: &Evaluation, active: &[usize]) -> Option<Evaluation> {
        if active.is_empty() {
            return None;
        }
        let jacobian = e
            .jacobian
            .iter()
            .map(|row| {
                let mut row = row.clone();
                for i in active {
                    if let Some(x) = row.get_mut(*i) {
                        *x = 0.0;
                    }
                }
                row
            })
            .collect();
        Some(Evaluation {
            values: e.values.clone(),
            jacobian,
            satisfied: e.satisfied,
            valid: e.valid,
        })
    }

    /// Cuts `step` where it would carry a radius below 0: `x + δ ≥ 0`, and
    /// `x + (−x)` is exactly 0.
    fn cut(&self, step: &mut [f64]) {
        for i in self.radii {
            if let (Some(x), Some(d)) = (self.position.get(*i), step.get_mut(*i)) {
                if *x + *d < 0.0 {
                    *d = -*x;
                }
            }
        }
    }
}

/// The objective's gradient `(Jᵀf)_i` along parameter `i`, summed in row
/// order: positive when descent would lower it.
fn pressed(e: &Evaluation, i: usize) -> f64 {
    e.jacobian.iter().zip(&e.values).fold(0.0, |sum, (row, f)| {
        sum + row.get(i).copied().unwrap_or(0.0) * f
    })
}

fn trial(
    component: &Component,
    values: &Coordinates,
    position: &[f64],
    step: &[f64],
) -> Option<(Coordinates, Vec<f64>)> {
    let (moved, next) = trial_values(component, values, position, step)?;
    let mut coordinates = values.clone();
    for (key, value) in component.params.iter().zip(moved) {
        coordinates.insert(*key, value);
    }
    Some((coordinates, next))
}

/// [`trial`] without the copy of the coordinates: the component's parameters'
/// new values in `component.params` order, and the normalized position.
fn trial_values(
    component: &Component,
    values: &Coordinates,
    position: &[f64],
    step: &[f64],
) -> Option<(Vec<f64>, Vec<f64>)> {
    let scale = component.scale.max(LENGTH_FLOOR);
    let mut moved = Vec::with_capacity(position.len());
    let mut next = Vec::with_capacity(position.len());
    for ((key, x), delta) in component.params.iter().zip(position).zip(step) {
        let origin = match key.1 {
            Coordinate::X => component.origin[0],
            Coordinate::Y => component.origin[1],
            Coordinate::R => 0.0,
        };
        // A radius is held at its bound, never carried below it (ADR-0010):
        // a NaN is kept, for the finite check below to refuse.
        let x = match key.1 {
            Coordinate::R if x + delta < 0.0 => 0.0,
            Coordinate::X | Coordinate::Y | Coordinate::R => x + delta,
        };
        // Do not round-trip an unchanged parameter through normalization.
        let value = if *delta == 0.0 {
            *values.get(key)?
        } else {
            origin + x * scale
        };
        if !x.is_finite() || !value.is_finite() {
            return None;
        }
        moved.push(value);
        next.push(x);
    }
    Some((moved, next))
}

// (J^T J + lambda I) d = -J^T f is the unique damped least-squares
// minimizer. In exact arithmetic it lies in the row space of J, so it adds
// no null-space motion. One physical scale per component preserves that metric.
// Summation order is explicit; Cholesky has no pivot ties or iterative loop.
fn damped_step(e: &Evaluation, damping: f64) -> Option<Vec<f64>> {
    let n = e.jacobian.first()?.len();
    let mut normal = DMatrix::<f64>::zeros(n, n);
    let mut rhs = DVector::<f64>::zeros(n);
    for (row, value) in e.jacobian.iter().zip(&e.values) {
        for i in 0..n {
            rhs[i] -= row[i] * value;
            for j in 0..=i {
                normal[(i, j)] += row[i] * row[j];
            }
        }
    }
    let magnitude = (0..n).map(|i| normal[(i, i)]).fold(1.0, f64::max);
    for i in 0..n {
        normal[(i, i)] += damping * magnitude;
        for j in 0..i {
            normal[(j, i)] = normal[(i, j)];
        }
    }
    if !normal.iter().chain(rhs.iter()).all(|x| x.is_finite()) {
        return None;
    }
    let step = normal.cholesky()?.solve(&rhs);
    step.iter()
        .all(|x| x.is_finite())
        .then(|| step.iter().copied().collect())
}

/// Sum of squares in a fixed order, then the correctly rounded `sqrt`.
fn norm(v: &[f64]) -> f64 {
    v.iter().fold(0.0, |sum, x| sum + x * x).sqrt()
}

/// `|f|^2 - |f + J h|^2`: the decrease of the objective the linearization predicts.
fn predicted_decrease(e: &Evaluation, step: &[f64]) -> f64 {
    let mut after = 0.0;
    for (row, value) in e.jacobian.iter().zip(&e.values) {
        let linear = row.iter().zip(step).fold(*value, |sum, (j, h)| sum + j * h);
        after += linear * linear;
    }
    objective(e) - after
}

// Minimum-norm least-squares Gauss–Newton step, h = -J^+ f, through a
// truncated SVD J = U S V^T: h = -sum_i v_i (u_i . f) / s_i over s_i above the
// cutoff. Normal equations fail one way or the other on C1 systems: J^T J
// leaks rounding into free directions (a rank-1 distance moved a free pair by
// 1e-5), J J^T loses the step to cancellation on dependent equations (two fixes
// of one point stalled 1.9e-7 short of their midpoint). The sums here run in
// index order by hand: nalgebra's dense products dispatch to `matrixmultiply`,
// whose FMA kernels are chosen by CPU at run time.
fn gauss_newton(e: &Evaluation, cutoff: f64) -> Option<Vec<f64>> {
    let m = e.jacobian.len();
    let n = e.jacobian.first()?.len();
    if !e
        .jacobian
        .iter()
        .flatten()
        .chain(&e.values)
        .all(|x| x.is_finite())
    {
        return None;
    }
    let k = m.min(n);
    let j = DMatrix::<f64>::from_fn(m, n, |r, c| e.jacobian[r][c]);
    // Machine epsilon is the convergence test of the off-diagonal, not a tolerance.
    let svd = j.try_svd_unordered(
        true,
        true,
        f64::EPSILON,
        SVD_STEPS_PER_VALUE_SQUARED * k * k,
    )?;
    let (u, v_t) = (svd.u?, svd.v_t?);
    let top = svd.singular_values.iter().copied().fold(0.0, f64::max);
    let mut step = vec![0.0; n];
    for (i, sigma) in svd.singular_values.iter().enumerate() {
        // A NaN sigma passes and is refused by the finite check below.
        if *sigma <= cutoff * top {
            continue;
        }
        let along = (0..m).fold(0.0, |sum, r| sum + u[(r, i)] * e.values[r]) / sigma;
        for (c, h) in step.iter_mut().enumerate() {
            *h -= along * v_t[(i, c)];
        }
    }
    step.iter().all(|x| x.is_finite()).then_some(step)
}

// Powell's dog leg (MNT §3.3) inside `radius`: the Gauss–Newton step when it
// fits, else the steepest-descent (Cauchy) step cut to the radius, else the
// point where the leg from the Cauchy point to Gauss–Newton leaves the region.
// Every candidate is a combination of J^T f and a regularized GN step, so it
// lies in the row space of J: no null-space motion, as with LM.
fn dog_leg(e: &Evaluation, radius: f64) -> Option<Vec<f64>> {
    let n = e.jacobian.first()?.len();
    let mut g = vec![0.0; n];
    for (row, value) in e.jacobian.iter().zip(&e.values) {
        for (gi, j) in g.iter_mut().zip(row) {
            *gi += j * value;
        }
    }
    let g_norm = norm(&g);
    if g_norm == 0.0 {
        // A stationary point above tolerance: a zero step, which the driver
        // reports as stagnation.
        return Some(vec![0.0; n]);
    }
    let jg: Vec<f64> = e
        .jacobian
        .iter()
        .map(|row| row.iter().zip(&g).fold(0.0, |sum, (j, x)| sum + j * x))
        .collect();
    let alpha = (g_norm * g_norm) / jg.iter().fold(0.0, |sum, x| sum + x * x);
    let gauss_newton = gauss_newton(e, GN_CUTOFF)?;
    let step = if norm(&gauss_newton) <= radius {
        gauss_newton
    } else if !alpha.is_finite() || alpha * g_norm >= radius {
        g.iter().map(|x| -radius / g_norm * x).collect()
    } else {
        // a = Cauchy point, d = leg to GN; beta in (0, 1) solves |a + beta d| = radius,
        // choosing the root formula that avoids cancellation (as MNT §3.3 does).
        let a: Vec<f64> = g.iter().map(|x| -alpha * x).collect();
        let d: Vec<f64> = gauss_newton.iter().zip(&a).map(|(b, a)| b - a).collect();
        let c = a.iter().zip(&d).fold(0.0, |sum, (x, y)| sum + x * y);
        let dd = d.iter().fold(0.0, |sum, x| sum + x * x);
        let gap = radius * radius - a.iter().fold(0.0, |sum, x| sum + x * x);
        let root = (c * c + dd * gap).sqrt();
        let beta = if c <= 0.0 {
            (root - c) / dd
        } else {
            gap / (c + root)
        };
        a.iter().zip(&d).map(|(a, d)| a + beta * d).collect()
    };
    step.iter().all(|x| x.is_finite()).then_some(step)
}

#[cfg(test)]
mod tests;
