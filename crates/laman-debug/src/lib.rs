//! `laman-debug`: the tooling around a sketch that the library itself does
//! not need. Never published, and only ever a dev-dependency
//! (`docs/ARCHITECTURE.md` §Crates).
//!
//! - [`json`]: load and write a sketch's serde form as JSON text.
//! - [`fixture`]: load a fixture directory (its sketch, expectations, drag).
//! - [`runner`]: check a fixture at its expected coordinates and digest it.
//! - [`digest`]: the SHA-256 over a run's bits.
//! - [`strategy`]: `proptest` strategies for well-formed sketches.
#![forbid(unsafe_code)]

pub mod digest;
pub mod fixture;
pub mod json;
pub mod runner;
pub mod strategy;
