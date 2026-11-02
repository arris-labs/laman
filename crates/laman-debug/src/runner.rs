//! The runner: a fixture loaded, checked at its expected coordinates and
//! digested ([`run`]), or solved from its guesses, checked at the solution
//! and digested ([`solve_run`]).

use crate::digest::{Digest, of_drag_step, of_run, of_solution};
use crate::fixture::Fixture;
use laman::solution::{Solution, SolverOptions};
use laman_check::{Report, check};

/// What a run found: the checker's report at the expected coordinates, and
/// the digest of the run.
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    /// The checker's report.
    pub report: Report,
    /// The digest of the sketch and the report.
    pub digest: Digest,
}

/// Checks a fixture at its expected coordinates and digests the result.
///
/// The coordinates checked are the sketch's, with each listed expected
/// coordinate replacing its guess; a parameter the expectations do not name
/// keeps the sketch's value (a ground point's, say).
///
/// ```
/// use laman_debug::fixture::Fixture;
/// use laman_debug::runner::run;
///
/// let fixture = Fixture::parse(
///     r#"{"format":"laman.sketch","version":1,"entities":[{"id":0,"kind":"point","x":0.0,"y":0.0},{"id":1,"kind":"point","x":3.0,"y":4.0}],"constraints":[{"id":0,"kind":"distance","a":0,"b":1,"value":5.0}]}"#,
///     r#"{"status":"FullyConstrained","dof":0,"coordinates":[]}"#,
///     None,
/// )?;
/// assert!(run(&fixture).report.is_clean());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn run(fixture: &Fixture) -> Run {
    let mut values = fixture.sketch.coordinates();
    for (key, value) in &fixture.expected.coordinates {
        values.insert(*key, *value);
    }
    let report = check(&fixture.sketch, &values);
    let digest = of_run(&fixture.sketch, &report);
    Run { report, digest }
}

/// What a solved run found: the solution, the checker's report at its
/// coordinates and the digest of the run.
#[derive(Debug, Clone, PartialEq)]
pub struct SolvedRun {
    /// The solution from the fixture's guesses.
    pub solution: Solution,
    /// The checker's report at the solution's coordinates.
    pub report: Report,
    /// The digest of the sketch, the solution and the report.
    pub digest: Digest,
}

/// Solves a fixture's sketch from its guesses and checks the answer with the
/// independent checker. Comparing with the expectations is the caller's.
///
/// ```
/// use laman::solution::{SolverOptions, Status};
/// use laman_debug::fixture::Fixture;
/// use laman_debug::runner::solve_run;
///
/// let fixture = Fixture::parse(
///     r#"{"format":"laman.sketch","version":1,"entities":[{"id":0,"kind":"point","x":0.0,"y":0.0,"fixed":["x","y"]},{"id":1,"kind":"point","x":3.0,"y":1.0}],"constraints":[{"id":0,"kind":"distance","a":0,"b":1,"value":5.0}]}"#,
///     r#"{"status":"UnderConstrained","dof":1,"coordinates":[]}"#,
///     None,
/// )?;
/// let solved = solve_run(&fixture, &SolverOptions::default());
/// assert_eq!(solved.solution.status, Status::UnderConstrained);
/// assert!(solved.report.is_clean());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn solve_run(fixture: &Fixture, options: &SolverOptions) -> SolvedRun {
    let solution = fixture.sketch.solve(options);
    let report = check(&fixture.sketch, &solution.coordinates);
    let digest = of_solution(&fixture.sketch, &solution, &report);
    SolvedRun {
        solution,
        report,
        digest,
    }
}

/// One step of a drag run: the solution, the checker's report at it and the
/// step's digest.
#[derive(Debug, Clone, PartialEq)]
pub struct DragStepRun {
    /// The solution the session returned for the step's target.
    pub solution: Solution,
    /// The checker's report at the solution's coordinates.
    pub report: Report,
    /// The digest of the step ([`of_drag_step`]).
    pub digest: Digest,
}

/// Drags a fixture's point through its targets and checks every step with
/// the independent checker. Returns one run per target, in order; `None`
/// when the fixture has no drag or the session refuses the point.
/// Comparing with the expectations is the caller's.
///
/// ```
/// use laman::solution::SolverOptions;
/// use laman_debug::fixture::Fixture;
/// use laman_debug::runner::drag_run;
///
/// let fixture = Fixture::parse(
///     r#"{"format":"laman.sketch","version":1,"entities":[{"id":0,"kind":"point","x":0.0,"y":0.0,"fixed":["x","y"]},{"id":1,"kind":"point","x":3.0,"y":4.0}],"constraints":[{"id":0,"kind":"distance","a":0,"b":1,"value":5.0}]}"#,
///     r#"{"status":"UnderConstrained","dof":1,"coordinates":[]}"#,
///     Some(r#"{"dragged":1,"targets":[[0.0,9.0]],"after":[]}"#),
/// )?;
/// let steps = drag_run(&fixture, &SolverOptions::default()).unwrap();
/// assert_eq!(steps.len(), 1);
/// assert!(steps[0].report.is_clean());
/// # Ok::<(), laman_debug::fixture::FixtureError>(())
/// ```
pub fn drag_run(fixture: &Fixture, options: &SolverOptions) -> Option<Vec<DragStepRun>> {
    let drag = fixture.drag.as_ref()?;
    let mut session = fixture.sketch.drag(drag.dragged, options).ok()?;
    let mut steps = Vec::with_capacity(drag.targets.len());
    for (k, target) in drag.targets.iter().enumerate() {
        let solution = session.to(*target);
        let report = check(&fixture.sketch, &solution.coordinates);
        let digest = of_drag_step(&fixture.sketch, k + 1, *target, &solution, &report);
        steps.push(DragStepRun {
            solution,
            report,
            digest,
        });
    }
    Some(steps)
}
