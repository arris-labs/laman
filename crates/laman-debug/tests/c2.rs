//! The generated C2 corpus: expectations come from construction, never a solve.
#![allow(clippy::unwrap_used)]

use laman::solution::{Algorithm, SolverOptions};
use laman_check::check;
use laman_debug::corpus::{Case, rect_chain, slot_grid, triangle_strip};
use laman_debug::dump;

// Semicircle sweep is dimensionless; coordinate acceptance scales with size.
const SWEEP_TOLERANCE: f64 = 1e-9;

fn assert_solution(
    name: &str,
    sketch: &laman::model::Sketch,
    solution: &laman::solution::Solution,
    expected: &laman_debug::fixture::Expected,
) {
    let report = check(sketch, &solution.coordinates);
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/inspect")
        .join(name);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("solved.txt"),
        dump::solution_text(sketch, solution),
    )
    .unwrap();
    std::fs::write(
        dir.join("solved.svg"),
        dump::svg(sketch, &solution.coordinates, &report),
    )
    .unwrap();
    assert!(report.is_clean(), "{name}: {report:?}");
    assert_eq!(
        solution.status.to_string(),
        expected.status,
        "{name}: status"
    );
    assert_eq!(solution.diagnostics.dof as u32, expected.dof, "{name}: DoF");
    assert!(
        solution.diagnostics.redundant.is_empty(),
        "{name}: redundant"
    );
    assert!(
        solution.diagnostics.conflicts.is_empty(),
        "{name}: conflicts"
    );
    assert!(solution.diagnostics.flips.is_empty(), "{name}: flips");
    for (_, entity) in sketch.entities() {
        if let laman::model::Entity::Arc { c, s, e } = entity {
            let at = |p: laman::model::Point, axis| solution.coordinates[&(p.id(), axis)];
            let sx = at(*s, laman::model::Coordinate::X) - at(*c, laman::model::Coordinate::X);
            let sy = at(*s, laman::model::Coordinate::Y) - at(*c, laman::model::Coordinate::Y);
            let ex = at(*e, laman::model::Coordinate::X) - at(*c, laman::model::Coordinate::X);
            let ey = at(*e, laman::model::Coordinate::Y) - at(*c, laman::model::Coordinate::Y);
            let sweep = libm::atan2(sx * ey - sy * ex, sx * ex + sy * ey).abs();
            assert!(
                (sweep - std::f64::consts::PI).abs() <= SWEEP_TOLERANCE,
                "{name}: arc sweep"
            );
        }
    }
    for (key, want) in &expected.coordinates {
        let got = solution.coordinates[key];
        assert!(
            (got - want).abs() <= expected.tolerance,
            "{name}: {key:?}: {got} vs {want}"
        );
    }
}

fn run_case(name: &str, case: &Case) {
    for algorithm in [Algorithm::LevenbergMarquardt, Algorithm::DogLeg] {
        let mut options = SolverOptions::default();
        options.algorithm = algorithm;
        let label = format!("{name}-{algorithm:?}");
        let initial = case.sketch.solve(&options);
        assert_solution(&label, &case.sketch, &initial, &case.expected);

        let mut edited = case.sketch.clone();
        edited.apply(&initial).unwrap();
        let (id, constraint) = edited
            .constraints()
            .find(|(id, _)| edited.constraint_tag_of(*id) == Some(case.edit.tag.as_str()))
            .map(|(id, constraint)| (id, *constraint))
            .unwrap();
        let replacement = match constraint {
            laman::model::Constraint::Distance { p, q, .. } => laman::model::Constraint::Distance {
                p,
                q,
                value: case.edit.value,
            },
            laman::model::Constraint::PointLineDistance { p, line, sense, .. } => {
                laman::model::Constraint::PointLineDistance {
                    p,
                    line,
                    sense,
                    value: case.edit.value,
                }
            }
            laman::model::Constraint::Coincident { .. }
            | laman::model::Constraint::PointOnLine { .. }
            | laman::model::Constraint::PointOnCircle { .. }
            | laman::model::Constraint::Horizontal(_)
            | laman::model::Constraint::Vertical(_)
            | laman::model::Constraint::Parallel { .. }
            | laman::model::Constraint::Perpendicular { .. }
            | laman::model::Constraint::EqualLength { .. }
            | laman::model::Constraint::EqualRadius { .. }
            | laman::model::Constraint::TangentLineCircle { .. }
            | laman::model::Constraint::TangentCircles { .. }
            | laman::model::Constraint::Midpoint { .. }
            | laman::model::Constraint::Symmetric { .. }
            | laman::model::Constraint::Fix { .. }
            | laman::model::Constraint::Radius { .. }
            | laman::model::Constraint::Angle { .. } => {
                unreachable!("corpus edit must name a distance dimension")
            }
        };
        edited.remove_constraint(id).unwrap();
        edited.add_constraint_with_id(id, replacement).unwrap();
        edited.tag_constraint(id, &case.edit.tag).unwrap();
        let solution = edited.solve(&options);
        assert_solution(
            &format!("{label}-edit"),
            &edited,
            &solution,
            &case.edit.expected,
        );
        for (key, before) in &initial.coordinates {
            if case.edit.unmoved.contains(&key.0) {
                // M5 checks geometric preservation. Exact bits remain the
                // desired assertion in regression/upstream-edit-drift (M6).
                assert!(
                    (solution.coordinates[key] - before).abs() <= case.edit.expected.tolerance,
                    "{label} edit: upstream {key:?}: {} vs {before}",
                    solution.coordinates[key]
                );
            }
        }

        if let Some(drag) = &case.drag {
            let initial = drag.sketch.solve(&options);
            assert_solution(
                &format!("{label}-drag-initial"),
                &drag.sketch,
                &initial,
                &drag.expected,
            );
            let mut session = drag.sketch.drag(drag.drag.dragged, &options).unwrap();
            assert_eq!(drag.drag.targets.len(), drag.drag.after.len());
            for (index, (target, after)) in
                drag.drag.targets.iter().zip(&drag.drag.after).enumerate()
            {
                assert_eq!(after.step, index + 1);
                let solution = session.to(*target);
                let mut expected = drag.expected.clone();
                expected.coordinates = after.coordinates.clone();
                assert_solution(
                    &format!("{label}-drag-{}", after.step),
                    &drag.sketch,
                    &solution,
                    &expected,
                );
            }
        }
    }
}

#[test]
fn rect_chain_three() {
    let case = rect_chain(3).unwrap();
    assert_eq!(case.sketch.entities().count(), 18);
    run_case("rect-chain-3", &case);
    // The edit and each drag expectation satisfy the geometry independently.
    for step in &case.drag.as_ref().unwrap().drag.after {
        assert!(check(&case.drag.as_ref().unwrap().sketch, &step.coordinates).is_clean());
    }
}

#[test]
fn slot_grid_two_by_two() {
    let case = slot_grid(2, 2).unwrap();
    assert_eq!(case.sketch.entities().count(), 48);
    // C1's local slot coordinates are (40,30), (40,35), (40,25),
    // (80,30), (80,25), (80,35), translated into each grid cell.
    for i in 0..4 {
        let mut centres = Vec::new();
        for side in ["left", "right"] {
            let tag = format!("slot-{i}-{side}");
            let (_, entity) = case
                .sketch
                .entities()
                .find(|(id, _)| case.sketch.tag_of(*id) == Some(tag.as_str()))
                .unwrap();
            assert!(matches!(entity, laman::model::Entity::Arc { .. }));
            if let laman::model::Entity::Arc { c, s, e } = entity {
                let xy = |p: laman::model::Point| {
                    [
                        case.expected.coordinates[&(p.id(), laman::model::Coordinate::X)],
                        case.expected.coordinates[&(p.id(), laman::model::Coordinate::Y)],
                    ]
                };
                let centre = xy(*c);
                let (start, end) = (xy(*s), xy(*e));
                let sign = if side == "left" { 1.0 } else { -1.0 };
                assert_eq!(start, [centre[0], centre[1] + sign * 5.0]);
                assert_eq!(end, [centre[0], centre[1] - sign * 5.0]);
                centres.push(centre);
            }
        }
        assert_eq!(centres[1], [centres[0][0] + 40.0, centres[0][1]]);
    }
    run_case("slot-grid-2-2", &case);
    for step in &case.drag.as_ref().unwrap().drag.after {
        assert!(check(&case.drag.as_ref().unwrap().sketch, &step.coordinates).is_clean());
    }
}

#[test]
fn triangle_strip_six_and_its_rigid_tail_edit() {
    let case = triangle_strip(6).unwrap();
    assert_eq!(case.sketch.entities().count(), 21);
    run_case("triangle-strip-6", &case);
    assert!(case.drag.is_none());
}
