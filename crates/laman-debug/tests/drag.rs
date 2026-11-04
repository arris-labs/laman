//! The drag fixtures under `tests/fixtures/drag/`: sessions through targets
//! whose branch-keeping bisections are known by construction (each README
//! has the arithmetic), under both algorithms, checked independently at
//! every step.
#![allow(clippy::unwrap_used, clippy::panic)]

use laman::model::Coordinate;
use laman::solution::{Algorithm, FlipSubject, SolverOptions};
use laman_check::check;
use laman_debug::fixture::Fixture;
use std::path::PathBuf;

const ALGORITHMS: [Algorithm; 2] = [Algorithm::LevenbergMarquardt, Algorithm::DogLeg];

fn load(name: &str) -> Fixture {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/drag")
        .join(name);
    Fixture::load(&dir).unwrap()
}

fn tags(flips: &[laman::solution::Flip]) -> Vec<String> {
    let mut out: Vec<String> = flips
        .iter()
        .map(|f| match &f.subject {
            FlipSubject::Entity(e) => e.tag.clone().unwrap_or_default(),
            FlipSubject::Constraint(c) => c.tag.clone().unwrap_or_default(),
        })
        .collect();
    out.sort();
    out
}

/// Runs a drag fixture: the checker is clean at every step, every listed
/// step lands within the fixture's tolerance and names exactly the flips it
/// lists; an unlisted step names none.
fn run(name: &str) {
    let fixture = load(name);
    let drag = fixture.drag.as_ref().unwrap();
    for algorithm in ALGORITHMS {
        let mut options = SolverOptions::default();
        options.algorithm = algorithm;
        let mut session = fixture.sketch.drag(drag.dragged, &options).unwrap();
        for (k, target) in drag.targets.iter().enumerate() {
            let label = format!("{name} {algorithm:?} step {}", k + 1);
            let step = session.to(*target);
            assert!(
                check(&fixture.sketch, &step.coordinates).is_clean(),
                "{label}"
            );
            let listed = drag.after.iter().find(|a| a.step == k + 1);
            let want = listed.map_or(Vec::new(), |a| {
                let mut f = a.flips.clone();
                f.sort();
                f
            });
            assert_eq!(tags(&step.diagnostics.flips), want, "{label}: flips");
            for (key, expected) in listed.iter().flat_map(|a| &a.coordinates) {
                let got = step.coordinates[key];
                assert!(
                    (got - expected).abs() <= fixture.expected.tolerance,
                    "{label}: {key:?}: {got} vs {expected}"
                );
            }
        }
    }
}

#[test]
fn a_flipping_step_is_halved_to_the_first_target_that_keeps_the_branch() {
    run("bisect-keeps");
}

#[test]
fn a_step_that_flips_at_every_halving_stays_put_and_names_the_flip() {
    run("bisect-stuck");
}

/// A frame that moves an arc's sweep past `MAX_SWEEP_STEP` is bisected
/// without naming a flip (ADR-0006): the end lands at the second halving,
/// sweep 0.465, not at the target's own 3.09 that the tracking would have
/// read as a short move.
#[test]
fn a_sweep_jump_is_bisected_without_a_flip() {
    run("sweep-jump");
    let fixture = load("sweep-jump");
    let drag = fixture.drag.as_ref().unwrap();
    for algorithm in ALGORITHMS {
        let mut options = SolverOptions::default();
        options.algorithm = algorithm;
        let mut session = fixture.sketch.drag(drag.dragged, &options).unwrap();
        let step = session.to(drag.targets[0]);
        let at = |c| step.coordinates[&(drag.dragged, c)];
        let sweep = libm::atan2(at(Coordinate::Y), at(Coordinate::X));
        assert!(
            sweep > 0.3 && sweep - 0.3 <= laman::model::tolerance::MAX_SWEEP_STEP,
            "{algorithm:?}: sweep {sweep}"
        );
    }
}
