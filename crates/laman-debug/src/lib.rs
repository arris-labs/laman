//! `laman-debug`: the tooling around a sketch that the library itself does
//! not need. Never published, and only ever a dev-dependency
//! (`docs/ARCHITECTURE.md` §Crates).
//!
//! - [`json`]: load and write a sketch's serde form as JSON text.
//! - [`strategy`]: `proptest` strategies for well-formed sketches.
#![forbid(unsafe_code)]

pub mod json;
pub mod strategy;
