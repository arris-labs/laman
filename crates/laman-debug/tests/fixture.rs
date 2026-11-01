//! The loader and the runner: a fixture parses from its files, refuses what
//! it cannot name, and a run checks it at the closed-form coordinates and
//! digests the result. The triangle is the SSS fixture of ROADMAP.md §C1,
//! built here, its expected coordinates from the 30–40–50 geometry.
#![allow(clippy::unwrap_used, clippy::expect_used)] // tests: a failing unwrap is the test failing (clippy.toml)

use laman::model::{Coordinate, EntityId, Sketch};
use laman_check::Source;
use laman_debug::fixture::{Fixture, FixtureError};
use laman_debug::json::sketch_to_string;
use laman_debug::runner::run;

/// The SSS triangle built in code: ground `o` at the origin, `a` and `b`
/// guessed off their answers, the three sides and a horizontal base.
/// Returns the sketch and the ids of `a` and `b`.
fn sss() -> (Sketch, EntityId, EntityId) {
    let mut sk = Sketch::new();
    let o = sk.point_fixed(0.0, 0.0).unwrap();
    let a = sk.point(40.0, 5.0).unwrap();
    let b = sk.point(10.0, 30.0).unwrap();
    let base = sk.line(o, a).unwrap();
    sk.line(o, b).unwrap();
    sk.line(a, b).unwrap();
    sk.horizontal(base).unwrap();
    sk.distance(o, a, 50.0).unwrap();
    sk.distance(o, b, 30.0).unwrap();
    sk.distance(a, b, 40.0).unwrap();
    sk.tag(o.id(), "origin").unwrap();
    (sk, a.id(), b.id())
}

/// The expected file for the triangle, with `a` = (50, 0) and `b` = (18, 24).
fn expected(a: EntityId, b: EntityId) -> String {
    format!(
        r#"{{
  "status": "FullyConstrained",
  "dof": 0,
  "coordinates": [
    {{"id": {a}, "coordinate": "x", "value": 50.0}},
    {{"id": {a}, "coordinate": "y", "value": 0.0}},
    {{"id": {b}, "coordinate": "x", "value": 18.0}},
    {{"id": {b}, "coordinate": "y", "value": 24.0}}
  ],
  "redundant": [],
  "conflicts": []
}}"#,
        a = a.0,
        b = b.0
    )
}

#[test]
fn the_triangle_checks_clean_at_its_closed_form_coordinates() {
    let (sk, a, b) = sss();
    let fixture = Fixture::parse(&sketch_to_string(&sk), &expected(a, b), None).unwrap();
    assert_eq!(fixture.expected.status, "FullyConstrained");
    assert_eq!(fixture.expected.dof, 0);
    assert_eq!(fixture.expected.tolerance, laman::model::tolerance::LENGTH);
    assert!(fixture.drag.is_none());

    let result = run(&fixture);
    assert!(result.report.is_clean(), "{:?}", result.report);
}

#[test]
fn a_guess_left_in_place_is_a_violation_named_by_its_constraint() {
    // The sketch's own guess for `b` is (10, 30): the expectations do not
    // move it, so the `left` and `right` distances are off.
    let (sk, a, _) = sss();
    let text = format!(
        r#"{{"status":"FullyConstrained","dof":0,"coordinates":[
            {{"id":{a},"coordinate":"x","value":50.0}},
            {{"id":{a},"coordinate":"y","value":0.0}}]}}"#,
        a = a.0
    );
    let fixture = Fixture::parse(&sketch_to_string(&sk), &text, None).unwrap();
    let result = run(&fixture);
    assert!(!result.report.is_clean());
    assert!(
        result
            .report
            .violations
            .iter()
            .all(|v| matches!(v.id, Source::Constraint(_)))
    );
}

#[test]
fn the_digest_is_the_same_on_every_run_and_moves_with_the_report() {
    let (sk, a, b) = sss();
    let good = Fixture::parse(&sketch_to_string(&sk), &expected(a, b), None).unwrap();
    let again = Fixture::parse(&sketch_to_string(&sk), &expected(a, b), None).unwrap();
    assert_eq!(run(&good).digest, run(&again).digest);

    let off = format!(
        r#"{{"status":"FullyConstrained","dof":0,"coordinates":[
            {{"id":{a},"coordinate":"x","value":50.0}},
            {{"id":{a},"coordinate":"y","value":0.0}},
            {{"id":{b},"coordinate":"x","value":18.5}},
            {{"id":{b},"coordinate":"y","value":24.0}}]}}"#,
        a = a.0,
        b = b.0
    );
    let bad = Fixture::parse(&sketch_to_string(&sk), &off, None).unwrap();
    assert_ne!(run(&good).digest, run(&bad).digest);
}

#[test]
fn a_coordinate_for_an_unknown_entity_is_refused_naming_its_id() {
    let (sk, a, _) = sss();
    let text = format!(
        r#"{{"status":"FullyConstrained","dof":0,"coordinates":[{{"id":{id},"coordinate":"x","value":1.0}}]}}"#,
        id = 999
    );
    let err = Fixture::parse(&sketch_to_string(&sk), &text, None).unwrap_err();
    assert!(
        matches!(err, FixtureError::UnknownCoordinate { file: "expected.json", id, coordinate: Coordinate::X } if id.0 == 999),
        "{err:?}"
    );
    let _ = a;
}

#[test]
fn a_parameter_the_entity_does_not_have_is_refused() {
    // A point has x and y, not r.
    let (sk, a, _) = sss();
    let text = format!(
        r#"{{"status":"FullyConstrained","dof":0,"coordinates":[{{"id":{a},"coordinate":"r","value":1.0}}]}}"#,
        a = a.0
    );
    let err = Fixture::parse(&sketch_to_string(&sk), &text, None).unwrap_err();
    assert!(matches!(
        err,
        FixtureError::UnknownCoordinate {
            coordinate: Coordinate::R,
            ..
        }
    ));
}

#[test]
fn an_unknown_key_is_refused_with_its_file_named() {
    let (sk, a, b) = sss();
    let text = expected(a, b).replace("\"dof\"", "\"dofs\"");
    let err = Fixture::parse(&sketch_to_string(&sk), &text, None).unwrap_err();
    assert!(
        matches!(
            err,
            FixtureError::Json {
                file: "expected.json",
                ..
            }
        ),
        "{err:?}"
    );
}

#[test]
fn a_bad_tolerance_is_refused() {
    let (sk, a, b) = sss();
    for bad in ["0.0", "-1e-9"] {
        let text = expected(a, b).replace(
            "\"redundant\"",
            &format!("\"tolerance\": {bad}, \"redundant\""),
        );
        let err = Fixture::parse(&sketch_to_string(&sk), &text, None).unwrap_err();
        assert!(
            matches!(err, FixtureError::Tolerance { .. }),
            "{bad}: {err:?}"
        );
    }
}

#[test]
fn a_sketch_the_model_refuses_is_named_by_its_file() {
    let err = Fixture::parse(
        r#"{"format":"laman.sketch","version":2,"entities":[]}"#,
        r#"{"status":"FullyConstrained","dof":0,"coordinates":[]}"#,
        None,
    )
    .unwrap_err();
    assert!(
        matches!(
            err,
            FixtureError::Sketch {
                file: "sketch.json",
                ..
            }
        ),
        "{err:?}"
    );
}

#[test]
fn a_drag_names_a_point_and_its_steps_in_range() {
    let (sk, a, _) = sss();
    let line = sk
        .entities()
        .find(|(_, e)| e.kind() == laman::model::EntityKind::Line)
        .unwrap()
        .0;
    let exp = r#"{"status":"FullyConstrained","dof":0,"coordinates":[]}"#;
    let drag = |dragged: u32, step: usize| {
        format!(
            r#"{{"dragged":{dragged},"targets":[[1.0,2.0],[3.0,4.0]],"after":[{{"step":{step},"coordinates":[{{"id":{a},"coordinate":"x","value":1.0}}]}}]}}"#,
            a = a.0
        )
    };
    let sketch = sketch_to_string(&sk);

    let ok = Fixture::parse(&sketch, exp, Some(&drag(a.0, 2))).unwrap();
    let d = ok.drag.unwrap();
    assert_eq!(d.dragged, a);
    assert_eq!(d.targets, vec![[1.0, 2.0], [3.0, 4.0]]);
    assert_eq!(d.after[0].step, 2);

    let err = Fixture::parse(&sketch, exp, Some(&drag(line.0, 1))).unwrap_err();
    assert!(
        matches!(err, FixtureError::NotAPoint { id } if id == line),
        "{err:?}"
    );

    for step in [0, 3] {
        let err = Fixture::parse(&sketch, exp, Some(&drag(a.0, step))).unwrap_err();
        assert!(
            matches!(err, FixtureError::StepOutOfRange { targets: 2, .. }),
            "{err:?}"
        );
    }
}

#[test]
fn a_fixture_directory_loads_and_a_missing_drag_file_is_no_drag() {
    let (sk, a, b) = sss();
    let dir = std::env::temp_dir().join(format!("laman-debug-fixture-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("sketch.json"), sketch_to_string(&sk)).unwrap();
    std::fs::write(dir.join("expected.json"), expected(a, b)).unwrap();

    let loaded = Fixture::load(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    let fixture = loaded.unwrap();
    assert!(fixture.drag.is_none());
    assert!(run(&fixture).report.is_clean());
}

#[test]
fn a_missing_directory_names_the_path_it_could_not_read() {
    let dir = std::env::temp_dir().join("laman-debug-no-such-fixture-dir");
    let err = Fixture::load(&dir).unwrap_err();
    assert!(
        matches!(&err, FixtureError::Read { path, .. } if path.ends_with("sketch.json")),
        "{err:?}"
    );
}
