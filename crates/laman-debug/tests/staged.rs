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

#[test]
fn c1_redundant_twins_name_their_extra_constraint() {
    // The head-only deletion names what the whole path names (the equality
    // is held in `assert_same_freedom`): nothing is lost to the skip.
    for name in [
        "sss-triangle-redundant",
        "belt-redundant",
        "slotted-plate-redundant",
    ] {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/c1")
            .join(name);
        let fixture = Fixture::load(&dir).unwrap();
        let sketch = placed(&fixture.sketch, &fixture.expected.coordinates);
        for algorithm in ALGORITHMS {
            let solution = assert_same_freedom(name, &sketch, &options(algorithm));
            assert!(!solution.diagnostics.redundant.is_empty(), "{name}");
        }
    }
}

#[test]
fn a_dependent_row_outside_the_head_is_still_named() {
    // `p` is fixed, `a` coincides with it, and `p` is on the line from `a`
    // to `b`: structurally the last row reads `b`, but its gradient on `b`
    // vanishes because `p = a`, so it is a numeric redundancy outside the
    // head. The head-only deletion must not skip it.
    let mut sk = laman::model::Sketch::new();
    let p = sk.point_fixed(1.0, 1.0).unwrap();
    let a = sk.point(1.0, 1.0).unwrap();
    let b = sk.point(5.0, 3.0).unwrap();
    let line = sk.line(a, b).unwrap();
    sk.coincident(p, a).unwrap();
    let on = sk.point_on_line(p, line).unwrap();
    for algorithm in ALGORITHMS {
        let solution = assert_same_freedom("dependent row", &sk, &options(algorithm));
        assert!(
            solution.diagnostics.redundant.iter().any(|r| r.id == on),
            "{algorithm:?}"
        );
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
