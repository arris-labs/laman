//! The oracle: other Rust constraint solvers, run out of process on laman's
//! C1 fixtures (`README.md`).
//!
//! ```text
//! laman-oracle <solver> <fixture-dir>                      one report, JSON on stdout
//! laman-oracle crashed <solver> <fixture-dir> <outcome> <detail>
//!                                                         a report for a run that died
//! laman-oracle hello <solver>                              what the API exposes (README)
//! ```
//!
//! `run.sh` calls it once per solver and fixture, each in its own process
//! under a timeout, so a panic or a hang in a foreign solver is a report.

mod fixture;
mod hello;
mod report;

use std::path::Path;
use std::process::ExitCode;

use report::{Outcome, Report};

/// Run one solver on one fixture. The stub stands in for a solver in the
/// tests of the plumbing; a solver without an adapter yet reports `error`.
fn run(solver: &str, fx: &fixture::Fixture) -> Report {
    match solver {
        "stub" => Report::without_numbers(solver, &fx.name, Outcome::Unsupported, "stub"),
        "sketch-solver" | "acs-solver" | "ezpz" => {
            Report::without_numbers(solver, &fx.name, Outcome::Error, "no adapter yet")
        }
        _ => Report::without_numbers(solver, &fx.name, Outcome::Error, "unknown solver"),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        ["hello", "sketch-solver"] => hello::sketch_solver(),
        ["hello", "acs-solver"] => hello::acs_solver(),
        ["hello", "ezpz"] => hello::ezpz(),
        ["crashed", solver, dir, outcome, detail] => {
            let outcome = match *outcome {
                "timeout" => Outcome::Timeout,
                _ => Outcome::Crashed,
            };
            let name = Path::new(dir)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(dir);
            print!(
                "{}",
                Report::without_numbers(solver, name, outcome, *detail).to_json()
            );
        }
        [solver, dir] => match fixture::load(Path::new(dir)) {
            Ok(fx) => print!("{}", run(solver, &fx).to_json()),
            Err(e) => {
                let name = Path::new(dir)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(dir);
                print!(
                    "{}",
                    Report::without_numbers(solver, name, Outcome::Error, e).to_json()
                );
            }
        },
        _ => {
            eprintln!(
                "usage: laman-oracle <solver> <fixture-dir> | hello <solver> | crashed <solver> <fixture-dir> <outcome> <detail>"
            );
            return ExitCode::from(2);
        }
    }
    ExitCode::SUCCESS
}
