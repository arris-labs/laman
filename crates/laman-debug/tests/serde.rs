//! The serde form round-trips byte for byte over random sketches, and a
//! newer file is refused whatever its shape.
#![allow(clippy::unwrap_used)] // tests: a failing unwrap is the test failing (clippy.toml)

use laman::model::{EntityKind, ModelError};
use laman_debug::json::{LoadError, sketch_from_str, sketch_to_string};
use laman_debug::strategy::{has_kind, sketch};
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn a_sketch_round_trips_byte_for_byte(sk in sketch(40)) {
        let text = sketch_to_string(&sk);
        let back = sketch_from_str(&text).unwrap();
        prop_assert_eq!(sketch_to_string(&back), text);
        prop_assert_eq!(back.coordinates(), sk.coordinates());
        prop_assert_eq!(
            back.constraints().map(|(i, c)| (i, *c)).collect::<Vec<_>>(),
            sk.constraints().map(|(i, c)| (i, *c)).collect::<Vec<_>>()
        );
        for (id, entity) in sk.entities() {
            prop_assert_eq!(back.entity(id), Some(entity));
            prop_assert_eq!(back.tag_of(id), sk.tag_of(id));
            prop_assert_eq!(back.payload_of(id), sk.payload_of(id));
            prop_assert_eq!(back.is_construction(id), sk.is_construction(id));
        }
    }
}

#[test]
fn the_strategy_reaches_every_entity_kind_and_most_constraints() {
    use proptest::strategy::{Strategy, ValueTree};
    use proptest::test_runner::TestRunner;
    let mut runner = TestRunner::deterministic();
    let mut kinds = std::collections::BTreeSet::new();
    let mut entities = std::collections::BTreeSet::new();
    for _ in 0..400 {
        let sk = sketch(40).new_tree(&mut runner).unwrap().current();
        kinds.extend(sk.constraints().map(|(_, c)| c.kind()));
        for k in [
            EntityKind::Point,
            EntityKind::Line,
            EntityKind::Circle,
            EntityKind::Arc,
        ] {
            if has_kind(&sk, k) {
                entities.insert(k);
            }
        }
    }
    assert_eq!(entities.len(), 4);
    assert!(kinds.len() >= 16, "only reached {kinds:?}");
}

#[test]
fn a_newer_file_is_refused_even_when_its_shape_changed() {
    let text = r#"{"format":"laman.sketch","version":2,"entities":{"not":"a list"}}"#;
    match sketch_from_str(text) {
        Err(LoadError::Model(ModelError::FormatVersion {
            found: 2,
            supported: 1,
        })) => {}
        other => panic!("{other:?}"),
    }
}

#[test]
fn text_that_is_not_a_sketch_is_a_json_error() {
    assert!(matches!(sketch_from_str("{"), Err(LoadError::Json(_))));
    assert!(matches!(sketch_from_str("[]"), Err(LoadError::Json(_))));
}

#[test]
fn a_malformed_sketch_is_a_model_error_naming_the_id() {
    let text =
        r#"{"format":"laman.sketch","version":1,"entities":[{"id":3,"kind":"line","a":0,"b":1}]}"#;
    match sketch_from_str(text) {
        Err(LoadError::Model(ModelError::Entity { id, .. })) => assert_eq!(id.0, 3),
        other => panic!("{other:?}"),
    }
}
