//! Cases the review of plans/m9-profiles found, through the public API
//! only: every loop is accounted for, and joins and free ends are named by
//! the end points of the edges.
#![allow(clippy::unwrap_used, clippy::expect_used)] // tests: a failing unwrap is the test failing (clippy.toml)

use laman::model::profile::{Loop, Profiles};
use laman::model::tolerance::Tolerances;
use laman::model::{Coordinate, Coordinates, Entity, EntityId, Point, Sketch};
use laman::profile::{ProfileError, profiles};
use laman_check::{Missing, ProfileViolation, check_profiles};
use laman_debug::fixture::Fixture;
use std::path::PathBuf;

/// A closed rectangle over `[x0, x1] x [y0, y1]`, counter-clockwise, and its
/// corner points.
fn rectangle(sk: &mut Sketch, x0: f64, y0: f64, x1: f64, y1: f64) -> [Point; 4] {
    let p = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)].map(|(x, y)| sk.point(x, y).unwrap());
    for i in 0..4 {
        sk.line(p[i], p[(i + 1) % 4]).unwrap();
    }
    p
}

/// Two bars crossing inside a plate and a speck at their crossing:
/// the bars' curves cross without a shared point (unspecified nesting), and
/// the speck lies in all three, so its container count is odd while its
/// innermost container is itself a hole. No loop may vanish.
#[test]
fn a_loop_whose_innermost_container_is_a_hole_is_still_reported() {
    let mut sk = Sketch::new();
    rectangle(&mut sk, 0.0, 0.0, 100.0, 100.0); // A
    rectangle(&mut sk, 10.0, 40.0, 60.0, 60.0); // B, a wide bar
    rectangle(&mut sk, 40.0, 10.0, 65.0, 70.0); // C, a tall bar crossing B
    rectangle(&mut sk, 45.0, 45.0, 55.0, 55.0); // the speck, in A, B and C
    let found = profiles(&sk, &sk.coordinates()).unwrap();
    let loops: usize = found.profiles.iter().map(|p| 1 + p.holes.len()).sum();
    assert_eq!(loops, 4, "{found:#?}");
    // Every outer loop turns counter-clockwise, every hole clockwise.
    for p in &found.profiles {
        assert!(p.outer.signed_area > 0.0);
        assert!(p.holes.iter().all(|h| h.signed_area < 0.0));
    }
    assert_eq!(found, profiles(&sk, &sk.coordinates()).unwrap());
}

/// A four-sided open chain a-b-c-d-e around a square of side 10 whose free
/// ends `a` and `e` are both at the origin, `e` made of its own point.
fn open_square(sk: &mut Sketch) -> (Point, Point) {
    let a = sk.point(0.0, 0.0).unwrap();
    let b = sk.point(10.0, 0.0).unwrap();
    let c = sk.point(10.0, 10.0).unwrap();
    let d = sk.point(0.0, 10.0).unwrap();
    let e = sk.point(0.0, 0.0).unwrap();
    sk.line(a, b).unwrap();
    sk.line(b, c).unwrap();
    sk.line(c, d).unwrap();
    sk.line(d, e).unwrap();
    (a, e)
}

/// D4 names both end points of the edges, not the joins' smallest ids: a
/// construction point with a smaller id made coincident with an end must not
/// be reported in its place.
#[test]
fn unjoined_names_the_edges_end_points() {
    let mut sk = Sketch::new();
    let k = sk.point(0.0, 0.0).unwrap(); // smaller than every end
    let (a, e) = open_square(&mut sk);
    sk.coincident(k, e).unwrap();
    let found = profiles(&sk, &sk.coordinates()).unwrap();
    assert!(found.profiles.is_empty());
    assert_eq!(found.unjoined.len(), 1, "{found:#?}");
    assert_eq!(
        (found.unjoined[0].first, found.unjoined[0].second),
        (a.id(), e.id())
    );
}

/// `Loop::joins` names the `coincident`s that join two or more distinct end
/// points of the loop, not one that only ties a corner to a point no edge of
/// the loop uses.
#[test]
fn loop_joins_exclude_coincidents_to_points_off_the_loop() {
    let mut sk = Sketch::new();
    let (a, e) = open_square(&mut sk);
    let q = sk.point(0.0, 0.0).unwrap(); // on no edge
    let join = sk.coincident(a, e).unwrap();
    let off = sk.coincident(a, q).unwrap();
    let found = profiles(&sk, &sk.coordinates()).unwrap();
    assert_eq!(found.profiles.len(), 1);
    let joins = &found.profiles[0].outer.joins;
    assert!(joins.contains(&join), "{joins:?}");
    assert!(!joins.contains(&off), "{joins:?}");
}

/// A chain through a point no edge uses is a join: both links are named.
#[test]
fn loop_joins_name_a_chain_through_an_off_loop_point() {
    let mut sk = Sketch::new();
    let (a, e) = open_square(&mut sk);
    let q = sk.point(0.0, 0.0).unwrap();
    let first = sk.coincident(a, q).unwrap();
    let second = sk.coincident(q, e).unwrap();
    let found = profiles(&sk, &sk.coordinates()).unwrap();
    assert_eq!(found.profiles.len(), 1);
    assert_eq!(found.profiles[0].outer.joins, vec![first, second]);
}

/// D1/Q3 A: a `coincident` the values violate still joins its ends, so the
/// loop closes and names it; the free ends it joins are not also reported.
#[test]
fn a_violated_joining_coincident_still_closes_its_loop_and_is_named() {
    let mut sk = Sketch::new();
    let (a, e) = open_square(&mut sk);
    let join = sk.coincident(a, e).unwrap();
    let mut values = sk.coordinates();
    values.insert((e.id(), Coordinate::X), 3.0);
    values.insert((e.id(), Coordinate::Y), 4.0);
    let found = profiles(&sk, &values).unwrap();
    assert_eq!(found.profiles.len(), 1, "{found:#?}");
    assert_eq!(found.profiles[0].outer.joins, vec![join]);
    assert!(found.open_chains.is_empty() && found.unjoined.is_empty());
}

/// An edge whose two ends are one join is a self-joined edge, listed in
/// `Profiles::degenerate` and left out of every other list.
#[test]
fn a_self_joined_edge_is_reported_degenerate() {
    let mut sk = Sketch::new();
    let a = sk.point(0.0, 0.0).unwrap();
    let b = sk.point(5.0, 0.0).unwrap();
    let line = sk.line(a, b).unwrap();
    sk.coincident(a, b).unwrap();
    let found = profiles(&sk, &sk.coordinates()).unwrap();
    assert_eq!(found.degenerate, vec![line.id()]);
    assert!(found.profiles.is_empty() && found.open_chains.is_empty());
    assert!(found.dangling.is_empty() && found.unjoined.is_empty());
}

/// D4: two free ends are reported when they lie within the smaller of their
/// edges' length tolerances, not the larger.
#[test]
fn unjoined_ends_are_held_to_the_smaller_tolerance() {
    let build = |gap: f64| {
        let mut sk = Sketch::new();
        let a = sk.point(-1.0, 0.0).unwrap();
        let b = sk.point(0.0, 0.0).unwrap();
        let c = sk.point(0.0, gap).unwrap();
        let d = sk.point(1e6, 0.0).unwrap();
        let (short, long) = (sk.line(a, b).unwrap(), sk.line(c, d).unwrap());
        let t = Tolerances::at(&sk, &sk.coordinates());
        // The premise: a short edge near the origin is held far tighter than
        // a long one.
        assert!(t.entity(short.id()) * 100.0 < t.entity(long.id()));
        let ends = (b.id(), c.id());
        (profiles(&sk, &sk.coordinates()).unwrap(), t, short, ends)
    };
    let (_, t, short, _) = build(1e-7);
    let tight = t.entity(short.id());
    let (inside, ..) = build(tight / 2.0);
    let (outside, ..) = build(tight * 4.0);
    let (_, _, _, ends) = build(1e-7);
    assert_eq!(inside.unjoined.len(), 1, "{inside:#?}");
    assert_eq!((inside.unjoined[0].first, inside.unjoined[0].second), ends);
    assert!(outside.unjoined.is_empty(), "{outside:#?}");
}

/// A non-finite coordinate of a point an edge uses is refused by id.
#[test]
fn a_non_finite_point_coordinate_is_refused_by_id() {
    let mut sk = Sketch::new();
    let (a, _) = open_square(&mut sk);
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut values = sk.coordinates();
        values.insert((a.id(), Coordinate::Y), bad);
        assert_eq!(
            profiles(&sk, &values),
            Err(ProfileError::NonFinite { entity: a.id() })
        );
    }
}

// ---- Broken twins of the C3 fixtures (plans/m10-profile-check step 5) ----
//
// Each takes a fixture's own extraction at its expected coordinates, which
// the checker holds clean (`c3.rs`), breaks it one way and names the
// violations the checker must raise, in report order.

/// A violation's variant, without its numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Area,
    NetArea,
    Orientation,
    NotChained,
    OpenJoin,
    ArcOffRadius,
    NotAnEdge,
    Repeated,
    Uncovered,
    Containment,
}

/// A violation's variant and the id it names: a loop's first edge, or the
/// entity itself.
type Named = (Kind, Option<EntityId>);

fn kind(v: &ProfileViolation) -> Named {
    match v {
        ProfileViolation::Area { first, .. } => (Kind::Area, *first),
        ProfileViolation::NetArea { first, .. } => (Kind::NetArea, *first),
        ProfileViolation::Orientation { first } => (Kind::Orientation, *first),
        ProfileViolation::NotChained { first } => (Kind::NotChained, *first),
        ProfileViolation::OpenJoin { first } => (Kind::OpenJoin, *first),
        ProfileViolation::ArcOffRadius { first } => (Kind::ArcOffRadius, *first),
        ProfileViolation::NotAnEdge { entity } => (Kind::NotAnEdge, Some(*entity)),
        ProfileViolation::Repeated { entity } => (Kind::Repeated, Some(*entity)),
        ProfileViolation::Uncovered { entity } => (Kind::Uncovered, Some(*entity)),
        ProfileViolation::Containment { first } => (Kind::Containment, *first),
    }
}

/// A loop's first edge, which is how a violation names the loop.
fn first(l: &Loop) -> Option<EntityId> {
    l.edges.first().map(|t| t.entity)
}

/// A fixture with its expected coordinates and the profiles extracted there.
struct Twin {
    sketch: Sketch,
    values: Coordinates,
    found: Profiles,
}

fn twin(name: &str) -> Twin {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/c3")
        .join(name);
    let fixture = Fixture::load(&dir).unwrap();
    let mut values = fixture.sketch.coordinates();
    for (key, value) in &fixture.expected.coordinates {
        values.insert(*key, *value);
    }
    let found = profiles(&fixture.sketch, &values).unwrap();
    let control = check_profiles(&fixture.sketch, &values, &found);
    assert!(control.is_clean(), "{name}: {control:#?}");
    Twin {
        sketch: fixture.sketch,
        values,
        found,
    }
}

impl Twin {
    fn named(&self) -> Vec<Named> {
        let report = check_profiles(&self.sketch, &self.values, &self.found);
        assert!(report.missing.is_empty(), "{report:#?}");
        report.violations.iter().map(kind).collect()
    }

    /// The end points of an edge, as the sketch draws them.
    fn ends(&self, edge: EntityId) -> Vec<Point> {
        match self.sketch.entity(edge) {
            Some(Entity::Line { a, b }) => vec![*a, *b],
            Some(Entity::Arc { c, s, e }) => vec![*s, *e, *c],
            Some(Entity::Point { .. } | Entity::Circle { .. }) | None => Vec::new(),
        }
    }

    fn shift(&mut self, p: Point, coordinate: Coordinate, by: f64) {
        *self.values.get_mut(&(p.id(), coordinate)).unwrap() += by;
    }
}

/// The loop traversed the other way: edges reversed in order and sense.
fn turned_round(l: &mut Loop) {
    l.edges.reverse();
    for t in &mut l.edges {
        t.reversed = !t.reversed;
    }
}

#[test]
fn an_area_nudged_past_the_allowance_is_named() {
    let mut t = twin("rectangle");
    let outer = first(&t.found.profiles[0].outer);
    t.found.profiles[0].outer.signed_area *= 1.001;
    assert_eq!(t.named(), [(Kind::Area, outer)]);

    let mut t = twin("slotted-plate");
    let hole = first(&t.found.profiles[0].holes[0]);
    t.found.profiles[0].holes[0].signed_area *= 1.01;
    assert_eq!(t.named(), [(Kind::Area, hole)]);
}

/// The net area is compared on its own: the loops' reported areas stay
/// right, and a profile's `net_area` is nudged, saturated or not a number.
#[test]
fn a_net_area_nudged_past_the_allowance_is_named() {
    let mut t = twin("rectangle");
    let outer = first(&t.found.profiles[0].outer);
    t.found.profiles[0].net_area *= 1.001;
    assert_eq!(t.named(), [(Kind::NetArea, outer)]);

    let mut t = twin("slotted-plate");
    let outer = first(&t.found.profiles[0].outer);
    t.found.profiles[0].net_area *= 1.01;
    assert_eq!(t.named(), [(Kind::NetArea, outer)]);

    for bad in [f64::MAX, -f64::MAX, f64::NAN, f64::INFINITY, 0.0] {
        let mut t = twin("slotted-plate");
        let outer = first(&t.found.profiles[0].outer);
        t.found.profiles[0].net_area = bad;
        assert_eq!(t.named(), [(Kind::NetArea, outer)], "{bad}");
    }
}

/// Holes added to the outer area instead of subtracted from it.
#[test]
fn holes_added_instead_of_subtracted_are_named() {
    let mut t = twin("slotted-plate");
    let p = &mut t.found.profiles[0];
    let outer = first(&p.outer);
    p.net_area = p.outer.signed_area - p.holes.iter().map(|h| h.signed_area).sum::<f64>();
    assert_eq!(t.named(), [(Kind::NetArea, outer)]);
}

/// A loop left uncompared leaves the net undecided: the open join is named,
/// not also the net the open loop would have moved.
#[test]
fn a_net_area_is_not_compared_where_a_loop_is_open() {
    let mut t = twin("slotted-plate-coincident");
    let hole = &t.found.profiles[0].holes[0];
    let hole_first = first(hole);
    let lines: Vec<EntityId> = hole
        .edges
        .iter()
        .map(|e| e.entity)
        .filter(|e| matches!(t.sketch.entity(*e), Some(Entity::Line { .. })))
        .collect();
    let end = t.ends(lines[0])[0];
    t.shift(end, Coordinate::X, 5.0);
    t.found.profiles[0].net_area *= 2.0;
    assert_eq!(t.named(), [(Kind::OpenJoin, hole_first)]);
}

#[test]
fn a_loop_reversed_is_named_by_its_orientation() {
    // Edges and area turned together: only the role is wrong.
    let mut t = twin("slotted-plate");
    let outer = &mut t.found.profiles[0].outer;
    turned_round(outer);
    outer.signed_area = -outer.signed_area;
    let named = first(outer);
    assert_eq!(t.named(), [(Kind::Orientation, named)]);

    // Edges turned and the area left: the area is wrong as well.
    let mut t = twin("slotted-plate");
    turned_round(&mut t.found.profiles[0].holes[0]);
    let named = first(&t.found.profiles[0].holes[0]);
    assert_eq!(t.named(), [(Kind::Area, named), (Kind::Orientation, named)]);
}

#[test]
fn a_hole_moved_to_the_wrong_parent_is_named() {
    // The plate's hole given to the island, which lies inside it: both
    // nets are wrong, the hole is outside the island, and the island is
    // inside a plate with no hole.
    let mut t = twin("plate-island");
    assert_eq!(t.found.profiles.len(), 2);
    let (plate, island) = (
        first(&t.found.profiles[0].outer),
        first(&t.found.profiles[1].outer),
    );
    let holes = std::mem::take(&mut t.found.profiles[0].holes);
    let hole = first(&holes[0]);
    t.found.profiles[1].holes = holes;
    assert_eq!(
        t.named(),
        [
            (Kind::NetArea, plate),
            (Kind::NetArea, island),
            (Kind::Containment, hole),
            (Kind::Containment, island),
        ]
    );

    // The island listed as a second hole of the plate: inside, but turning
    // the wrong way for a hole, and subtracted from the plate's net.
    let mut t = twin("plate-island");
    let plate = first(&t.found.profiles[0].outer);
    let island = t.found.profiles.pop().unwrap().outer;
    let named = first(&island);
    t.found.profiles[0].holes.push(island);
    assert_eq!(
        t.named(),
        [(Kind::Orientation, named), (Kind::NetArea, plate)]
    );
}

#[test]
fn an_edge_dropped_or_repeated_is_named() {
    let mut t = twin("dangling-edge");
    let dangling = t.found.dangling[0];
    t.found.dangling.clear();
    assert_eq!(t.named(), [(Kind::Uncovered, Some(dangling))]);

    let mut t = twin("dangling-edge");
    let dangling = t.found.dangling.clone();
    t.found.open_chains.push(dangling.clone());
    assert_eq!(t.named(), [(Kind::Repeated, Some(dangling[0]))]);

    // Out of a loop: it no longer closes, and the edge is in no list.
    let mut t = twin("rectangle");
    let outer = first(&t.found.profiles[0].outer);
    let dropped = t.found.profiles[0].outer.edges.pop().unwrap().entity;
    assert_eq!(
        t.named(),
        [(Kind::NotChained, outer), (Kind::Uncovered, Some(dropped))]
    );

    let mut t = twin("rectangle");
    let outer = first(&t.found.profiles[0].outer);
    let again = t.found.profiles[0].outer.edges[0];
    t.found.profiles[0].outer.edges.push(again);
    assert_eq!(
        t.named(),
        [
            (Kind::NotChained, outer),
            (Kind::Repeated, Some(again.entity))
        ]
    );
}

#[test]
fn a_stale_id_or_a_wrong_kind_is_named() {
    let mut t = twin("rectangle");
    let was = t.found.profiles[0].outer.edges[1].entity;
    t.found.profiles[0].outer.edges[1].entity = EntityId(9999);
    assert_eq!(
        t.named(),
        [
            (Kind::NotAnEdge, Some(EntityId(9999))),
            (Kind::Uncovered, Some(was))
        ]
    );

    let mut t = twin("rectangle");
    let point = t
        .sketch
        .entities()
        .find_map(|(id, e)| matches!(e, Entity::Point { .. }).then_some(id))
        .unwrap();
    let was = t.found.profiles[0].outer.edges[2].entity;
    t.found.profiles[0].outer.edges[2].entity = point;
    assert_eq!(
        t.named(),
        [(Kind::NotAnEdge, Some(point)), (Kind::Uncovered, Some(was))]
    );

    let mut t = twin("dangling-edge");
    t.found.open_chains.push(vec![EntityId(9999)]);
    assert_eq!(t.named(), [(Kind::NotAnEdge, Some(EntityId(9999)))]);
}

#[test]
fn an_empty_loop_is_named_and_never_panics() {
    let mut t = twin("rectangle");
    t.found.profiles[0].holes.push(Loop {
        edges: Vec::new(),
        signed_area: -1.0,
        joins: Vec::new(),
    });
    assert_eq!(t.named(), [(Kind::NotChained, None)]);
}

#[test]
fn values_moved_after_extraction_are_named() {
    // A corner moved: neither the area nor the net is the reported one.
    let mut t = twin("rectangle");
    let outer = first(&t.found.profiles[0].outer);
    let corner = t.ends(t.found.profiles[0].outer.edges[0].entity)[1];
    t.shift(corner, Coordinate::X, 10.0);
    assert_eq!(t.named(), [(Kind::Area, outer), (Kind::NetArea, outer)]);

    // One end of a joining coincident moved away: the join is open and the
    // loop's area is not compared.
    let mut t = twin("slotted-plate-coincident");
    let hole = &t.found.profiles[0].holes[0];
    let named = first(hole);
    let lines: Vec<EntityId> = hole
        .edges
        .iter()
        .map(|e| e.entity)
        .filter(|e| matches!(t.sketch.entity(*e), Some(Entity::Line { .. })))
        .collect();
    let end = t.ends(lines[0])[0];
    t.shift(end, Coordinate::X, 5.0);
    assert_eq!(t.named(), [(Kind::OpenJoin, named)]);

    // An arc's centre moved off its bisector: its ends are at two radii.
    let mut t = twin("slotted-plate");
    let named = first(&t.found.profiles[0].holes[0]);
    let arc = t.found.profiles[0].holes[0]
        .edges
        .iter()
        .map(|e| e.entity)
        .find(|e| matches!(t.sketch.entity(*e), Some(Entity::Arc { .. })))
        .unwrap();
    let centre = t.ends(arc)[2];
    t.shift(centre, Coordinate::X, 1.0);
    t.shift(centre, Coordinate::Y, 0.7);
    assert_eq!(t.named(), [(Kind::ArcOffRadius, named)]);
}
#[test]
fn a_coordinate_the_values_lack_is_listed_never_judged() {
    for bad in [None, Some(f64::NAN), Some(f64::INFINITY)] {
        let mut t = twin("rectangle");
        let corner = t.ends(t.found.profiles[0].outer.edges[0].entity)[0];
        let key = (corner.id(), Coordinate::Y);
        match bad {
            None => {
                t.values.remove(&key);
            }
            Some(v) => {
                t.values.insert(key, v);
            }
        }
        let report = check_profiles(&t.sketch, &t.values, &t.found);
        assert_eq!(
            report.missing,
            [Missing {
                id: corner.id(),
                coordinate: Some(Coordinate::Y)
            }]
        );
        assert!(report.violations.is_empty(), "{report:#?}");
    }
}
