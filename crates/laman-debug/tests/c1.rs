//! The C1 fixtures of ROADMAP.md §C1 acceptance, under `tests/fixtures/c1/`:
//! each base and redundant twin checks clean at its expected coordinates,
//! each conflicting twin flags exactly the constraint it added, and every
//! `sketch.json` round-trips byte for byte. The solve-and-compare tests are
//! written against the runner with the solve stubbed to return the guesses,
//! and wait for the solver (M2).
#![allow(clippy::unwrap_used, clippy::expect_used)] // tests: a failing unwrap is the test failing (clippy.toml)

use laman::model::{ConstraintId, Coordinate, Coordinates, Entity, EntityId, Sketch, tolerance};
use laman::residual::{Unit, equations, intrinsic};
use laman_check::{Source, check};
use laman_debug::fixture::{Fixture, SKETCH_FILE};
use laman_debug::json::{sketch_from_str, sketch_to_string};
use laman_debug::runner::run;
use std::collections::BTreeSet;
use std::path::PathBuf;

fn dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/c1")
        .join(name)
}

fn load(name: &str) -> Fixture {
    Fixture::load(&dir(name)).unwrap()
}

/// The tags of the violations a fixture shows at its expected coordinates.
fn flagged(name: &str) -> Vec<String> {
    let result = run(&load(name));
    assert!(result.report.degenerate.is_empty(), "{name}: degenerate");
    assert!(result.report.missing.is_empty(), "{name}: missing");
    result
        .report
        .violations
        .iter()
        .map(|v| v.tag.clone().unwrap_or_default())
        .collect()
}

/// The solve, stubbed until M2: it returns the guesses.
fn solve(sketch: &Sketch) -> Coordinates {
    sketch.coordinates()
}

const FIXTURES: [&str; 10] = [
    "sss-triangle",
    "sss-triangle-redundant",
    "sss-triangle-conflicting",
    "belt",
    "belt-redundant",
    "belt-conflicting",
    "slotted-plate",
    "slotted-plate-redundant",
    "slotted-plate-conflicting",
    "slotted-plate-drag",
];

#[test]
fn every_sketch_file_round_trips_byte_for_byte() {
    for name in FIXTURES {
        let text = std::fs::read_to_string(dir(name).join(SKETCH_FILE)).unwrap();
        let back = sketch_to_string(&sketch_from_str(&text).unwrap());
        assert_eq!(back, text, "{name}");
    }
}

#[test]
fn the_bases_and_redundant_twins_check_clean_at_their_expected_coordinates() {
    for name in [
        "sss-triangle",
        "sss-triangle-redundant",
        "belt",
        "belt-redundant",
        "slotted-plate",
        "slotted-plate-redundant",
        "slotted-plate-drag",
    ] {
        let result = run(&load(name));
        assert!(result.report.is_clean(), "{name}: {:?}", result.report);
    }
}

/// The tags the sketch's constraints carry, in constraint order.
fn tags(fixture: &Fixture) -> Vec<&str> {
    let sketch = &fixture.sketch;
    sketch
        .constraints()
        .filter_map(|(id, _)| sketch.constraint_tag_of(id))
        .collect()
}

#[test]
fn the_redundant_twins_name_what_they_added() {
    // The loader refuses a tag no constraint carries; here each name is also
    // the one constraint the twin has beyond its base.
    for (twin, base, added) in [
        ("sss-triangle-redundant", "sss-triangle", "right-angle"),
        ("belt-redundant", "belt", "same-belt"),
        ("slotted-plate-redundant", "slotted-plate", "level"),
    ] {
        let (twin, base) = (load(twin), load(base));
        assert_eq!(twin.expected.redundant, [added]);
        assert!(base.expected.redundant.is_empty());
        let (twin_tags, base_tags) = (tags(&twin), tags(&base));
        assert_eq!(twin_tags.iter().filter(|t| **t == added).count(), 1);
        assert!(!base_tags.contains(&added));
        assert_eq!(
            twin.sketch.constraints().count(),
            base.sketch.constraints().count() + 1
        );
    }
}

#[test]
fn the_slotted_plate_conflicting_twin_flags_only_slot_r2() {
    assert_eq!(flagged("slotted-plate-conflicting"), ["slot-r2"]);
    assert_eq!(
        load("slotted-plate-conflicting").expected.conflicts,
        [["same-ends", "slot-r", "slot-r2"]]
    );
}

#[test]
fn the_drag_fixture_is_the_plate_without_width_and_a_stray_point() {
    let drag = load("slotted-plate-drag");
    assert_eq!(drag.expected.status, "UnderConstrained");
    assert_eq!(drag.expected.dof, 3);
    let plate = load("slotted-plate");
    let tags = |f: &Fixture| -> Vec<String> {
        f.sketch
            .constraints()
            .filter_map(|(id, _)| f.sketch.constraint_tag_of(id).map(String::from))
            .collect()
    };
    assert!(tags(&plate).contains(&"width".to_string()));
    assert!(!tags(&drag).contains(&"width".to_string()));
    assert_eq!(
        drag.sketch.entities().count(),
        plate.sketch.entities().count() + 1
    );
}

/// Every one of the drag's ten closed-form steps is clean under the
/// checker: the sketch at the expected coordinates, the step's overriding
/// them, and the stray point where it was.
#[test]
fn the_checker_passes_each_closed_form_step_of_the_drag() {
    let fixture = load("slotted-plate-drag");
    let drag = fixture.drag.as_ref().unwrap();
    assert_eq!(drag.targets.len(), 10);
    assert_eq!(drag.after.len(), 10);
    for (i, step) in drag.after.iter().enumerate() {
        assert_eq!(step.step, i + 1);
        let mut values = fixture.sketch.coordinates();
        for (key, value) in &step.coordinates {
            values.insert(*key, *value);
        }
        let report = check(&fixture.sketch, &values);
        assert!(report.is_clean(), "step {}: {report:?}", step.step);
    }
    let last = &drag.after[9].coordinates;
    let at = |name: u32, c: Coordinate| last[&(EntityId(name), c)];
    let p2 = drag.dragged.0;
    assert_eq!(at(p2, Coordinate::X), 150.0);
    assert_eq!(at(p2, Coordinate::Y), 60.0);
}

#[test]
fn the_sss_conflicting_twin_flags_only_apex() {
    assert_eq!(flagged("sss-triangle-conflicting"), ["apex"]);
}

#[test]
fn the_belt_conflicting_twin_flags_only_top_length() {
    assert_eq!(flagged("belt-conflicting"), ["top-length"]);
}

#[test]
fn the_conflict_sets_are_the_ones_the_roadmap_names() {
    assert_eq!(
        load("sss-triangle-conflicting").expected.conflicts,
        [["base", "right", "apex"]]
    );
    let belt = &load("belt-conflicting").expected.conflicts;
    assert_eq!(belt.len(), 1);
    assert_eq!(
        belt[0],
        [
            "r1",
            "r2",
            "span",
            "top-on-c1",
            "top-on-c2",
            "top-tangent-c1",
            "top-tangent-c2",
            "top-length"
        ]
    );
}

/// The values a fixture is checked at: its sketch's, each expected
/// coordinate replacing its guess, as `laman_debug::runner::run` does.
fn expected_values(fixture: &Fixture) -> Coordinates {
    let mut values = fixture.sketch.coordinates();
    for (key, value) in &fixture.expected.coordinates {
        values.insert(*key, *value);
    }
    values
}

/// The tolerance an equation of `unit` is held to (`laman::model::tolerance`).
fn tolerance_of(unit: Unit) -> f64 {
    match unit {
        Unit::Length => tolerance::LENGTH,
        Unit::Ratio => tolerance::ANGLE,
    }
}

/// The constraints with an equation outside its unit's tolerance at
/// `values`, by the residual's own equations (not the checker's).
fn residual_off(sketch: &Sketch, values: &Coordinates) -> BTreeSet<ConstraintId> {
    let mut off = BTreeSet::new();
    for (id, _) in sketch.constraints() {
        let eqs = equations(sketch, values, id).unwrap();
        assert!(!eqs.is_empty(), "{id:?} has no equations");
        for eq in eqs.as_slice() {
            assert!(
                !eq.is_degenerate(),
                "{id:?} is degenerate at the expected coordinates"
            );
            if eq.value().abs() > tolerance_of(eq.unit()) {
                off.insert(id);
            }
        }
    }
    off
}

/// At every fixture's expected coordinates the residual's equations and the
/// checker agree constraint by constraint: a constraint has an equation out
/// of tolerance exactly when the checker reports it. Every arc's intrinsic
/// equation holds, and the checker reports no arc.
#[test]
fn the_residual_agrees_with_the_checker_at_every_fixture() {
    for name in FIXTURES {
        let fixture = load(name);
        let sketch = &fixture.sketch;
        let values = expected_values(&fixture);
        let report = check(sketch, &values);
        assert!(report.degenerate.is_empty(), "{name}: degenerate");
        assert!(report.missing.is_empty(), "{name}: missing");

        let checker: BTreeSet<ConstraintId> = report
            .violations
            .iter()
            .filter_map(|v| match v.id {
                Source::Constraint(id) => Some(id),
                Source::Arc(_) => None,
            })
            .collect();
        assert!(
            report
                .violations
                .iter()
                .all(|v| matches!(v.id, Source::Constraint(_))),
            "{name}: the checker reports an arc"
        );
        assert_eq!(residual_off(sketch, &values), checker, "{name}");

        for (id, entity) in sketch.entities() {
            if let Entity::Arc { .. } = entity {
                let eq = intrinsic(sketch, &values, id).unwrap();
                assert!(!eq.is_degenerate(), "{name}: {id:?} is degenerate");
                assert!(
                    eq.value().abs() <= tolerance_of(eq.unit()),
                    "{name}: {id:?} intrinsic {}",
                    eq.value()
                );
            }
        }
    }
}

/// In each conflicting twin the one constraint it added, and only it, is out
/// of tolerance at the base's expected coordinates.
#[test]
fn each_conflicting_twin_puts_off_only_the_constraint_it_added() {
    for (name, added) in [
        ("sss-triangle-conflicting", "apex"),
        ("belt-conflicting", "top-length"),
        ("slotted-plate-conflicting", "slot-r2"),
    ] {
        let fixture = load(name);
        let off: Vec<&str> = residual_off(&fixture.sketch, &expected_values(&fixture))
            .into_iter()
            .map(|id| fixture.sketch.constraint_tag_of(id).unwrap_or(""))
            .collect();
        assert_eq!(off, [added], "{name}");
    }
}

/// The solve from the guesses lands on the expected coordinates (checked
/// there: the answer is clean). Runs when `solve` is the real one.
#[test]
#[ignore = "solver: M2"]
fn the_solve_reaches_the_expected_coordinates() {
    for name in [
        "sss-triangle",
        "sss-triangle-redundant",
        "belt",
        "belt-redundant",
        "slotted-plate",
        "slotted-plate-redundant",
    ] {
        let fixture = load(name);
        let solved = solve(&fixture.sketch);
        for (key, want) in &fixture.expected.coordinates {
            let got = solved.get(key).copied().unwrap();
            assert!(
                (got - want).abs() <= fixture.expected.tolerance,
                "{name}: {key:?}: {got} vs {want}"
            );
        }
    }
}

/// The drag, with the stub solve returning the guesses: each step's
/// coordinates compare with the solution after pulling the point to its
/// target. Runs when the real drag session exists.
#[test]
#[ignore = "drag: M3"]
fn the_drag_follows_the_closed_form_steps() {
    let fixture = load("slotted-plate-drag");
    let drag = fixture.drag.as_ref().unwrap();
    for step in &drag.after {
        let solved = solve(&fixture.sketch);
        for (key, want) in &step.coordinates {
            let got = solved.get(key).copied().unwrap();
            assert!(
                (got - want).abs() <= fixture.expected.tolerance,
                "step {}: {key:?}: {got} vs {want}",
                step.step
            );
        }
    }
}
