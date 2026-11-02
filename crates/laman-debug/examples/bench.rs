//! The C1 frame budget (docs/ROADMAP.md §C1): each static fixture solves
//! from its guesses in at most 1 ms, release build, on the machine named in
//! the output. Run `cargo run --release -p laman-debug --example bench`; it
//! exits non-zero when a base or redundant twin goes over (the conflicting
//! twins are timed and printed, not budgeted: their conflict analysis is
//! bounded by `max_analysis_solves`, not by the frame).
//!
//! The time is of the whole `Sketch::solve`, diagnostics included.

use laman::solution::{Algorithm, SolverOptions};
use laman_debug::fixture::Fixture;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

/// The budget of one static solve (ROADMAP.md §C1 frame budget).
const BUDGET: Duration = Duration::from_millis(1);
/// Timed solves per fixture and algorithm; the median and the worst are
/// reported.
const RUNS: usize = 200;
/// Fixtures budgeted, then fixtures only timed.
const BUDGETED: [&str; 6] = [
    "sss-triangle",
    "sss-triangle-redundant",
    "belt",
    "belt-redundant",
    "slotted-plate",
    "slotted-plate-redundant",
];
const TIMED: [&str; 3] = [
    "sss-triangle-conflicting",
    "belt-conflicting",
    "slotted-plate-conflicting",
];

fn cpu() -> String {
    std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|text| {
            text.lines()
                .find(|l| l.starts_with("model name"))
                .and_then(|l| l.split(':').nth(1))
                .map(|name| name.trim().to_owned())
        })
        .unwrap_or_else(|| "unknown CPU".to_owned())
}

fn main() -> ExitCode {
    println!("machine: {} ({})", cpu(), std::env::consts::ARCH);
    if cfg!(debug_assertions) {
        println!("warning: a debug build; the budget is for --release");
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/c1");
    let mut over = Vec::new();
    for (names, budgeted) in [(&BUDGETED[..], true), (&TIMED[..], false)] {
        for name in names {
            let Ok(fixture) = Fixture::load(&root.join(name)) else {
                println!("{name}: cannot load");
                return ExitCode::FAILURE;
            };
            for algorithm in [Algorithm::LevenbergMarquardt, Algorithm::DogLeg] {
                let mut options = SolverOptions::default();
                options.algorithm = algorithm;
                let mut times: Vec<Duration> = (0..RUNS)
                    .map(|_| {
                        let start = Instant::now();
                        let solution = fixture.sketch.solve(&options);
                        let took = start.elapsed();
                        std::hint::black_box(solution);
                        took
                    })
                    .collect();
                times.sort();
                let (median, worst) = (times[RUNS / 2], times[RUNS - 1]);
                println!("{name} {algorithm:?}: median {median:?}, worst {worst:?}");
                if budgeted && median > BUDGET {
                    over.push(format!("{name} {algorithm:?}"));
                }
            }
        }
    }
    if over.is_empty() {
        println!("within the {BUDGET:?} budget");
        ExitCode::SUCCESS
    } else {
        println!("over the {BUDGET:?} budget: {over:?}");
        ExitCode::FAILURE
    }
}
