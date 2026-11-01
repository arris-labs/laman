//! The finite-difference rule: an analytic gradient held against a
//! numerical one, for any `Fn(&[f64]) -> f64`.
//!
//! # The rule
//!
//! Each partial derivative is taken by the eighth-order central stencil:
//! nine nodes `x_i + k h`, `k = −4..4`, with step `h = STEP · ℓ`, `ℓ` the
//! equation's **local scale** (the shortest length among the lines, radii
//! and point pairs it reads; the caller computes it). Its weights are
//! Fornberg's for the nodes as `f64` rounds them, not for the ideal `k h`.
//! An analytic gradient `J` agrees when every term satisfies
//!
//! ```text
//! |J_i − D_i| ≤ AGREEMENT · max(‖J‖∞, floor)
//! ```
//!
//! where `floor` is the natural size of a gradient of that unit: 1 for a
//! length residual ([`Rule::length`]), `1/ℓ` for a dimensionless one
//! ([`Rule::ratio`]). The floor keeps a vanishing gradient from demanding
//! an exact zero of the stencil.
//!
//! # The bound, and why it is enough
//!
//! The rule is sound while `X/ℓ ≤ 1e6`, `X` the magnitude of the
//! coordinates: a line a millionth of the sketch's size, anywhere in it.
//! The stencil's error has three sources.
//!
//! - **Truncation**, `h⁸/630 · |f⁽⁹⁾|`. A residual is built of rational
//!   functions and square roots whose nearest singularity is at least `ℓ`
//!   away, so by Cauchy's estimate `|f⁽⁹⁾| ≲ 9! 2⁹ N / ℓ⁸`, `N` the
//!   gradient's natural size (which a configuration can make exceed `‖J‖∞`
//!   by up to `X/ℓ`: a point square off a short line's end). That is
//!   `1.2e-13 N ≤ 1.2e-7 · max(‖J‖∞, floor)` at `h/ℓ = 5e-3`.
//! - **Rounding of `f`**: each evaluation errs by about `c ε X · floor` (`ε`
//!   the machine epsilon, `c` a few units), and the weights sum to `2.08/h`
//!   in magnitude, giving `2.08 c ε (X/ℓ)/(h/ℓ) · floor ≈ 9e-8 c · floor`.
//! - **Rounding of the nodes**: `x_i + k h` is off by up to half an ulp of
//!   `X`, `1e-8 h` here; across a stencil whose slope grows to `N · 4h/ℓ`
//!   that alone can reach `1e-4` of the floor (measured past `1e-6`). The
//!   weights for the rounded nodes remove it: their offsets from `x_i` are
//!   exact (Sterbenz's lemma).
//!
//! Measured: over 150 000 cases of the self-tests (sizes `1e-6`, `1`,
//! `1e3`; `X/ℓ` up to `2e6`; the adversarial configurations included) the
//! worst disagreement is under `3e-7 · max(‖J‖∞, floor)`, a third of
//! [`AGREEMENT`]. A step of `1e-6 ℓ` with the three-point stencil has
//! rounding `ε (X/ℓ)/1e-6 = 2e-4` of the floor at `X/ℓ = 1e6`: it cannot
//! check a short line far from the origin, which is why the step is wide
//! and the order high.
//!
//! What the rule sees: an error in any term above `1.3 · AGREEMENT ·
//! max(‖J‖∞, floor)`. A negated, dropped or mis-scaled term is caught
//! wherever it is not a millionth of the largest term. In a near-degenerate
//! configuration small terms hide under large ones (a point's term beside
//! a short line's, `X/ℓ` times bigger), so the property tests also draw
//! generic configurations, where every term has its natural size and each
//! is seen. Beyond `X/ℓ = 1e6` the rule is not sound and a caller does not
//! use it.

/// The step of the stencil, as a fraction of the local scale. Not a
/// tolerance: it balances truncation (`(h/ℓ)⁸`, against a gradient up to
/// `1e6` times its natural size) and rounding (`ε (X/ℓ)/(h/ℓ)`), both under
/// [`AGREEMENT`] up to `X/ℓ = 1e6` (the module doc gives the bound).
pub const STEP: f64 = 5.0e-3;

/// The agreement demanded of each term, relative to the gradient's size.
/// The roadmap's number (docs/ROADMAP.md §M1); the module doc shows the
/// stencil's own error stays under it.
pub const AGREEMENT: f64 = 1.0e-6;

/// How one equation is held: its local scale and the size below which a
/// gradient of its unit counts as zero.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rule {
    local_scale: f64,
    floor: f64,
}

impl Rule {
    /// The rule for a length residual (its gradient is dimensionless), at
    /// local scale `local_scale`. A scale that is not positive and finite
    /// is refused by [`check`].
    pub fn length(local_scale: f64) -> Self {
        Self {
            local_scale,
            floor: 1.0,
        }
    }

    /// The rule for a dimensionless residual (its gradient is an inverse
    /// length, `1/ℓ` in size), at local scale `local_scale`. A scale that is
    /// not positive and finite is refused by [`check`].
    pub fn ratio(local_scale: f64) -> Self {
        Self {
            local_scale,
            floor: 1.0 / local_scale,
        }
    }

    /// The equation's local scale.
    pub fn local_scale(&self) -> f64 {
        self.local_scale
    }

    /// The stencil's step, [`STEP`] of the local scale.
    pub fn step(&self) -> f64 {
        STEP * self.local_scale
    }
}

/// Why a gradient was not accepted.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum Disagreement {
    /// The gradient has a different length than the point.
    #[error("the gradient has {gradient} terms for a point of {point} coordinates")]
    Shape {
        /// The point's length.
        point: usize,
        /// The gradient's length.
        gradient: usize,
    },
    /// The local scale is not a positive finite length, or the point or
    /// gradient holds a non-finite number.
    #[error("local scale {0} is not a positive finite length, or an input is not finite")]
    Input(f64),
    /// A term differs from the stencil by more than the rule allows; the
    /// worst one by its ratio to the bound.
    #[error(
        "term {index}: analytic {analytic}, numerical {numerical}, differ by more than {bound}"
    )]
    Term {
        /// The parameter's position in the point.
        index: usize,
        /// The analytic term.
        analytic: f64,
        /// The stencil's derivative (non-finite when `f` was).
        numerical: f64,
        /// What the rule allowed.
        bound: f64,
    },
}

/// The stencil's partial derivative of `f` at `x` along parameter `index`,
/// with step `step`; `None` when `index` is outside `x`.
pub fn derivative(f: &impl Fn(&[f64]) -> f64, x: &[f64], index: usize, step: f64) -> Option<f64> {
    let centre = *x.get(index)?;
    // The nodes as `f64` holds them, and their offsets from the centre
    // (exact by Sterbenz's lemma once the step is under the coordinate).
    let mut nodes = [centre; NODES];
    let mut offsets = [0.0; NODES];
    for k in 1..=PAIRS {
        let d = k as f64 * step;
        for (slot, node) in [(2 * k - 1, centre + d), (2 * k, centre - d)] {
            if let (Some(n), Some(o)) = (nodes.get_mut(slot), offsets.get_mut(slot)) {
                *n = node;
                *o = node - centre;
            }
        }
    }
    let weights = fornberg(&offsets);
    let mut at = x.to_vec();
    // A fixed summation order, the centre first: same bits every run.
    let mut sum = 0.0;
    for (node, w) in nodes.iter().zip(weights) {
        if let Some(slot) = at.get_mut(index) {
            *slot = *node;
        }
        sum += w * f(&at);
    }
    Some(sum)
}

/// Symmetric node pairs of the stencil: eighth order.
const PAIRS: usize = 4;
/// The stencil's nodes: the centre and its pairs.
const NODES: usize = 2 * PAIRS + 1;

/// The weights of the first derivative at offset 0 over `offsets`, by
/// Fornberg's recurrence (B. Fornberg, "Generation of finite difference
/// formulas on arbitrarily spaced grids", Math. Comp. 51 (1988) 699–706).
/// Weights on the nodes as rounded, not on the ideal `k h`: at a coordinate
/// a million times the step, rounding a node moves it by `1e-8 h`, and over
/// a slope that grows across the stencil that error alone exceeds
/// [`AGREEMENT`].
fn fornberg(offsets: &[f64; NODES]) -> [f64; NODES] {
    // c[j] = (weight of the value, weight of the first derivative) at node j.
    let mut c = [[0.0_f64; 2]; NODES];
    c[0][0] = 1.0;
    let mut c1 = 1.0;
    let mut c4 = offsets[0];
    for i in 1..NODES {
        let mut c2 = 1.0;
        let c5 = c4;
        c4 = offsets[i];
        for j in 0..i {
            let c3 = offsets[i] - offsets[j];
            c2 *= c3;
            if j == i - 1 {
                c[i][1] = c1 * (c[i - 1][0] - c5 * c[i - 1][1]) / c2;
                c[i][0] = -c1 * c5 * c[i - 1][0] / c2;
            }
            c[j][1] = (c4 * c[j][1] - c[j][0]) / c3;
            c[j][0] = c4 * c[j][0] / c3;
        }
        c1 = c2;
    }
    c.map(|w| w[1])
}

/// Holds `gradient`, the analytic gradient of `f` at `x`, against the
/// stencil by `rule`. `Ok` when every term agrees; otherwise the worst
/// term, or the malformed input.
///
/// ```
/// use laman_debug::fd::{check, Rule};
/// // |v| at (3, 4): gradient (3/5, 4/5); local scale |v| = 5.
/// let norm = |v: &[f64]| (v[0] * v[0] + v[1] * v[1]).sqrt();
/// assert!(check(norm, &[3.0, 4.0], &[0.6, 0.8], Rule::length(5.0)).is_ok());
/// assert!(check(norm, &[3.0, 4.0], &[0.6, -0.8], Rule::length(5.0)).is_err());
/// ```
pub fn check(
    f: impl Fn(&[f64]) -> f64,
    x: &[f64],
    gradient: &[f64],
    rule: Rule,
) -> Result<(), Disagreement> {
    if x.len() != gradient.len() {
        return Err(Disagreement::Shape {
            point: x.len(),
            gradient: gradient.len(),
        });
    }
    let finite = x.iter().chain(gradient).all(|v| v.is_finite());
    if !(rule.local_scale > 0.0 && rule.local_scale.is_finite() && finite) {
        return Err(Disagreement::Input(rule.local_scale));
    }
    let size = gradient.iter().fold(rule.floor, |m, g| m.max(g.abs()));
    let bound = AGREEMENT * size;
    let mut worst: Option<(f64, Disagreement)> = None;
    for (index, &analytic) in gradient.iter().enumerate() {
        let numerical = derivative(&f, x, index, rule.step()).unwrap_or(f64::NAN);
        let excess = (analytic - numerical).abs() / bound;
        // A NaN from `f` is the worst disagreement, never a pass.
        let excess = if excess.is_nan() {
            f64::INFINITY
        } else {
            excess
        };
        if excess > 1.0 && worst.as_ref().is_none_or(|(w, _)| excess > *w) {
            let term = Disagreement::Term {
                index,
                analytic,
                numerical,
                bound,
            };
            worst = Some((excess, term));
        }
    }
    match worst {
        None => Ok(()),
        Some((_, term)) => Err(term),
    }
}
