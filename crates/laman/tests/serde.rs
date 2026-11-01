//! The serde form: its shape, and one test per malformed file, each naming
//! the id of the entity or constraint it is wrong at.
#![allow(clippy::unwrap_used)] // tests: a failing unwrap is the test failing (clippy.toml)

use laman::model::{
    Constraint, ConstraintId, Contact, EntityId, EntityKind, ModelError, Payload, Sense, Sketch,
    SketchForm,
};
use serde_json::{Value, json};

/// A file with the given members, `format` and `version` filled in.
fn file(entities: Value, constraints: Value) -> Value {
    json!({
        "format": "laman.sketch", "version": 1,
        "entities": entities, "constraints": constraints,
    })
}

fn load(v: Value) -> Result<Sketch, ModelError> {
    Sketch::from_form(serde_json::from_value::<SketchForm>(v).unwrap())
}

fn two_points() -> Value {
    json!([
        {"id": 0, "kind": "point", "x": 0.0, "y": 0.0},
        {"id": 1, "kind": "point", "x": 3.0, "y": 4.0},
    ])
}

fn at_entity(id: u32, source: ModelError) -> ModelError {
    ModelError::Entity {
        id: EntityId(id),
        source: Box::new(source),
    }
}

fn at_constraint(id: u32, source: ModelError) -> ModelError {
    ModelError::Constraint {
        id: ConstraintId(id),
        source: Box::new(source),
    }
}

#[test]
fn the_form_omits_defaults_and_puts_the_id_first() {
    let mut sk = Sketch::new();
    let a = sk.point_fixed(0.0, 0.0).unwrap();
    let b = sk.point(40.0, 5.0).unwrap();
    sk.tag(a.id(), "origin").unwrap();
    let l = sk.line(a, b).unwrap();
    sk.payload(l.id(), Payload(vec![1, 2, 3])).unwrap();
    let h = sk.horizontal(l).unwrap();
    sk.tag_constraint(h, "level").unwrap();
    let text = serde_json::to_string(&sk).unwrap();
    assert_eq!(
        text,
        concat!(
            r#"{"format":"laman.sketch","version":1,"entities":["#,
            r#"{"id":0,"kind":"point","x":0.0,"y":0.0,"fixed":["x","y"],"tag":"origin"},"#,
            r#"{"id":1,"kind":"point","x":40.0,"y":5.0},"#,
            r#"{"id":2,"kind":"line","a":0,"b":1,"payload":"AQID"}],"#,
            r#""constraints":[{"id":0,"kind":"horizontal","line":2,"tag":"level"}]}"#
        )
    );
}

#[test]
fn a_sketch_reloads_to_the_same_description() {
    let mut sk = Sketch::new();
    let a = sk.point(0.0, 0.0).unwrap();
    let b = sk.point(40.0, 5.0).unwrap();
    let l = sk.line(a, b).unwrap();
    sk.set_construction(l.id(), true).unwrap();
    let c = sk.circle(b, 10.0).unwrap();
    sk.set_fixed(c.id(), laman::model::Coordinate::R, true)
        .unwrap();
    sk.tangent_line_circle(l, c).unwrap();
    let back: Sketch = serde_json::from_str(&serde_json::to_string(&sk).unwrap()).unwrap();
    assert_eq!(back.to_form(), sk.to_form());
    assert!(back.is_construction(l.id()));
    assert_eq!(back.coordinates(), sk.coordinates());
}

#[test]
fn entities_are_written_after_the_points_they_use() {
    let mut sk = Sketch::new();
    let b = sk.point_with_id(EntityId(5), 1.0, 0.0).unwrap();
    let a = sk.point_with_id(EntityId(6), 0.0, 0.0).unwrap();
    sk.line_with_id(EntityId(1), a, b).unwrap();
    let ids: Vec<u32> = serde_json::to_value(&sk).unwrap()["entities"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| u32::try_from(e["id"].as_u64().unwrap()).unwrap())
        .collect();
    assert_eq!(ids, [6, 5, 1]);
    let back: Sketch = serde_json::from_value(serde_json::to_value(&sk).unwrap()).unwrap();
    assert_eq!(back.entities().count(), 3);
}

#[test]
fn a_missing_sense_is_taken_from_the_drawing_and_a_stored_one_is_kept() {
    let ents = json!([
        {"id": 0, "kind": "point", "x": 0.0, "y": 0.0},
        {"id": 1, "kind": "point", "x": 10.0, "y": 0.0},
        {"id": 2, "kind": "line", "a": 0, "b": 1},
        {"id": 3, "kind": "point", "x": 5.0, "y": -2.0},
        {"id": 4, "kind": "circle", "c": 3, "r": 2.0},
    ]);
    let cons = json!([
        {"id": 0, "kind": "tangent", "line": 2, "circle": 4},
        {"id": 1, "kind": "tangent", "line": 2, "circle": 4, "sense": 1},
    ]);
    let sk = load(file(ents, cons)).unwrap();
    let sense = |id| match sk.constraint(ConstraintId(id)) {
        Some(Constraint::TangentLineCircle { sense, .. }) => *sense,
        other => panic!("not a line-circle tangent: {other:?}"),
    };
    assert_eq!(sense(0), Sense::Negative);
    assert_eq!(sense(1), Sense::Positive);
}

#[test]
fn circle_contact_is_stored_and_computed() {
    let ents = json!([
        {"id": 0, "kind": "point", "x": 0.0, "y": 0.0},
        {"id": 1, "kind": "point", "x": 3.0, "y": 0.0},
        {"id": 2, "kind": "circle", "c": 0, "r": 5.0},
        {"id": 3, "kind": "circle", "c": 1, "r": 2.0},
    ]);
    let cons = json!([
        {"id": 0, "kind": "tangent", "a": 2, "b": 3},
        {"id": 1, "kind": "tangent", "a": 2, "b": 3, "contact": "external"},
        {"id": 2, "kind": "tangent", "a": 2, "b": 3, "contact": "internal"},
        {"id": 3, "kind": "tangent", "a": 3, "b": 2, "contact": "internal", "sense": 1},
    ]);
    let sk = load(file(ents, cons)).unwrap();
    let contact = |id| match sk.constraint(ConstraintId(id)) {
        Some(Constraint::TangentCircles { contact, .. }) => *contact,
        other => panic!("not a circle tangent: {other:?}"),
    };
    assert_eq!(contact(0), Contact::Internal(Sense::Positive));
    assert_eq!(contact(1), Contact::External);
    assert_eq!(contact(2), Contact::Internal(Sense::Positive));
    assert_eq!(contact(3), Contact::Internal(Sense::Positive));
    // And the stored form reloads to itself.
    assert_eq!(
        Sketch::from_form(sk.to_form()).unwrap().to_form(),
        sk.to_form()
    );
}

#[test]
fn a_wrong_format_is_refused() {
    let mut v = file(json!([]), json!([]));
    v["format"] = json!("laman.solution");
    assert_eq!(
        load(v).err(),
        Some(ModelError::WrongFormat {
            found: "laman.solution".into()
        })
    );
}

#[test]
fn a_newer_or_zero_version_is_refused() {
    for found in [0_u64, 2, 99] {
        let mut v = file(json!([]), json!([]));
        v["version"] = json!(found);
        assert_eq!(
            load(v).err(),
            Some(ModelError::FormatVersion {
                found,
                supported: 1
            })
        );
    }
}

#[test]
fn a_line_to_an_unknown_point_names_both_ids() {
    let ents = json!([
        {"id": 0, "kind": "point", "x": 0.0, "y": 0.0},
        {"id": 2, "kind": "line", "a": 0, "b": 9},
    ]);
    assert_eq!(
        load(file(ents, json!([]))).err(),
        Some(at_entity(2, ModelError::UnknownId { id: EntityId(9) }))
    );
}

#[test]
fn a_reference_of_the_wrong_kind_is_named() {
    let ents = json!([
        {"id": 0, "kind": "point", "x": 0.0, "y": 0.0},
        {"id": 1, "kind": "point", "x": 1.0, "y": 0.0},
        {"id": 2, "kind": "line", "a": 0, "b": 1},
        {"id": 3, "kind": "circle", "c": 2, "r": 1.0},
    ]);
    assert_eq!(
        load(file(ents, json!([]))).err(),
        Some(at_entity(
            3,
            ModelError::WrongKind {
                id: EntityId(2),
                expected: EntityKind::Point,
                found: EntityKind::Line
            }
        ))
    );
}

#[test]
fn a_repeated_entity_id_is_named() {
    let ents = json!([
        {"id": 4, "kind": "point", "x": 0.0, "y": 0.0},
        {"id": 4, "kind": "point", "x": 1.0, "y": 0.0},
    ]);
    assert_eq!(
        load(file(ents, json!([]))).err(),
        Some(at_entity(4, ModelError::DuplicateId { id: EntityId(4) }))
    );
}

#[test]
fn a_line_from_a_point_to_itself_is_named() {
    let ents = json!([
        {"id": 0, "kind": "point", "x": 0.0, "y": 0.0},
        {"id": 1, "kind": "line", "a": 0, "b": 0},
    ]);
    assert_eq!(
        load(file(ents, json!([]))).err(),
        Some(at_entity(
            1,
            ModelError::RepeatedEntity {
                id: EntityId(0),
                kind: EntityKind::Line
            }
        ))
    );
}

#[test]
fn a_negative_radius_is_named() {
    let ents = json!([
        {"id": 0, "kind": "point", "x": 0.0, "y": 0.0},
        {"id": 1, "kind": "circle", "c": 0, "r": -1.0},
    ]);
    assert!(matches!(
        load(file(ents, json!([]))).err(),
        Some(ModelError::Entity { id: EntityId(1), source })
            if matches!(*source, ModelError::DimensionOutOfRange { what: "radius", .. })
    ));
}

#[test]
fn fixing_a_parameter_the_entity_lacks_is_named() {
    let ents = json!([
        {"id": 0, "kind": "point", "x": 0.0, "y": 0.0, "fixed": ["r"]},
    ]);
    assert!(matches!(
        load(file(ents, json!([]))).err(),
        Some(ModelError::Entity { id: EntityId(0), source })
            if matches!(*source, ModelError::NoSuchParameter { .. })
    ));
}

#[test]
fn a_payload_that_is_not_base64_is_named() {
    let ents = json!([
        {"id": 0, "kind": "point", "x": 0.0, "y": 0.0, "payload": "not base64!"},
    ]);
    assert_eq!(
        load(file(ents, json!([]))).err(),
        Some(at_entity(0, ModelError::InvalidPayload))
    );
}

#[test]
fn a_constraint_on_an_unknown_entity_names_both_ids() {
    let cons = json!([{"id": 3, "kind": "coincident", "a": 0, "b": 7}]);
    assert_eq!(
        load(file(two_points(), cons)).err(),
        Some(at_constraint(3, ModelError::UnknownId { id: EntityId(7) }))
    );
}

#[test]
fn a_constraint_on_the_wrong_kind_is_named() {
    let cons = json!([{"id": 1, "kind": "point_on_line", "point": 0, "line": 1}]);
    assert_eq!(
        load(file(two_points(), cons)).err(),
        Some(at_constraint(
            1,
            ModelError::WrongKind {
                id: EntityId(1),
                expected: EntityKind::Line,
                found: EntityKind::Point
            }
        ))
    );
}

#[test]
fn a_constraint_naming_one_entity_twice_is_named() {
    let cons = json!([{"id": 0, "kind": "coincident", "a": 1, "b": 1}]);
    assert_eq!(
        load(file(two_points(), cons)).err(),
        Some(at_constraint(
            0,
            ModelError::RepeatedEntity {
                id: EntityId(1),
                kind: EntityKind::Point
            }
        ))
    );
}

#[test]
fn a_repeated_constraint_id_is_named() {
    let cons = json!([
        {"id": 2, "kind": "coincident", "a": 0, "b": 1},
        {"id": 2, "kind": "distance", "a": 0, "b": 1, "value": 5.0},
    ]);
    assert_eq!(
        load(file(two_points(), cons)).err(),
        Some(at_constraint(
            2,
            ModelError::DuplicateConstraintId {
                id: ConstraintId(2)
            }
        ))
    );
}

#[test]
fn a_distance_of_zero_names_coincident() {
    let cons = json!([{"id": 5, "kind": "distance", "a": 0, "b": 1, "value": 0.0}]);
    match load(file(two_points(), cons)).err() {
        Some(ModelError::Constraint {
            id: ConstraintId(5),
            source,
        }) => assert!(matches!(
            *source,
            ModelError::DimensionOutOfRange { what: "distance", wanted, .. }
                if wanted.contains("coincident")
        )),
        other => panic!("{other:?}"),
    }
}

#[test]
fn an_angle_out_of_range_is_named() {
    let ents = json!([
        {"id": 0, "kind": "point", "x": 0.0, "y": 0.0},
        {"id": 1, "kind": "point", "x": 1.0, "y": 0.0},
        {"id": 2, "kind": "point", "x": 0.0, "y": 1.0},
        {"id": 3, "kind": "line", "a": 0, "b": 1},
        {"id": 4, "kind": "line", "a": 0, "b": 2},
    ]);
    let cons = json!([{"id": 0, "kind": "angle", "a": 3, "b": 4, "value": 4.0}]);
    assert!(matches!(
        load(file(ents, cons)).err(),
        Some(ModelError::Constraint { id: ConstraintId(0), source })
            if matches!(*source, ModelError::DimensionOutOfRange { what: "angle", .. })
    ));
}

#[test]
fn a_sense_that_is_not_a_sign_is_named() {
    let ents = json!([
        {"id": 0, "kind": "point", "x": 0.0, "y": 0.0},
        {"id": 1, "kind": "point", "x": 1.0, "y": 0.0},
        {"id": 2, "kind": "line", "a": 0, "b": 1},
        {"id": 3, "kind": "point", "x": 0.5, "y": 2.0},
    ]);
    let cons = json!([
        {"id": 8, "kind": "point_line_distance", "point": 3, "line": 2, "value": 2.0, "sense": 2},
    ]);
    assert!(matches!(
        load(file(ents, cons)).err(),
        Some(ModelError::Constraint { id: ConstraintId(8), source })
            if matches!(*source, ModelError::DimensionOutOfRange { what: "sense", .. })
    ));
}

#[test]
fn deserialising_a_sketch_carries_the_error_message() {
    let ents = json!([
        {"id": 0, "kind": "point", "x": 0.0, "y": 0.0},
        {"id": 2, "kind": "line", "a": 0, "b": 9},
    ]);
    let err = serde_json::from_value::<Sketch>(file(ents, json!([]))).unwrap_err();
    assert!(err.to_string().contains("entity 2"), "{err}");
    assert!(err.to_string().contains("entity 9"), "{err}");
}
