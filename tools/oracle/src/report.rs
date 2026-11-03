//! The report: one JSON document per solver and fixture, the only thing
//! `compare.py` reads. A solver's own words are kept beside the laman status
//! they map to, and a field a solver does not expose is `null`, never an
//! empty set.

use serde::{Deserialize, Serialize};

/// How the run ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    /// The solver ran and its numbers are in the report.
    Solved,
    /// A kind of the fixture has no translation; see `unsupported`.
    Unsupported,
    /// The process died (a panic, a signal); see `detail`.
    Crashed,
    /// The process outran its time limit.
    Timeout,
    /// The translation failed its residual check, or the fixture or the
    /// solver's API refused the input; see `detail`. Never numbers.
    Error,
}

/// laman's four statuses, restated here because the oracle
/// names only laman's model types.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Status {
    FullyConstrained,
    UnderConstrained,
    Unsatisfiable,
    NotConverged,
}

/// A laman kind with no translation, and how it was approximated, if it was.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Unsupported {
    /// The laman kind, as in `sketch.json` (`"symmetric"`).
    pub kind: String,
    /// The constraint's tag, or `#id` when it has none.
    pub constraint: String,
    /// What stood in for it; `None` when it was left out (which makes the
    /// report `unsupported`).
    pub approximation: Option<String>,
}

/// One solver on one fixture.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub solver: String,
    /// The pinned crate version that produced it.
    pub version: String,
    /// The fixture's directory name under `tests/fixtures/c1/`.
    pub fixture: String,
    pub outcome: Outcome,
    /// The solver's status mapped to laman's; `None` where the mapping is
    /// ambiguous (e.g. one word for conflicting and not converged).
    pub status: Option<Status>,
    /// The solver's own word for its status, kept as it said it.
    pub solver_status: Option<String>,
    /// Degrees of freedom; `None` when the solver does not report it.
    pub dof: Option<u32>,
    /// Redundant constraints by laman tag (`#id` when untagged); `None`
    /// when the solver names none.
    pub redundant: Option<Vec<String>>,
    /// Conflict sets by laman tag; a solver that names one flat list gives
    /// one set. `None` when the solver names none.
    pub conflicts: Option<Vec<Vec<String>>>,
    pub unsupported: Vec<Unsupported>,
    /// The largest residual of the solver's own equations at the fixture's
    /// expected coordinates: the proof that the translation says what the
    /// fixture says. `None` until checked.
    pub translation_residual: Option<f64>,
    /// Why, for `crashed`, `timeout` and `error`.
    pub detail: Option<String>,
}

/// The pinned version of a solver, or `None` for a name that is not one.
/// `tests::versions_match_the_manifest` holds it to `Cargo.toml`.
pub fn version(solver: &str) -> Option<&'static str> {
    match solver {
        "sketch-solver" => Some("0.1.0"),
        "acs-solver" => Some("0.1.19"),
        "ezpz" => Some("0.2.29"),
        "stub" => Some("0.0.0"),
        _ => None,
    }
}

impl Report {
    /// A report with no numbers: `outcome` and a reason.
    pub fn without_numbers(
        solver: &str,
        fixture: &str,
        outcome: Outcome,
        detail: impl Into<String>,
    ) -> Report {
        Report {
            solver: solver.to_owned(),
            version: version(solver).unwrap_or("unknown").to_owned(),
            fixture: fixture.to_owned(),
            outcome,
            status: None,
            solver_status: None,
            dof: None,
            redundant: None,
            conflicts: None,
            unsupported: Vec::new(),
            translation_residual: None,
            detail: Some(detail.into()),
        }
    }

    /// Pretty JSON with a trailing newline, the form committed under
    /// `results/`.
    pub fn to_json(&self) -> String {
        let mut s = serde_json::to_string_pretty(self).unwrap_or_default();
        s.push('\n');
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stub() -> Report {
        Report {
            solver: "stub".into(),
            version: "0.0.0".into(),
            fixture: "belt".into(),
            outcome: Outcome::Solved,
            status: Some(Status::UnderConstrained),
            solver_status: Some("converged".into()),
            dof: Some(3),
            redundant: Some(vec!["same-belt".into(), "#12".into()]),
            conflicts: Some(vec![vec!["r1".into(), "span".into()]]),
            unsupported: vec![Unsupported {
                kind: "symmetric".into(),
                constraint: "centred".into(),
                approximation: None,
            }],
            translation_residual: Some(1.5e-13),
            detail: None,
        }
    }

    #[test]
    fn a_report_round_trips() {
        let r = stub();
        let back: Report = serde_json::from_str(&r.to_json()).expect("parses");
        assert_eq!(back, r);
        // A field the solver does not expose stays null, not [].
        let none = Report::without_numbers("ezpz", "belt", Outcome::Timeout, "10 s");
        let v: serde_json::Value = serde_json::from_str(&none.to_json()).expect("parses");
        assert!(v["redundant"].is_null() && v["dof"].is_null());
        assert_eq!(v["outcome"], "timeout");
        assert_eq!(v["version"], "0.2.29");
    }

    #[test]
    fn versions_match_the_manifest() {
        let manifest = include_str!("../Cargo.toml");
        for (name, crate_name) in [
            ("sketch-solver", "sketch-solver"),
            ("acs-solver", "acs-solver"),
            ("ezpz", "ezpz"),
        ] {
            let line = format!(
                "{crate_name} = \"={}\"",
                version(name).expect("a pinned solver")
            );
            assert!(manifest.contains(&line), "Cargo.toml lacks {line}");
        }
    }
}
