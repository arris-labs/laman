//! The generated C2 corpus: expectations come from construction, never a solve.
#![allow(clippy::unwrap_used)]

use laman::solution::{Algorithm, SolverOptions};
use laman_check::check;
use laman_debug::corpus::Case;
use laman_debug::dump;

// Semicircle sweep is dimensionless; coordinate acceptance scales with size.
const SWEEP_TOLERANCE: f64 = 1e-9;

// The whole-component analysis a staged one is compared with tests every
// constraint for redundancy by a whole factorization, O(n⁴) in the sketch's
// size: 8 s in release at 501 entities, 19 s in debug at the 98-entity
// slot grid. Up to 50 entities it fits the 10 s debug bound; the small
// cases and every property case cover each family's edit and drag.
const REFERENCE_ENTITIES: usize = 50;

/// The staged diagnosis of `solution` equals the whole-component one at the
/// same coordinates on DoF, redundant and conflicting sets (plans/m6-stages
/// step 7), for a sketch small enough to analyse whole.
fn assert_reference(
    name: &str,
    sketch: &laman::model::Sketch,
    solution: &laman::solution::Solution,
    options: &SolverOptions,
) {
    if sketch.entities().count() > REFERENCE_ENTITIES {
        return;
    }
    let whole = laman::reference::diagnose(sketch, &solution.coordinates, options);
    let (staged, whole) = (&solution.diagnostics, &whole.diagnostics);
    assert_eq!(staged.dof, whole.dof, "{name}: DoF against the reference");
    assert_eq!(
        staged.redundant, whole.redundant,
        "{name}: redundant against the reference"
    );
    assert_eq!(
        staged.conflicts, whole.conflicts,
        "{name}: conflicts against the reference"
    );
}

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

pub(crate) fn run_case(name: &str, case: &Case) {
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
        assert_reference(&format!("{label}-edit"), &edited, &solution, &options);
        // Locality (plans/m6-stages): an edit moves only its stage and the
        // parts that read it. The construction's upstream entities and every
        // parameter outside that closure keep their bits.
        let reached = laman::reference::downstream(&edited, id);
        for key in &reached {
            assert!(
                !case.edit.unmoved.contains(&key.0),
                "{label} edit: {key:?} is upstream by construction, downstream by structure"
            );
        }
        for (key, before) in &initial.coordinates {
            if case.edit.unmoved.contains(&key.0) || !reached.contains(key) {
                assert_eq!(
                    solution.coordinates[key].to_bits(),
                    before.to_bits(),
                    "{label} edit: outside the edit's reach {key:?}: {} vs {before}",
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
                assert_reference(
                    &format!("{label}-drag-{}", after.step),
                    &drag.sketch,
                    &solution,
                    &options,
                );
            }
        }
    }
}
