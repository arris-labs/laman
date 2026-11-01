//! The runner: a fixture loaded, checked at its expected coordinates and
//! digested. It is the M0 half of a fixture test; the solve and the
//! comparison with the solution join it in M2.

use crate::digest::{Digest, of_run};
use crate::fixture::Fixture;
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
