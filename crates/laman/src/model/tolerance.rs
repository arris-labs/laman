//! The named tolerances the model and the checker share.
//!
//! These are provisional: absolute, in the caller's units, until the
//! tolerance ADR (cycle C1, M2) decides between absolute and relative to the
//! sketch's scale. Nothing else in the workspace writes a tolerance literal.

/// How close two lengths, or a length and zero, must be to count as equal,
/// in the caller's units.
///
/// `1e-9` is close to the solver's target of a billionth of the sketch's
/// size for the C1 fixtures, which are about 100 units across. It is far
/// above the rounding of `f64` at that scale (about `1e-14`) and far below
/// any dimension a person types.
///
/// ```
/// use laman::model::tolerance;
/// assert!(tolerance::LENGTH > 0.0);
/// ```
pub const LENGTH: f64 = 1e-9;

/// How close two angles, or a dimensionless ratio and its target, must be
/// to count as equal, in radians.
///
/// `1e-12` is a few thousand times the rounding of an `atan2` result near
/// `π`, and tight enough that a length of 100 units is off by under
/// `1e-10` at its far end.
///
/// ```
/// use laman::model::tolerance;
/// assert!(tolerance::ANGLE < tolerance::LENGTH);
/// ```
pub const ANGLE: f64 = 1e-12;
