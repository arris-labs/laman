//! `laman`: a deterministic 2D geometric constraint solver in pure Rust.
//!
//! The solver (sketches of points, lines, circles and arcs, their
//! constraints, the numerical solve and its diagnostics) is being built;
//! `SEED.md` in the repository is the charter. This early release holds the
//! combinatorial core the name comes from: [`rigidity`], the generic
//! rigidity count of a bar framework in the plane (Laman's theorem), with
//! the bars that are redundant named.
//!
//! ```
//! use laman::rigidity;
//!
//! // A triangle is rigid; a fourth point on two bars makes it a rigid
//! // "diamond"; one more bar across it is redundant.
//! let bars = [(0, 1), (1, 2), (2, 0), (1, 3), (2, 3), (0, 3)];
//! let r = rigidity(4, &bars).unwrap();
//! assert_eq!(r.dof, 0);
//! assert_eq!(r.redundant, vec![5]);
//! ```
#![forbid(unsafe_code)]

mod branch;
mod diagnose;
pub mod model;
pub mod residual;
mod rigidity;
pub mod solution;
mod solve;
mod system;

pub use rigidity::{Rigidity, RigidityError, rigidity};

/// The README's Rust example runs as a doctest, so it cannot drift.
#[doc = include_str!("../../../README.md")]
#[cfg(doctest)]
struct ReadmeDoctests;
