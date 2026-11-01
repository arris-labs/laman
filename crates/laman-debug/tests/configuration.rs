//! The configuration strategy (`laman_debug::strategy::configuration`):
//! every variant's sketch builds with the constraint it names, every mode a
//! variant lists is reached, and every length a local scale counts stays at
//! least a millionth of the sketch's size, which is what the
//! finite-difference rule (`laman_debug::fd`) needs to be sound.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests: a failing unwrap is the test failing (clippy.toml)

use std::collections::BTreeSet;

use laman_debug::strategy::configuration::{
    Configuration, MIN_LENGTH, Mode, configuration, configuration_in, variants,
};
use proptest::strategy::{Strategy, ValueTree};
use proptest::test_runner::TestRunner;

const DRAWS: usize = 300;

fn draws<S: Strategy<Value = Configuration>>(strategy: S, n: usize) -> Vec<Configuration> {
    let mut runner = TestRunner::deterministic();
    (0..n)
        .map(|_| strategy.new_tree(&mut runner).unwrap().current())
        .collect()
}

#[test]
fn there_are_at_least_thirty_variants_and_each_builds_its_constraint() {
    let all = variants();
    assert!(all.len() >= 30, "{} variants", all.len());
    for (i, v) in all.iter().enumerate() {
        assert!(!all[..i].contains(v), "{} listed twice", v.name());
    }
    for v in all {
        for c in draws(configuration(v), 20) {
            let built = c.sketch.constraint(c.constraint).unwrap();
            assert_eq!(built.kind(), v.kind(), "{}", v.name());
            assert_eq!(c.sketch.constraints().count(), 1, "{}", v.name());
        }
    }
}

#[test]
fn every_mode_of_every_variant_is_reached() {
    for v in variants() {
        let modes = v.modes();
        assert_eq!(modes[0], Mode::Generic, "{}", v.name());
        let reached: BTreeSet<Mode> = draws(configuration(v), DRAWS)
            .iter()
            .map(|c| c.mode)
            .collect();
        assert_eq!(
            reached,
            modes.iter().copied().collect::<BTreeSet<_>>(),
            "{}",
            v.name()
        );
        for mode in modes {
            for c in draws(configuration_in(v, mode), 20) {
                assert_eq!((c.variant, c.mode), (v, mode));
            }
        }
    }
}

#[test]
fn every_counted_length_is_at_least_a_millionth_of_the_size() {
    for v in variants() {
        for mode in v.modes() {
            for c in draws(configuration_in(v, mode), DRAWS) {
                assert!(c.size.is_finite() && c.size > 0.0, "{}", v.name());
                let least = c.lengths().into_iter().fold(f64::INFINITY, f64::min);
                assert!(
                    least >= MIN_LENGTH * c.size,
                    "{} in {mode:?}: a length {least:e} of size {:e}",
                    v.name(),
                    c.size
                );
            }
        }
    }
}

#[test]
fn the_short_line_mode_reaches_the_short_end_of_the_bound() {
    // Not just "not too short": the mode does draw lines near the limit.
    for v in variants() {
        if !v.modes().contains(&Mode::ShortLine) {
            continue;
        }
        let least = draws(configuration_in(v, Mode::ShortLine), DRAWS)
            .iter()
            .map(|c| c.lengths().into_iter().fold(f64::INFINITY, f64::min) / c.size)
            .fold(f64::INFINITY, f64::min);
        assert!(
            least < 1.0e-5,
            "{}: shortest {least:e} of the size",
            v.name()
        );
    }
}

#[test]
fn the_on_mode_satisfies_its_constraint_by_the_independent_checker() {
    for v in variants() {
        if !v.modes().contains(&Mode::On) {
            continue;
        }
        for c in draws(configuration_in(v, Mode::On), 100) {
            let report = laman_check::check(&c.sketch, &c.sketch.coordinates());
            assert!(report.is_clean(), "{} in On: {report:?}", v.name());
        }
    }
}
