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
#![allow(dead_code)] // Internal engine is exposed by M2 step 7.
use crate::model::tolerance::{Component, LENGTH_FLOOR, Source};
use crate::model::{Constraint, Coordinate, Coordinates, Sketch};
use crate::system::{Evaluation, System};
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
/// LAPACK's bidiagonal SVD (xBDSQR) allows 6 k^2 inner QR steps for k
/// singular values; the same hard bound, so a factorization cannot hang.
const SVD_STEPS_PER_VALUE_SQUARED: usize = 6;

/// Which step an iteration proposes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Algorithm {
    LevenbergMarquardt,
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
    let system = System::new(sketch);
    let mut out = Outcome {
        coordinates: sketch.coordinates(),
        converged: true,
        iterations: 0,
        evaluations: 0,
        rejected: 0,
    };
    for component in &system.components {
        if component.sources.is_empty() {
            continue;
        }
        if out.evaluations == options.max_evaluations {
            out.converged = false;
            continue;
        }
        out.evaluations += 1;
        let mut current = system.evaluate(component, &out.coordinates);
        current.satisfied &= vector_acceptance(&system, component, &out.coordinates, 1.0);
        if current.satisfied {
            continue;
        } // Preserve every bit, including signed zero.
        let Some(mut position) = normalized(component, &out.coordinates) else {
            out.converged = false;
            continue;
        };
        if !current.valid || component.params.is_empty() {
            out.converged = false;
            continue;
        }
        let mut cost = objective(&current);
        let Some(mut control) = Control::new(&options).filter(|_| cost.is_finite()) else {
            out.converged = false;
            continue;
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
                next.satisfied &= vector_acceptance(&system, component, &coordinates, SOLVE_TARGET);
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
        out.converged &= current.satisfied;
    }
    out
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
