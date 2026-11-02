//! Rank, geometric DoF, per-entity DoF and free directions at given coordinates.
//!
//! The Jacobian is the compiled [`System`]'s: required equations after
//! structural contact regularization (ADR-0002), intrinsic equations included,
//! scaled per component and row-normalized as ADR-0003 fixes. Rank is a
//! column-pivoted Householder QR (Businger & Golub, *Linear least squares
//! solutions by Householder transformations*, Numer. Math. 7, 1965) whose
//! column norms are recomputed at every step and whose ties go to the lowest
//! original parameter, so no library tie rule reaches a result. Every sum runs
//! in index order. Status, redundancy, conflicts and degeneracy build on it
//! in [`analysis`] (ADR-0005).
use crate::model::tolerance::{self, Component, RANK};
use crate::model::{Coordinate, Coordinates, EntityId, Sketch};
use crate::system::System;
use std::collections::BTreeMap;

type Param = (EntityId, Coordinate);

mod analysis;
pub use analysis::*;

/// The freedom left at some coordinates.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Freedom {
    /// Free parameters minus the rank of the regularized Jacobian.
    pub dof: usize,
    /// Every entity's DoF: the rank of the null basis restricted to the
    /// free parameters it reads (a line's are its two points').
    pub entities: BTreeMap<EntityId, usize>,
    /// An orthonormal null basis, component by component, each vector
    /// listed by the parameters it moves in compiled order.
    pub directions: Vec<Vec<(EntityId, Coordinate, f64)>>,
}

/// [`Freedom`] of `sketch` at `values` (missing keys read from the sketch).
pub(crate) fn freedom(sketch: &Sketch, values: &Coordinates) -> Freedom {
    freedom_of(&System::new(sketch), values)
}

/// The `dof` of [`freedom`] alone: the same ranks from the same
/// factorizations, without the null basis or the per-entity counts, which
/// is all redundancy needs and most of what a freedom costs.
pub(crate) fn dof_only(sketch: &Sketch, values: &Coordinates) -> usize {
    let system = System::new(sketch);
    let coordinates = merged(sketch, values);
    system
        .components
        .iter()
        .map(|component| component.params.len() - rank_of(&system, component, &coordinates))
        .sum()
}

/// The sketch's coordinates with `values` laid over them.
fn merged(sketch: &Sketch, values: &Coordinates) -> Coordinates {
    let mut coordinates = sketch.coordinates();
    for (key, value) in coordinates.iter_mut() {
        if let Some(v) = values.get(key) {
            *value = *v;
        }
    }
    coordinates
}

/// [`Freedom`] of an already compiled system.
pub(crate) fn freedom_of(system: &System<'_>, values: &Coordinates) -> Freedom {
    let coordinates = merged(system.sketch, values);
    let mut dof = 0;
    let mut basis: Vec<Vec<(Param, f64)>> = Vec::new();
    for component in &system.components {
        let (rank, vectors) = null_space(system, component, &coordinates);
        dof += component.params.len() - rank;
        basis.extend(vectors.into_iter().map(|v| {
            component
                .params
                .iter()
                .copied()
                .zip(v)
                .filter(|(_, x)| *x != 0.0)
                .collect::<Vec<_>>()
        }));
    }
    let entities = system
        .sketch
        .entities()
        .map(|(id, _)| {
            let mut params = tolerance::entity_params(system.sketch, id);
            params.sort();
            params.dedup();
            let rows: Vec<Vec<f64>> = params
                .iter()
                .map(|p| {
                    basis
                        .iter()
                        .map(|v| v.iter().find(|(q, _)| q == p).map_or(0.0, |(_, x)| *x))
                        .collect()
                })
                .collect();
            (id, Qr::new(rows, basis.len()).rank)
        })
        .collect();
    let directions = basis
        .into_iter()
        .map(|v| v.into_iter().map(|((id, c), x)| (id, c, x)).collect())
        .collect();
    Freedom {
        dof,
        entities,
        directions,
    }
}

/// A component's rank and its null basis over `component.params`: the
/// required equations' only, which is also the space a goal moves in.
pub(crate) fn null_space(
    system: &System<'_>,
    component: &Component,
    values: &Coordinates,
) -> (usize, Vec<Vec<f64>>) {
    if component.params.is_empty() {
        return (0, vec![]);
    }
    let qr = component_qr(system, component, values);
    (qr.rank, qr.null_basis())
}

/// A component's rank alone: [`null_space`]'s, without the basis.
fn rank_of(system: &System<'_>, component: &Component, values: &Coordinates) -> usize {
    if component.params.is_empty() {
        return 0;
    }
    component_qr(system, component, values).rank
}

/// The factorization of a component's unit-row Jacobian at `values`.
fn component_qr(system: &System<'_>, component: &Component, values: &Coordinates) -> Qr {
    // A non-finite row (an unreadable configuration) contributes no rank:
    // the DoF is then an upper estimate, and degeneracy is reported separately.
    let rows = if component.sources.is_empty() {
        vec![]
    } else {
        system
            .evaluate(component, values)
            .jacobian
            .into_iter()
            .filter_map(unit_row)
            .collect()
    };
    Qr::new(rows, component.params.len())
}

/// The row scaled to unit Euclidean norm; `None` for a zero or non-finite row.
/// Dividing by the largest magnitude first keeps the squares representable.
fn unit_row(row: Vec<f64>) -> Option<Vec<f64>> {
    let big = row.iter().fold(0.0, |m: f64, x| m.max(x.abs()));
    if big == 0.0 || !big.is_finite() {
        return None;
    }
    let scaled: Vec<f64> = row.iter().map(|x| x / big).collect();
    let norm = scaled.iter().fold(0.0, |s, x| s + x * x).sqrt();
    Some(scaled.into_iter().map(|x| x / norm).collect())
}

/// Column-pivoted Householder QR of an `m × n` matrix, stopped at the first
/// remaining column whose part outside the chosen columns' span is at most
/// [`RANK`]. `a` holds R in its first `rank` rows, columns permuted by `perm`.
struct Qr {
    a: Vec<Vec<f64>>,
    perm: Vec<usize>,
    rank: usize,
}

impl Qr {
    fn new(mut a: Vec<Vec<f64>>, n: usize) -> Self {
        let m = a.len();
        let mut perm: Vec<usize> = (0..n).collect();
        let mut rank = 0;
        for k in 0..m.min(n) {
            // Recomputed norms (no downdating): ties go to the lowest original column.
            let mut best: Option<(f64, usize)> = None;
            for j in k..n {
                let norm = (k..m).fold(0.0, |s, i| s + a[i][j] * a[i][j]).sqrt();
                let better = match best {
                    None => true,
                    Some((b, bj)) => norm > b || (norm == b && perm[j] < perm[bj]),
                };
                if better {
                    best = Some((norm, j));
                }
            }
            let Some((norm, j)) = best else { break };
            // A NaN norm ends the factorization too.
            if norm.is_nan() || norm <= RANK {
                break;
            }
            perm.swap(k, j);
            for row in a.iter_mut() {
                row.swap(k, j);
            }
            // Reflect column k onto alpha e_k, alpha of the sign that avoids cancellation.
            let alpha = if a[k][k] > 0.0 { -norm } else { norm };
            let mut v: Vec<f64> = (k..m).map(|i| a[i][k]).collect();
            v[0] -= alpha;
            let vv = v.iter().fold(0.0, |s, x| s + x * x);
            // Column sums over row-major storage: the index is the column.
            #[allow(clippy::needless_range_loop)]
            for col in k + 1..n {
                let dot = (k..m).fold(0.0, |s, i| s + v[i - k] * a[i][col]);
                let f = 2.0 * dot / vv;
                for i in k..m {
                    a[i][col] -= f * v[i - k];
                }
            }
            a[k][k] = alpha;
            for row in a.iter_mut().skip(k + 1) {
                row[k] = 0.0;
            }
            rank += 1;
        }
        Self { a, perm, rank }
    }

    /// ADR-0003's null basis: one vector per non-pivot column in original
    /// order (that column 1, the other non-pivots 0, the pivots solved from
    /// R), unpermuted, then orthonormalized in that order with two
    /// Gram–Schmidt passes. Each is unit; components at or below [`RANK`]
    /// are canonical +0; the first nonzero component is positive.
    fn null_basis(&self) -> Vec<Vec<f64>> {
        let n = self.perm.len();
        let r = self.rank;
        let mut free: Vec<usize> = (r..n).collect();
        free.sort_by_key(|q| self.perm[*q]);
        let mut basis: Vec<Vec<f64>> = Vec::with_capacity(free.len());
        for q in free {
            let mut z = vec![0.0; n];
            z[q] = 1.0;
            for i in (0..r).rev() {
                let s = (i + 1..r).fold(self.a[i][q], |s, j| s + self.a[i][j] * z[j]);
                z[i] = -s / self.a[i][i];
            }
            let mut x = vec![0.0; n];
            for (i, value) in z.into_iter().enumerate() {
                x[self.perm[i]] = value;
            }
            for _ in 0..2 {
                for b in &basis {
                    let d = dot(b, &x);
                    for (xi, bi) in x.iter_mut().zip(b) {
                        *xi -= d * bi;
                    }
                }
            }
            let norm = dot(&x, &x).sqrt();
            // Unreachable in exact arithmetic (each vector owns a unit
            // non-pivot coordinate); a back substitution that overflowed is
            // dropped rather than reported as a direction.
            if !(norm > 0.0 && norm.is_finite()) {
                continue;
            }
            for xi in &mut x {
                *xi /= norm;
            }
            basis.push(x);
        }
        for x in &mut basis {
            let flip = x.iter().find(|v| v.abs() > RANK).is_some_and(|v| *v < 0.0);
            for xi in x.iter_mut() {
                *xi = if xi.abs() <= RANK {
                    0.0
                } else if flip {
                    -*xi
                } else {
                    *xi
                };
            }
        }
        basis
    }
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).fold(0.0, |s, (x, y)| s + x * y)
}

#[cfg(test)]
mod tests;
