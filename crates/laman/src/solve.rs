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
use crate::model::tolerance::{Component, LENGTH_FLOOR, Source};
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

    fn propose(&self, e: &Evaluation) -> Option<Proposal> {
        let step = match *self {
            Control::Damping(damping) => damped_step(e, damping)?,
            Control::Radius(radius) => dog_leg(e, radius)?,
        };
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
    let system = System::with_goals(sketch, goals);
    let mut out = Outcome {
        coordinates: sketch.coordinates(),
        converged: true,
        iterations: 0,
        evaluations: 0,
        rejected: 0,
    };
    for component in &system.components {
        let goals = system.goals_of(component);
        if component.sources.is_empty() && goals.is_empty() {
            continue;
        }
        let mut converged = descend(&system, component, &options, &mut out);
        if converged && !goals.is_empty() {
            converged = pursue(&system, component, &goals, &options, &mut out);
        }
        out.converged &= converged;
    }
    out
}

/// The required descent of one component from `out.coordinates`, under the
/// shared budgets: true when its required equations meet acceptance. A
/// satisfied start takes no step and keeps every bit, signed zeros included.
fn descend(
    system: &System<'_>,
    component: &Component,
    options: &Options,
    out: &mut Outcome,
) -> bool {
    if out.evaluations == options.max_evaluations {
        return false;
    }
    out.evaluations += 1;
    let mut current = system.evaluate(component, &out.coordinates);
    current.satisfied &= vector_acceptance(system, component, &out.coordinates, 1.0);
    if current.satisfied {
        return true;
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
    while out.iterations < options.max_iterations && out.evaluations < options.max_evaluations {
        out.iterations += 1;
        let proposal = control.propose(&current);
        let candidate = proposal
            .as_ref()
            .and_then(|p| trial(component, &out.coordinates, &position, &p.step));
        // An attempted trial, including a non-finite one, consumes one
        // evaluation allowance. Linear factorization failures consume both.
        out.evaluations += 1;
        if let (Some((coordinates, next_position)), Some(p)) = (candidate, proposal.as_ref()) {
            let mut next = system.evaluate_to(component, &coordinates, SOLVE_TARGET);
            next.satisfied &= vector_acceptance(system, component, &coordinates, SOLVE_TARGET);
            let next_cost = objective(&next);
            if next.valid && next_cost.is_finite() && (next_cost < cost || next.satisfied) {
                // A non-positive prediction (rounding) is a poor model.
                let gain = if p.predicted > 0.0 {
                    (cost - next_cost) / p.predicted
                } else {
                    0.0
                };
                control.accepted(gain, norm(&p.step));
                out.coordinates = coordinates;
                position = next_position;
                current = next;
                cost = next_cost;
                if current.satisfied {
                    break;
                }
                continue;
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
/// falls ([`Pursuit::decreased`]). Every probe costs one iteration and one evaluation of the shared
/// budgets, its correction its own.
///
/// Converged when every goal is met within the length acceptance (its
/// polishing target after a step, as in [`descend`]), or the goal is
/// stationary in the null space: `|p|` within that acceptance plus
/// [`GOAL_ROUNDING`] `· |g|` (an unreachable target, such as fixture 4's y). A goal that cannot be
/// decreased, or a spent budget, is not converged; the coordinates are then
/// the last iterate whose required equations held, so a goal never leaves a
/// required residual over acceptance.
fn pursue(
    system: &System<'_>,
    component: &Component,
    goals: &[Goal],
    options: &Options,
    out: &mut Outcome,
) -> bool {
    let index: Option<Vec<usize>> = goals
        .iter()
        .map(|g| component.params.iter().position(|p| *p == g.param))
        .collect();
    let Some(index) = index else {
        return false;
    };
    let pursuit = Pursuit {
        system,
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
    loop {
        let Some(g) = pursuit.residuals(&out.coordinates) else {
            return false;
        };
        let acceptance = target * system.acceptance(component, &out.coordinates) / pursuit.scale;
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
                if let Some(coordinates) = probe.filter(|c| pursuit.decreased(&start, &g, c)) {
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
            if out.evaluations >= self.options.max_evaluations {
                return None;
            }
            out.evaluations += 1;
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
        if out.iterations >= self.options.max_iterations
            || out.evaluations >= self.options.max_evaluations
        {
            return None;
        }
        out.iterations += 1;
        out.evaluations += 1;
        let mut reached = None;
        if let Some((coordinates, _)) = trial(self.component, start, position, step) {
            out.coordinates = coordinates;
            if descend(self.system, self.component, self.options, out) {
                reached = Some(out.coordinates.clone());
            }
        }
        out.coordinates = start.clone();
        Some(reached)
    }

    /// Whether the goal objective at `next` is below its value at `start`,
    /// whose residuals are `g`, beyond what the required equations'
    /// acceptance can move it: `Σ (r'_i − r_i)(r'_i + r_i) < a Σ |r'_i + r_i|`,
    /// `a` the length acceptance in normalized units. Each difference is
    /// taken from the coordinates themselves: the product form keeps a
    /// decrease of a goal's reachable part visible beside a large
    /// unreachable part, where a difference of two sums of squares rounds it
    /// away. The margin is the correction's own noise: a corrected point sits
    /// anywhere within acceptance of the required equations, and near a
    /// stationary point the true decrease is smaller than that; refusing such
    /// a step would stall the reduced Newton iteration short of stationarity.
    fn decreased(&self, start: &Coordinates, g: &[f64], next: &Coordinates) -> bool {
        let acceptance = self.system.acceptance(self.component, next) / self.scale;
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
        change.is_some_and(|(c, margin)| c < acceptance * margin)
    }
}

/// Whether every equation of `component` holds at `values` with geometric
/// acceptance: the test that lets a satisfied input take no step, and the one
/// the analysis reads.
pub(crate) fn satisfied(system: &System<'_>, component: &Component, values: &Coordinates) -> bool {
    system.evaluate(component, values).satisfied
        && vector_acceptance(system, component, values, 1.0)
}

// A vector condition has one geometric length tolerance. In symmetry, the
// reflected-point error is hypot(2 * midpoint-to-axis, along-axis separation).
// This complements scalar equation acceptance without importing the checker.
fn vector_acceptance(
    system: &System<'_>,
    component: &Component,
    values: &Coordinates,
    target: f64,
) -> bool {
    let sketch = system.sketch;
    let acceptance = system.acceptance(component, values);
    component.sources.iter().all(|source| {
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

fn trial(
    component: &Component,
    values: &Coordinates,
    position: &[f64],
    step: &[f64],
) -> Option<(Coordinates, Vec<f64>)> {
    let scale = component.scale.max(LENGTH_FLOOR);
    let mut coordinates = values.clone();
    let mut next = Vec::with_capacity(position.len());
    for ((key, x), delta) in component.params.iter().zip(position).zip(step) {
        let origin = match key.1 {
            Coordinate::X => component.origin[0],
            Coordinate::Y => component.origin[1],
            Coordinate::R => 0.0,
        };
        let x = x + delta;
        // Do not round-trip an unchanged parameter through normalization.
        let value = if *delta == 0.0 {
            *values.get(key)?
        } else {
            origin + x * scale
        };
        if !x.is_finite() || !value.is_finite() {
            return None;
        }
        coordinates.insert(*key, value);
        next.push(x);
    }
    Some((coordinates, next))
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
