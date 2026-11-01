//! Bounded Levenberg–Marquardt. Public statuses and diagnostics land separately.
//!
//! Identity-damped normal equations: Madsen, Nielsen & Tingleff, *Methods
//! for Non-Linear Least Squares Problems* (2004), the LM method. The damping
//! schedule and geometric stopping policy here are measured in
//! docs/ARCHITECTURE.md, “LM engine”; no reference solver code is used.
#![allow(dead_code)] // Internal engine is exposed by M2 step 7.
use crate::model::tolerance::{self, Component, LENGTH_FLOOR, Source};
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

#[derive(Clone, Copy, Debug)]
pub(crate) struct Options {
    pub max_iterations: usize,
    pub max_evaluations: usize,
    pub initial_damping: f64,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            max_iterations: MAX_ITERATIONS,
            max_evaluations: MAX_EVALUATIONS,
            initial_damping: INITIAL_DAMPING,
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

pub(crate) fn lm(sketch: &Sketch, options: Options) -> Outcome {
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
        current.satisfied &= vector_acceptance(sketch, component, &out.coordinates, 1.0);
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
        let mut damping = options.initial_damping;
        if !damping.is_finite() || damping <= 0.0 || !cost.is_finite() {
            out.converged = false;
            continue;
        }
        while out.iterations < options.max_iterations && out.evaluations < options.max_evaluations {
            out.iterations += 1;
            let step = damped_step(&current, damping);
            let candidate = step
                .as_ref()
                .and_then(|step| trial(component, &out.coordinates, &position, step));
            // An attempted trial, including a non-finite one, consumes one
            // evaluation allowance. Linear factorization failures consume both.
            out.evaluations += 1;
            if let Some((coordinates, next_position)) = candidate {
                let mut next = system.evaluate_to(component, &coordinates, SOLVE_TARGET);
                next.satisfied &= vector_acceptance(sketch, component, &coordinates, SOLVE_TARGET);
                let next_cost = objective(&next);
                if next.valid && next_cost.is_finite() && (next_cost < cost || next.satisfied) {
                    out.coordinates = coordinates;
                    position = next_position;
                    current = next;
                    cost = next_cost;
                    damping = (damping / ACCEPT_DIVISOR).max(MIN_DAMPING);
                    if current.satisfied {
                        break;
                    }
                    continue;
                }
            }
            out.rejected += 1;
            damping *= REJECT_MULTIPLIER;
            if !damping.is_finite() {
                break;
            }
            // A zero step above tolerance is stagnation, never convergence.
            if step.as_ref().is_some_and(|s| s.iter().all(|x| *x == 0.0)) {
                break;
            }
        }
        out.converged &= current.satisfied;
    }
    out
}

// A vector condition has one geometric length tolerance. In symmetry, the
// reflected-point error is hypot(2 * midpoint-to-axis, along-axis separation).
// This complements scalar equation acceptance without importing the checker.
fn vector_acceptance(
    sketch: &Sketch,
    component: &Component,
    values: &Coordinates,
    target: f64,
) -> bool {
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
        libm::hypot(weight * a.value(), b.value()) <= target * tolerance::length(component.scale)
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

#[cfg(test)]
mod tests;
