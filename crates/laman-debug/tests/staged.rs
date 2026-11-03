//! The staged freedom equals the whole-component reference (plans/m6-stages
//! step 2): DoF and per-entity DoF exactly, the free directions' projectors
//! within `PROJECTOR`, on the C1 fixtures at their expected coordinates and
//! the C2 corpus at its constructed ones, under both algorithms. The solve
//! properties hold the same in `solved.rs`.
#![allow(clippy::unwrap_used)]

#[path = "support/staged.rs"]
mod support;

use laman::solution::{Algorithm, SolverOptions};
use laman_debug::corpus::{Case, rect_chain, slot_grid, triangle_strip};
use laman_debug::fixture::Fixture;
use std::path::PathBuf;
use support::{assert_same_freedom, placed};

const ALGORITHMS: [Algorithm; 2] = [Algorithm::LevenbergMarquardt, Algorithm::DogLeg];

fn options(algorithm: Algorithm) -> SolverOptions {
    let mut options = SolverOptions::default();
    options.algorithm = algorithm;
    options
}

const C1: [&str; 10] = [
    "sss-triangle",
    "sss-triangle-redundant",
    "sss-triangle-conflicting",
    "belt",
    "belt-redundant",
    "belt-conflicting",
    "slotted-plate",
    "slotted-plate-redundant",
    "slotted-plate-conflicting",
    "slotted-plate-drag",
];

#[test]
fn c1_at_expected_coordinates() {
    for name in C1 {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/c1")
            .join(name);
        let fixture = Fixture::load(&dir).unwrap();
        let sketch = placed(&fixture.sketch, &fixture.expected.coordinates);
        for algorithm in ALGORITHMS {
            let solution = assert_same_freedom(name, &sketch, &options(algorithm));
            assert_eq!(
                solution.diagnostics.dof as u32, fixture.expected.dof,
                "{name}"
            );
        }
    }
}

fn corpus(name: &str, case: &Case) {
    let sketch = placed(&case.sketch, &case.expected.coordinates);
    for algorithm in ALGORITHMS {
        let solution = assert_same_freedom(name, &sketch, &options(algorithm));
        assert_eq!(solution.diagnostics.dof as u32, case.expected.dof, "{name}");
        if let Some(drag) = &case.drag {
            let solution = assert_same_freedom(name, &drag.sketch, &options(algorithm));
            assert_eq!(
                solution.diagnostics.dof as u32, drag.expected.dof,
                "{name} drag"
            );
        }
    }
}

#[test]
fn c2_corpus_at_constructed_coordinates() {
    corpus("rect-chain-3", &rect_chain(3).unwrap());
    corpus("rect-chain-19", &rect_chain(19).unwrap());
    corpus("slot-grid-2-2", &slot_grid(2, 2).unwrap());
    corpus("triangle-strip-6", &triangle_strip(6).unwrap());
    corpus("triangle-strip-32", &triangle_strip(32).unwrap());
}
