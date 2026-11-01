//! Constraint-level behaviour of `laman::model`: every kind builds, every
//! refusal is named, every sense rule is pinned, ties included.
// Tests: a failing unwrap or panic is the test failing (clippy.toml covers
// `#[test]` bodies only, not their helpers).
#![allow(clippy::unwrap_used, clippy::panic)]

use std::f64::consts::{FRAC_PI_2, PI};

use laman::model::{
    Constraint, ConstraintId, ConstraintKind, Contact, EntityId, EntityKind, Line, ModelError,
    Payload, Point, Round, Sense, Sketch, Span,
};

/// Points `(0,0)`, `(4,0)`, `(0,3)`, `(4,3)` and lines `p0p1`, `p2p3`.
struct Rig {
    sk: Sketch,
    p: Vec<Point>,
    l0: Line,
    l1: Line,
}

fn rig() -> Rig {
    let mut sk = Sketch::new();
    let p: Vec<Point> = [(0.0, 0.0), (4.0, 0.0), (0.0, 3.0), (4.0, 3.0)]
        .iter()
        .map(|&(x, y)| sk.point(x, y).unwrap())
        .collect();
    let l0 = sk.line(p[0], p[1]).unwrap();
    let l1 = sk.line(p[2], p[3]).unwrap();
    Rig { sk, p, l0, l1 }
}

#[test]
fn every_kind_builds_and_reports_its_kind() {
    let Rig { mut sk, p, l0, l1 } = rig();
    let c = sk.point(10.0, 10.0).unwrap();
    let circle = sk.circle(c, 2.0).unwrap();
    let c2 = sk.point(20.0, 10.0).unwrap();
    let s = sk.point(22.0, 10.0).unwrap();
    let e = sk.point(20.0, 12.0).unwrap();
    let arc = sk.arc(c2, s, e).unwrap();
    let built = [
        (
            sk.coincident(p[0], p[1]).unwrap(),
            ConstraintKind::Coincident,
        ),
        (
            sk.point_on_line(p[2], l0).unwrap(),
            ConstraintKind::PointOnLine,
        ),
        (
            sk.point_on_circle(p[3], circle).unwrap(),
            ConstraintKind::PointOnCircle,
        ),
        (
            sk.point_on_circle(p[3], arc).unwrap(),
            ConstraintKind::PointOnCircle,
        ),
        (sk.horizontal(l0).unwrap(), ConstraintKind::Horizontal),
        (
            sk.horizontal((p[0], p[1])).unwrap(),
            ConstraintKind::Horizontal,
        ),
        (sk.vertical(l1).unwrap(), ConstraintKind::Vertical),
        (sk.vertical((p[0], p[2])).unwrap(), ConstraintKind::Vertical),
        (sk.parallel(l0, l1).unwrap(), ConstraintKind::Parallel),
        (
            sk.perpendicular(l0, l1).unwrap(),
            ConstraintKind::Perpendicular,
        ),
        (
            sk.equal_length(l0, l1).unwrap(),
            ConstraintKind::EqualLength,
        ),
        (
            sk.equal_radius(circle, arc).unwrap(),
            ConstraintKind::EqualRadius,
        ),
        (
            sk.tangent_line_circle(l0, circle).unwrap(),
            ConstraintKind::TangentLineCircle,
        ),
        (
            sk.tangent_circles(circle, arc).unwrap(),
            ConstraintKind::TangentCircles,
        ),
        (sk.midpoint(p[2], l0).unwrap(), ConstraintKind::Midpoint),
        (
            sk.symmetric(p[2], p[3], l0).unwrap(),
            ConstraintKind::Symmetric,
        ),
        (sk.fix(p[0], 1.0, 2.0).unwrap(), ConstraintKind::Fix),
        (
            sk.distance(p[0], p[1], 4.0).unwrap(),
            ConstraintKind::Distance,
        ),
        (
            sk.point_line_distance(p[2], l0, 3.0).unwrap(),
            ConstraintKind::PointLineDistance,
        ),
        (sk.radius(arc, 2.0).unwrap(), ConstraintKind::Radius),
        (sk.angle(l0, l1, FRAC_PI_2).unwrap(), ConstraintKind::Angle),
    ];
    assert_eq!(built.len(), 21);
    for (i, (id, kind)) in built.iter().enumerate() {
        assert_eq!(id.0 as usize, i, "ids are minted in order");
        assert_eq!(sk.constraint(*id).map(Constraint::kind), Some(*kind));
    }
    assert_eq!(sk.constraints().count(), 21);
    assert_eq!(ConstraintKind::TangentCircles.to_string(), "tangent");
}

#[test]
fn an_unknown_entity_is_named() {
    let Rig { mut sk, p, .. } = rig();
    let mut other = Sketch::new();
    let ghosts: Vec<Point> = (0..8)
        .map(|i| other.point(f64::from(i), 0.0).unwrap())
        .collect();
    assert_eq!(
        sk.coincident(p[0], ghosts[7]),
        Err(ModelError::UnknownId { id: ghosts[7].id() })
    );
    let before = sk.revision();
    assert!(sk.fix(ghosts[7], 0.0, 0.0).is_err());
    assert_eq!(sk.revision(), before);
}

#[test]
fn the_wrong_kind_is_named_with_both_kinds() {
    let Rig { mut sk, p, l0, .. } = rig();
    let mut other = Sketch::new();
    // In `other`, id 2 is a line; in `sk` it is a point.
    let (a, b) = (
        other.point(0.0, 0.0).unwrap(),
        other.point(1.0, 0.0).unwrap(),
    );
    let foreign_line = other.line(a, b).unwrap();
    assert_eq!(foreign_line.id(), EntityId(2));
    // id 2 in `sk` is a point
    assert_eq!(
        sk.point_on_line(p[0], foreign_line),
        Err(ModelError::WrongKind {
            id: EntityId(2),
            expected: EntityKind::Line,
            found: EntityKind::Point,
        })
    );
    // A circle slot holding a line: l0 is id 4; a foreign circle with id 4.
    let mut third = Sketch::new();
    let c = third.point(0.0, 0.0).unwrap();
    for _ in 0..3 {
        third.point(1.0, 1.0).unwrap();
    }
    let foreign_circle = third.circle(c, 1.0).unwrap();
    assert_eq!(foreign_circle.id(), l0.id());
    assert_eq!(
        sk.point_on_circle(p[0], foreign_circle),
        Err(ModelError::WrongKind {
            id: l0.id(),
            expected: EntityKind::Circle,
            found: EntityKind::Line,
        })
    );
}

#[test]
fn a_repeated_entity_is_refused_where_two_are_needed() {
    let Rig { mut sk, p, l0, .. } = rig();
    let c = sk.point(10.0, 10.0).unwrap();
    let circle = sk.circle(c, 2.0).unwrap();
    let repeated = |id: EntityId, kind| Err(ModelError::RepeatedEntity { id, kind });
    assert_eq!(
        sk.coincident(p[0], p[0]),
        repeated(p[0].id(), EntityKind::Point)
    );
    assert_eq!(
        sk.distance(p[1], p[1], 1.0),
        repeated(p[1].id(), EntityKind::Point)
    );
    assert_eq!(
        sk.symmetric(p[2], p[2], l0),
        repeated(p[2].id(), EntityKind::Point)
    );
    assert_eq!(
        sk.horizontal((p[0], p[0])),
        repeated(p[0].id(), EntityKind::Point)
    );
    assert_eq!(
        sk.vertical((p[3], p[3])),
        repeated(p[3].id(), EntityKind::Point)
    );
    assert_eq!(sk.parallel(l0, l0), repeated(l0.id(), EntityKind::Line));
    assert_eq!(
        sk.perpendicular(l0, l0),
        repeated(l0.id(), EntityKind::Line)
    );
    assert_eq!(sk.equal_length(l0, l0), repeated(l0.id(), EntityKind::Line));
    assert_eq!(sk.angle(l0, l0, 1.0), repeated(l0.id(), EntityKind::Line));
    assert_eq!(
        sk.equal_radius(circle, circle),
        repeated(circle.id(), EntityKind::Circle)
    );
    assert_eq!(
        sk.tangent_circles(circle, circle),
        repeated(circle.id(), EntityKind::Circle)
    );
    // Sharing a point with the line is not a repetition.
    sk.point_on_line(p[0], l0).unwrap();
    sk.midpoint(p[1], l0).unwrap();
}

#[test]
fn a_distance_of_zero_points_to_coincident() {
    let Rig { mut sk, p, l0, .. } = rig();
    for bad in [0.0, -1.0] {
        let Err(ModelError::DimensionOutOfRange {
            what,
            value,
            wanted,
        }) = sk.distance(p[0], p[1], bad)
        else {
            panic!("distance {bad} must be refused");
        };
        assert_eq!((what, value), ("distance", bad));
        assert!(wanted.contains("coincident"), "{wanted}");
    }
    let Err(ModelError::DimensionOutOfRange { wanted, .. }) = sk.point_line_distance(p[2], l0, 0.0)
    else {
        panic!("point_line_distance 0 must be refused");
    };
    assert!(wanted.contains("point_on_line"), "{wanted}");
    assert!(sk.point_line_distance(p[2], l0, -2.0).is_err());
}

#[test]
fn a_radius_and_an_angle_stay_in_range() {
    let Rig { mut sk, l0, l1, .. } = rig();
    let c = sk.point(10.0, 10.0).unwrap();
    let circle = sk.circle(c, 2.0).unwrap();
    for bad in [0.0, -1.0] {
        assert!(matches!(
            sk.radius(circle, bad),
            Err(ModelError::DimensionOutOfRange { what: "radius", .. })
        ));
    }
    for bad in [0.0, -0.5, PI, 4.0] {
        assert!(
            matches!(
                sk.angle(l0, l1, bad),
                Err(ModelError::DimensionOutOfRange { what: "angle", .. })
            ),
            "angle {bad}"
        );
    }
    sk.angle(l0, l1, 1e-3).unwrap();
    sk.angle(l0, l1, PI - 1e-3).unwrap();
}

#[test]
fn a_non_finite_value_is_refused() {
    let Rig { mut sk, p, l0, l1 } = rig();
    let c = sk.point(10.0, 10.0).unwrap();
    let circle = sk.circle(c, 2.0).unwrap();
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(matches!(
            sk.fix(p[0], bad, 0.0),
            Err(ModelError::NonFinite { what: "x", .. })
        ));
        assert!(matches!(
            sk.fix(p[0], 0.0, bad),
            Err(ModelError::NonFinite { what: "y", .. })
        ));
        assert!(matches!(
            sk.distance(p[0], p[1], bad),
            Err(ModelError::NonFinite { .. })
        ));
        assert!(matches!(
            sk.point_line_distance(p[2], l0, bad),
            Err(ModelError::NonFinite { .. })
        ));
        assert!(matches!(
            sk.radius(circle, bad),
            Err(ModelError::NonFinite { .. })
        ));
        assert!(matches!(
            sk.angle(l0, l1, bad),
            Err(ModelError::NonFinite { .. })
        ));
    }
    assert_eq!(sk.constraints().count(), 0);
}

#[test]
fn a_refused_constraint_changes_nothing() {
    let Rig { mut sk, p, .. } = rig();
    let before = sk.revision();
    assert!(sk.coincident(p[0], p[0]).is_err());
    assert_eq!(sk.revision(), before);
    let id = sk.coincident(p[0], p[1]).unwrap();
    assert_eq!(sk.revision(), before + 1);
    // The refused call did not consume an id.
    assert_eq!(id, ConstraintId(0));
}

#[test]
fn a_tangent_line_takes_the_side_the_centre_is_on() {
    let Rig { mut sk, l0, .. } = rig(); // l0 runs (0,0) to (4,0)
    let above = sk.point(2.0, 5.0).unwrap();
    let below = sk.point(2.0, -5.0).unwrap();
    let on = sk.point(2.0, 0.0).unwrap();
    let sense_of = |sk: &mut Sketch, c: Point| {
        let circle = sk.circle(c, 5.0).unwrap();
        let id = sk.tangent_line_circle(l0, circle).unwrap();
        match sk.constraint(id) {
            Some(Constraint::TangentLineCircle { sense, .. }) => *sense,
            other => panic!("{other:?}"),
        }
    };
    assert_eq!(sense_of(&mut sk, above), Sense::Positive);
    assert_eq!(sense_of(&mut sk, below), Sense::Negative);
    // A centre exactly on the line takes +1.
    assert_eq!(sense_of(&mut sk, on), Sense::Positive);
}

#[test]
fn a_tangent_line_to_an_arc_uses_the_arcs_centre() {
    let Rig { mut sk, l0, .. } = rig();
    let c = sk.point(1.0, -2.0).unwrap();
    let s = sk.point(3.0, -2.0).unwrap();
    let e = sk.point(1.0, 0.0).unwrap();
    let arc = sk.arc(c, s, e).unwrap();
    let id = sk.tangent_line_circle(l0, arc).unwrap();
    assert!(matches!(
        sk.constraint(id),
        Some(Constraint::TangentLineCircle {
            sense: Sense::Negative,
            round: Round::Arc(_),
            ..
        })
    ));
}

#[test]
fn a_point_line_distance_takes_the_side_the_point_is_on() {
    let Rig { mut sk, p, l0, .. } = rig();
    let below = sk.point(1.0, -7.0).unwrap();
    let sense = |sk: &Sketch, id| match sk.constraint(id) {
        Some(Constraint::PointLineDistance { sense, .. }) => *sense,
        other => panic!("{other:?}"),
    };
    let up = sk.point_line_distance(p[2], l0, 3.0).unwrap();
    let down = sk.point_line_distance(below, l0, 7.0).unwrap();
    // On the line itself (p[1] is the line's end): +1.
    let on = sk.point_line_distance(p[1], l0, 1.0).unwrap();
    assert_eq!(sense(&sk, up), Sense::Positive);
    assert_eq!(sense(&sk, down), Sense::Negative);
    assert_eq!(sense(&sk, on), Sense::Positive);
}

fn contact_of(sk: &mut Sketch, a: Round, b: Round) -> Contact {
    let id = sk.tangent_circles(a, b).unwrap();
    match sk.constraint(id) {
        Some(Constraint::TangentCircles { contact, .. }) => *contact,
        other => panic!("{other:?}"),
    }
}

/// Two circles of radii `ra`, `rb` with centres `d` apart on the x axis.
fn two_circles(ra: f64, rb: f64, d: f64) -> (Sketch, Round, Round) {
    let mut sk = Sketch::new();
    let (ca, cb) = (sk.point(0.0, 0.0).unwrap(), sk.point(d, 0.0).unwrap());
    let a = sk.circle(ca, ra).unwrap();
    let b = sk.circle(cb, rb).unwrap();
    (sk, a.into(), b.into())
}

#[test]
fn circle_contact_is_external_or_internal_by_which_distance_is_nearer() {
    // r = 3, 1: external at d = 4, internal at d = 2.
    for (d, want) in [
        (4.0, Contact::External),
        (5.0, Contact::External),
        (2.0, Contact::Internal(Sense::Positive)),
        (1.0, Contact::Internal(Sense::Positive)),
        (0.0, Contact::Internal(Sense::Positive)),
    ] {
        let (mut sk, a, b) = two_circles(3.0, 1.0, d);
        assert_eq!(contact_of(&mut sk, a, b), want, "d = {d}");
    }
    // The smaller first: internal takes the negative sense.
    let (mut sk, a, b) = two_circles(1.0, 3.0, 2.0);
    assert_eq!(
        contact_of(&mut sk, a, b),
        Contact::Internal(Sense::Negative)
    );
}

#[test]
fn circle_contact_ties_are_external() {
    // r = 3, 1: |r_a - r_b| = 2 and r_a + r_b = 4, so d = 3 is equidistant.
    let (mut sk, a, b) = two_circles(3.0, 1.0, 3.0);
    assert_eq!(contact_of(&mut sk, a, b), Contact::External);
    // Equal radii, same centre cannot tie (0 against 2r) and is internal,
    // its sense the tie of equal radii: positive.
    let (mut sk, a, b) = two_circles(2.0, 2.0, 0.0);
    assert_eq!(
        contact_of(&mut sk, a, b),
        Contact::Internal(Sense::Positive)
    );
}

#[test]
fn an_arcs_radius_is_the_distance_from_centre_to_start() {
    let mut sk = Sketch::new();
    let c = sk.point(0.0, 0.0).unwrap();
    let s = sk.point(2.0, 0.0).unwrap();
    let e = sk.point(0.0, 2.0).unwrap();
    let arc = sk.arc(c, s, e).unwrap();
    let c2 = sk.point(5.0, 0.0).unwrap();
    let circle = sk.circle(c2, 3.0).unwrap();
    // d = 5 = 2 + 3.
    assert_eq!(
        contact_of(&mut sk, arc.into(), circle.into()),
        Contact::External
    );
}

#[test]
fn a_constraint_added_by_hand_keeps_the_sense_it_carries() {
    let Rig { mut sk, l0, .. } = rig();
    let c = sk.point(2.0, 5.0).unwrap();
    let circle = sk.circle(c, 5.0).unwrap();
    let hand = Constraint::TangentLineCircle {
        line: l0,
        round: circle.into(),
        sense: Sense::Negative,
    };
    let id = sk.add_constraint(hand).unwrap();
    assert_eq!(sk.constraint(id), Some(&hand));
}

#[test]
fn a_caller_id_must_be_free_and_moves_the_minting() {
    let Rig { mut sk, p, .. } = rig();
    let desc = Constraint::Coincident { p: p[0], q: p[1] };
    sk.add_constraint_with_id(ConstraintId(5), desc).unwrap();
    assert_eq!(
        sk.add_constraint_with_id(ConstraintId(5), desc),
        Err(ModelError::DuplicateConstraintId {
            id: ConstraintId(5)
        })
    );
    assert_eq!(sk.coincident(p[2], p[3]).unwrap(), ConstraintId(6));
}

#[test]
fn constraint_tags_and_payloads_are_kept_and_checked() {
    let Rig { mut sk, p, .. } = rig();
    let id = sk.distance(p[0], p[1], 4.0).unwrap();
    sk.tag_constraint(id, "width").unwrap();
    sk.payload_constraint(id, Payload(vec![1, 2])).unwrap();
    assert_eq!(sk.constraint_tag_of(id), Some("width"));
    assert_eq!(sk.constraint_payload_of(id), Some(&Payload(vec![1, 2])));
    let ghost = ConstraintId(99);
    let unknown = Err(ModelError::UnknownConstraint { id: ghost });
    assert_eq!(sk.tag_constraint(ghost, "x"), unknown);
    assert_eq!(sk.payload_constraint(ghost, Payload(vec![])), unknown);
    assert_eq!(sk.remove_constraint(ghost), unknown);
    sk.remove_constraint(id).unwrap();
    assert_eq!(sk.constraint_tag_of(id), None);
    assert_eq!(sk.constraint_payload_of(id), None);
    // The removed id is not minted again.
    assert_eq!(sk.distance(p[0], p[1], 4.0).unwrap(), ConstraintId(1));
}

#[test]
fn removing_an_entity_that_constraints_use_is_refused_naming_them() {
    let Rig { mut sk, p, l0, l1 } = rig();
    let d = sk.distance(p[0], p[1], 4.0).unwrap();
    let par = sk.parallel(l0, l1).unwrap();
    // p[3] is only in l1; l1 is in `par`.
    assert_eq!(
        sk.remove(l1.id()),
        Err(ModelError::InUse {
            id: l1.id(),
            entities: vec![],
            constraints: vec![par],
        })
    );
    assert_eq!(
        sk.remove(p[0].id()),
        Err(ModelError::InUse {
            id: p[0].id(),
            entities: vec![l0.id()],
            constraints: vec![d],
        })
    );
    sk.remove_constraint(par).unwrap();
    sk.remove(l1.id()).unwrap();
}

#[test]
fn the_cascade_takes_the_constraints_of_everything_it_removes() {
    let Rig { mut sk, p, l0, l1 } = rig();
    let d = sk.distance(p[0], p[1], 4.0).unwrap();
    let par = sk.parallel(l0, l1).unwrap();
    let keep = sk.distance(p[2], p[3], 4.0).unwrap();
    let gone = sk.remove_with_constraints(p[0].id()).unwrap();
    assert_eq!(gone.entities, vec![p[0].id(), l0.id()]);
    assert_eq!(gone.constraints, vec![d, par]);
    assert_eq!(
        sk.constraints().map(|(id, _)| id).collect::<Vec<_>>(),
        vec![keep]
    );
}

#[test]
fn a_span_converts_from_a_line_and_from_two_points() {
    let Rig { p, l0, .. } = rig();
    assert_eq!(Span::from(l0), Span::Line(l0));
    assert_eq!(Span::from((p[0], p[1])), Span::Points(p[0], p[1]));
}
