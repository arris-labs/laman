//! Entity-level behaviour of `laman::model`: one test per refusal.
#![allow(clippy::unwrap_used)] // tests: a failing unwrap is the test failing (clippy.toml)

use laman::model::{Coordinate, Entity, EntityId, EntityKind, ModelError, Payload, Sketch};

fn two_points() -> (Sketch, laman::model::Point, laman::model::Point) {
    let mut sk = Sketch::new();
    let a = sk.point(0.0, 0.0).unwrap();
    let b = sk.point(3.0, 4.0).unwrap();
    (sk, a, b)
}

#[test]
fn ids_are_minted_in_order_and_never_reused() {
    let (mut sk, a, b) = two_points();
    assert_eq!((a.id(), b.id()), (EntityId(0), EntityId(1)));
    sk.remove(b.id()).unwrap();
    assert_eq!(sk.point(1.0, 1.0).unwrap().id(), EntityId(2));
}

#[test]
fn unknown_id_is_named() {
    let (mut sk, ..) = two_points();
    assert_eq!(
        sk.tag(EntityId(42), "t"),
        Err(ModelError::UnknownId { id: EntityId(42) })
    );
    assert_eq!(
        sk.remove(EntityId(42)),
        Err(ModelError::UnknownId { id: EntityId(42) })
    );
    assert!(sk.point_handle(EntityId(42)).is_err());
}

#[test]
fn a_removed_point_is_unknown_to_a_line() {
    let (mut sk, a, b) = two_points();
    sk.remove(b.id()).unwrap();
    assert_eq!(sk.line(a, b), Err(ModelError::UnknownId { id: b.id() }));
}

#[test]
fn wrong_kind_names_both_kinds() {
    let (mut sk, a, b) = two_points();
    let l = sk.line(a, b).unwrap();
    assert_eq!(
        sk.point_handle(l.id()),
        Err(ModelError::WrongKind {
            id: l.id(),
            expected: EntityKind::Point,
            found: EntityKind::Line
        })
    );
    assert!(sk.circle_handle(a.id()).is_err());
    assert!(sk.arc_handle(a.id()).is_err());
    assert_eq!(sk.line_handle(l.id()), Ok(l));
}

#[test]
fn a_handle_from_another_sketch_is_checked_by_id_and_kind() {
    let (_, a, b) = two_points();
    let mut other = Sketch::new();
    let p = other.point(0.0, 0.0).unwrap();
    // `b` (id 1) is no entity of `other`; `a` (id 0) happens to be a point.
    assert_eq!(other.line(a, b), Err(ModelError::UnknownId { id: b.id() }));
    let q = other.point(1.0, 0.0).unwrap();
    let l = other.line(p, q).unwrap();
    // A foreign handle whose id is a line here is refused by kind.
    let foreign = two_points().1;
    assert_eq!(foreign.id(), p.id());
    assert!(matches!(
        other.arc(foreign, q, p),
        Err(ModelError::RepeatedEntity { .. })
    ));
    assert!(matches!(
        other.point_handle(l.id()),
        Err(ModelError::WrongKind { .. })
    ));
}

#[test]
fn non_finite_is_refused_everywhere() {
    let (mut sk, a, _) = two_points();
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(matches!(
            sk.point(bad, 0.0),
            Err(ModelError::NonFinite { what: "x", .. })
        ));
        assert!(matches!(
            sk.point(0.0, bad),
            Err(ModelError::NonFinite { what: "y", .. })
        ));
        assert!(matches!(
            sk.circle(a, bad),
            Err(ModelError::NonFinite { what: "radius", .. })
        ));
    }
    assert_eq!(sk.entities().count(), 2);
}

#[test]
fn a_negative_radius_is_out_of_range_and_zero_is_not() {
    let (mut sk, a, _) = two_points();
    assert!(matches!(
        sk.circle(a, -0.5),
        Err(ModelError::DimensionOutOfRange { .. })
    ));
    assert!(sk.circle(a, 0.0).is_ok());
}

#[test]
fn duplicate_id_is_refused_for_every_kind() {
    let mut sk = Sketch::new();
    let a = sk.point_with_id(EntityId(5), 0.0, 0.0).unwrap();
    let b = sk.point_with_id(EntityId(6), 1.0, 0.0).unwrap();
    let c = sk.point_with_id(EntityId(7), 0.0, 1.0).unwrap();
    let dup = ModelError::DuplicateId { id: EntityId(5) };
    assert_eq!(sk.point_with_id(EntityId(5), 0.0, 0.0), Err(dup.clone()));
    assert_eq!(sk.line_with_id(EntityId(5), a, b), Err(dup.clone()));
    assert_eq!(sk.circle_with_id(EntityId(5), a, 1.0), Err(dup.clone()));
    assert_eq!(sk.arc_with_id(EntityId(5), a, b, c), Err(dup));
}

#[test]
fn a_caller_id_moves_the_minting_counter_past_it() {
    let mut sk = Sketch::new();
    sk.point_with_id(EntityId(10), 0.0, 0.0).unwrap();
    assert_eq!(sk.point(0.0, 0.0).unwrap().id(), EntityId(11));
}

#[test]
fn ids_run_out_without_a_panic() {
    let mut sk = Sketch::new();
    sk.point_with_id(EntityId(u32::MAX), 0.0, 0.0).unwrap();
    assert_eq!(sk.point(0.0, 0.0), Err(ModelError::IdsExhausted));
}

#[test]
fn a_line_and_an_arc_refuse_a_repeated_point() {
    let (mut sk, a, b) = two_points();
    assert_eq!(
        sk.line(a, a),
        Err(ModelError::RepeatedEntity {
            id: a.id(),
            kind: EntityKind::Line
        })
    );
    assert_eq!(
        sk.arc(a, b, a),
        Err(ModelError::RepeatedEntity {
            id: a.id(),
            kind: EntityKind::Arc
        })
    );
    assert!(sk.arc(a, b, b).is_err());
    assert!(sk.arc(a, a, b).is_err());
}

#[test]
fn a_refused_call_changes_nothing() {
    let (mut sk, a, b) = two_points();
    sk.line(a, b).unwrap();
    let rev = sk.revision();
    let _ = sk.line(a, a);
    let _ = sk.point(f64::NAN, 0.0);
    let _ = sk.tag(EntityId(99), "x");
    let _ = sk.remove(a.id());
    assert_eq!(sk.revision(), rev);
    assert_eq!(sk.entities().count(), 3);
}

#[test]
fn every_successful_mutation_bumps_the_revision() {
    let (mut sk, a, b) = two_points();
    let mut rev = sk.revision();
    assert_eq!(rev, 2);
    let mut bumped = |sk: &Sketch| {
        assert!(sk.revision() > rev);
        rev = sk.revision();
    };
    let l = sk.line(a, b).unwrap();
    bumped(&sk);
    sk.tag(l.id(), "t").unwrap();
    bumped(&sk);
    sk.payload(l.id(), Payload(vec![1])).unwrap();
    bumped(&sk);
    sk.set_construction(l.id(), true).unwrap();
    bumped(&sk);
    sk.set_fixed(a.id(), Coordinate::X, true).unwrap();
    bumped(&sk);
    sk.remove(l.id()).unwrap();
    bumped(&sk);
    sk.remove_with_constraints(a.id()).unwrap();
    bumped(&sk);
}

#[test]
fn fixed_parameters_are_per_coordinate() {
    let (mut sk, a, b) = two_points();
    let c = sk.circle(a, 2.0).unwrap();
    let l = sk.line(a, b).unwrap();
    sk.set_fixed(a.id(), Coordinate::Y, true).unwrap();
    assert!(sk.is_fixed(a.id(), Coordinate::Y));
    assert!(!sk.is_fixed(a.id(), Coordinate::X));
    sk.set_fixed(c.id(), Coordinate::R, true).unwrap();
    sk.set_fixed(a.id(), Coordinate::Y, false).unwrap();
    assert!(!sk.is_fixed(a.id(), Coordinate::Y));
    for (id, co) in [
        (l.id(), Coordinate::X),
        (a.id(), Coordinate::R),
        (c.id(), Coordinate::X),
    ] {
        assert_eq!(
            sk.set_fixed(id, co, true),
            Err(ModelError::NoSuchParameter { id, coordinate: co })
        );
    }
    assert!(sk.set_fixed(EntityId(77), Coordinate::X, true).is_err());
    let g = sk.point_fixed(1.0, 1.0).unwrap();
    assert!(sk.is_fixed(g.id(), Coordinate::X) && sk.is_fixed(g.id(), Coordinate::Y));
}

#[test]
fn tags_payloads_and_construction_round_trip() {
    let (mut sk, a, _) = two_points();
    sk.tag(a.id(), "one").unwrap();
    sk.tag(a.id(), "two").unwrap();
    assert_eq!(sk.tag_of(a.id()), Some("two"));
    sk.payload(a.id(), Payload(vec![0, 255])).unwrap();
    assert_eq!(sk.payload_of(a.id()), Some(&Payload(vec![0, 255])));
    sk.set_construction(a.id(), true).unwrap();
    sk.set_construction(a.id(), false).unwrap();
    assert!(!sk.is_construction(a.id()));
    assert!(sk.set_construction(EntityId(9), true).is_err());
    assert!(sk.payload(EntityId(9), Payload(vec![])).is_err());
}

#[test]
fn remove_names_what_still_uses_the_entity() {
    let (mut sk, a, b) = two_points();
    let c = sk.point(0.0, 1.0).unwrap();
    let l = sk.line(a, b).unwrap();
    let arc = sk.arc(a, b, c).unwrap();
    assert_eq!(
        sk.remove(a.id()),
        Err(ModelError::InUse {
            id: a.id(),
            entities: vec![l.id(), arc.id()],
            constraints: vec![]
        })
    );
    assert!(sk.entity(a.id()).is_some());
}

#[test]
fn cascade_removes_dependents_transitively_and_cleans_metadata() {
    let (mut sk, a, b) = two_points();
    let l = sk.line(a, b).unwrap();
    let c = sk.circle(a, 1.0).unwrap();
    let other = sk.point(9.0, 9.0).unwrap();
    sk.tag(l.id(), "gone").unwrap();
    sk.set_fixed(a.id(), Coordinate::X, true).unwrap();
    let removed = sk.remove_with_constraints(a.id()).unwrap();
    assert_eq!(removed.entities, vec![a.id(), l.id(), c.id()]);
    assert!(removed.constraints.is_empty());
    assert_eq!(
        sk.entities().map(|(i, _)| i).collect::<Vec<_>>(),
        vec![b.id(), other.id()]
    );
    assert_eq!(sk.tag_of(l.id()), None);
    assert!(!sk.is_fixed(a.id(), Coordinate::X));
    assert!(sk.remove_with_constraints(a.id()).is_err());
}

#[test]
fn coordinates_hold_points_and_radii_in_id_order() {
    let (mut sk, a, _) = two_points();
    let c = sk.circle(a, 2.5).unwrap();
    sk.line(a, sk.point_handle(EntityId(1)).unwrap()).unwrap();
    let keys: Vec<_> = sk.coordinates().into_iter().collect();
    assert_eq!(
        keys,
        vec![
            ((a.id(), Coordinate::X), 0.0),
            ((a.id(), Coordinate::Y), 0.0),
            ((EntityId(1), Coordinate::X), 3.0),
            ((EntityId(1), Coordinate::Y), 4.0),
            ((c.id(), Coordinate::R), 2.5),
        ]
    );
}

#[test]
fn entities_iterate_in_id_order_with_their_refs() {
    let mut sk = Sketch::new();
    let b = sk.point_with_id(EntityId(8), 1.0, 0.0).unwrap();
    let a = sk.point_with_id(EntityId(2), 0.0, 0.0).unwrap();
    let l = sk.line_with_id(EntityId(1), a, b).unwrap();
    let ids: Vec<_> = sk.entities().map(|(i, _)| i.0).collect();
    assert_eq!(ids, vec![1, 2, 8]);
    assert!(matches!(sk.entity(l.id()), Some(Entity::Line { .. })));
    assert_eq!(sk.entity(l.id()).unwrap().refs(), vec![a.id(), b.id()]);
}
