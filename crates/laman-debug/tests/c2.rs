//! The generated C2 corpus: expectations come from construction, never a solve.
#![allow(clippy::unwrap_used)]

use laman::solution::{Algorithm, SolverOptions};
use laman_check::check;
use laman_debug::corpus::{Case, rect_chain};
use laman_debug::dump;

fn static_case(name: &str, case: &Case) {
    for algorithm in [Algorithm::LevenbergMarquardt, Algorithm::DogLeg] {
        let mut options = SolverOptions::default();
        options.algorithm = algorithm;
        let solution = case.sketch.solve(&options);
        let report = check(&case.sketch, &solution.coordinates);
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/inspect")
            .join(format!("{name}-{algorithm:?}"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("solved.txt"),
            dump::solution_text(&case.sketch, &solution),
        )
        .unwrap();
        std::fs::write(
            dir.join("solved.svg"),
            dump::svg(&case.sketch, &solution.coordinates, &report),
        )
        .unwrap();
        assert!(report.is_clean(), "{name} {algorithm:?}: {report:?}");
        assert_eq!(solution.status.to_string(), case.expected.status);
        assert_eq!(solution.diagnostics.dof as u32, case.expected.dof);
        assert!(solution.diagnostics.redundant.is_empty());
        assert!(solution.diagnostics.conflicts.is_empty());
        assert!(solution.diagnostics.flips.is_empty());
        for (key, want) in &case.expected.coordinates {
            let got = solution.coordinates[key];
            assert!(
                (got - want).abs() <= case.expected.tolerance,
                "{name} {algorithm:?}: {key:?}: {got} vs {want}"
            );
        }
    }
}

#[test]
fn rect_chain_three() {
    let case = rect_chain(3).unwrap();
    assert_eq!(case.sketch.entities().count(), 18);
    static_case("rect-chain-3", &case);
    // The edit and each drag expectation satisfy the geometry independently.
    for step in &case.drag.as_ref().unwrap().drag.after {
        assert!(check(&case.drag.as_ref().unwrap().sketch, &step.coordinates).is_clean());
    }
}
