//! The generated C2 corpus: expectations come from construction, never a solve.
#![allow(clippy::unwrap_used)]

use laman::solution::{Algorithm, SolverOptions};
use laman_check::check;
use laman_debug::corpus::{Case, rect_chain, slot_grid, triangle_strip};
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
        for (_, entity) in case.sketch.entities() {
            if let laman::model::Entity::Arc { c, s, e } = entity {
                let at = |p: laman::model::Point, axis| solution.coordinates[&(p.id(), axis)];
                let sx = at(*s, laman::model::Coordinate::X) - at(*c, laman::model::Coordinate::X);
                let sy = at(*s, laman::model::Coordinate::Y) - at(*c, laman::model::Coordinate::Y);
                let ex = at(*e, laman::model::Coordinate::X) - at(*c, laman::model::Coordinate::X);
                let ey = at(*e, laman::model::Coordinate::Y) - at(*c, laman::model::Coordinate::Y);
                let sweep = libm::atan2(sx * ey - sy * ex, sx * ex + sy * ey).abs();
                assert!((sweep - std::f64::consts::PI).abs() <= case.expected.tolerance);
            }
        }
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

#[test]
fn slot_grid_two_by_two() {
    let case = slot_grid(2, 2).unwrap();
    assert_eq!(case.sketch.entities().count(), 48);
    static_case("slot-grid-2-2", &case);
    for step in &case.drag.as_ref().unwrap().drag.after {
        assert!(check(&case.drag.as_ref().unwrap().sketch, &step.coordinates).is_clean());
    }
}

#[test]
fn triangle_strip_six_and_its_rigid_tail_edit() {
    let case = triangle_strip(6).unwrap();
    assert_eq!(case.sketch.entities().count(), 21);
    static_case("triangle-strip-6", &case);
    assert!(case.drag.is_none());
    for algorithm in [Algorithm::LevenbergMarquardt, Algorithm::DogLeg] {
        let mut edited = triangle_strip(6).unwrap();
        let mut options = SolverOptions::default();
        options.algorithm = algorithm;
        let initial = edited.sketch.solve(&options);
        assert!(check(&edited.sketch, &initial.coordinates).is_clean());
        edited.sketch.apply(&initial).unwrap();
        let (id, constraint) = edited
            .sketch
            .constraints()
            .find(|(id, _)| edited.sketch.constraint_tag_of(*id) == Some(edited.edit.tag.as_str()))
            .map(|(id, constraint)| (id, *constraint))
            .unwrap();
        assert!(matches!(
            constraint,
            laman::model::Constraint::Distance { .. }
        ));
        if let laman::model::Constraint::Distance { p, q, .. } = constraint {
            edited.sketch.remove_constraint(id).unwrap();
            edited
                .sketch
                .add_constraint_with_id(
                    id,
                    laman::model::Constraint::Distance {
                        p,
                        q,
                        value: edited.edit.value,
                    },
                )
                .unwrap();
            edited.sketch.tag_constraint(id, &edited.edit.tag).unwrap();
        }
        let solution = edited.sketch.solve(&options);
        let report = check(&edited.sketch, &solution.coordinates);
        assert!(report.is_clean());
        assert_eq!(solution.status.to_string(), edited.edit.expected.status);
        assert!(solution.diagnostics.redundant.is_empty());
        assert!(solution.diagnostics.conflicts.is_empty());
        assert!(solution.diagnostics.flips.is_empty());
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/inspect")
            .join(format!("triangle-strip-6-edit-{algorithm:?}"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("solved.txt"),
            dump::solution_text(&edited.sketch, &solution),
        )
        .unwrap();
        std::fs::write(
            dir.join("solved.svg"),
            dump::svg(&edited.sketch, &solution.coordinates, &report),
        )
        .unwrap();
        assert_eq!(solution.diagnostics.dof, 0);
        for (key, want) in &edited.edit.expected.coordinates {
            assert!(
                (solution.coordinates[key] - want).abs() <= edited.expected.tolerance,
                "{algorithm:?} edited {key:?}: {} vs {want}",
                solution.coordinates[key]
            );
        }
    }
}
