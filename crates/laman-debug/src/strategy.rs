//! `proptest` strategies for well-formed sketches.
//!
//! A sketch is drawn as a list of operations ("add a point", "constrain two
//! lines", "tag an entity") applied through the public builder, with indices
//! resolved against what exists. An operation the builder refuses is
//! skipped, so every sketch drawn is well-formed by construction, and the
//! failing recipe shrinks to a short list.
//!
//! [`configuration`] is the other kind: one constraint and the few entities
//! it reads, at coordinates drawn to reach the places a gradient is wrong.

pub mod configuration;

use laman::model::{
    ConstraintKind, Coordinate, Entity, EntityId, EntityKind, Payload, Point, Sketch,
};
use proptest::collection::vec;
use proptest::prelude::*;
use proptest::sample::select;

/// A coordinate: mostly a plain number a person would type, sometimes any
/// finite `f64` (subnormals, huge values, negative zero) to test the
/// round-trip bits.
fn coordinate() -> impl Strategy<Value = f64> {
    prop_oneof![
        8 => -1.0e3..1.0e3_f64,
        1 => proptest::num::f64::NORMAL | proptest::num::f64::SUBNORMAL | proptest::num::f64::ZERO,
    ]
}

/// A positive length or radius.
fn positive() -> impl Strategy<Value = f64> {
    prop_oneof![
        8 => 1.0e-3..1.0e3_f64,
        1 => proptest::num::f64::POSITIVE | proptest::num::f64::NORMAL,
    ]
}

#[derive(Debug, Clone)]
enum Op {
    Point(f64, f64),
    Line(usize, usize),
    Circle(usize, f64),
    Arc(usize, usize, usize),
    Constrain {
        kind: ConstraintKind,
        picks: [usize; 3],
        value: f64,
        flag: bool,
    },
    Fixed(usize, bool),
    Construction(usize),
    Tag(usize, String),
    Payload(usize, Vec<u8>),
    ConstraintTag(usize, String),
    ConstraintPayload(usize, Vec<u8>),
}

const KINDS: [ConstraintKind; 18] = [
    ConstraintKind::Coincident,
    ConstraintKind::PointOnLine,
    ConstraintKind::PointOnCircle,
    ConstraintKind::Horizontal,
    ConstraintKind::Vertical,
    ConstraintKind::Parallel,
    ConstraintKind::Perpendicular,
    ConstraintKind::EqualLength,
    ConstraintKind::EqualRadius,
    ConstraintKind::TangentLineCircle,
    ConstraintKind::TangentCircles,
    ConstraintKind::Midpoint,
    ConstraintKind::Symmetric,
    ConstraintKind::Fix,
    ConstraintKind::Distance,
    ConstraintKind::PointLineDistance,
    ConstraintKind::Radius,
    ConstraintKind::Angle,
];

fn text() -> impl Strategy<Value = String> {
    // Plain names and a few that need JSON escapes or are not ASCII.
    prop_oneof![
        4 => "[a-z][a-z0-9-]{0,8}",
        1 => "\\PC{0,6}",
    ]
}

fn op() -> impl Strategy<Value = Op> {
    let idx = || 0..64_usize;
    prop_oneof![
        6 => (coordinate(), coordinate()).prop_map(|(x, y)| Op::Point(x, y)),
        3 => (idx(), idx()).prop_map(|(a, b)| Op::Line(a, b)),
        2 => (idx(), prop_oneof![positive(), Just(0.0)]).prop_map(|(c, r)| Op::Circle(c, r)),
        1 => (idx(), idx(), idx()).prop_map(|(c, s, e)| Op::Arc(c, s, e)),
        10 => (
            select(KINDS.to_vec()),
            [idx(), idx(), idx()],
            prop_oneof![positive(), 0.01..3.1_f64, coordinate()],
            any::<bool>(),
        )
            .prop_map(|(kind, picks, value, flag)| Op::Constrain { kind, picks, value, flag }),
        2 => (idx(), any::<bool>()).prop_map(|(i, y)| Op::Fixed(i, y)),
        1 => idx().prop_map(Op::Construction),
        2 => (idx(), text()).prop_map(|(i, t)| Op::Tag(i, t)),
        1 => (idx(), vec(any::<u8>(), 0..8)).prop_map(|(i, p)| Op::Payload(i, p)),
        1 => (idx(), text()).prop_map(|(i, t)| Op::ConstraintTag(i, t)),
        1 => (idx(), vec(any::<u8>(), 0..8)).prop_map(|(i, p)| Op::ConstraintPayload(i, p)),
    ]
}

/// What a recipe has built so far, by kind, in creation order.
#[derive(Default)]
struct Built {
    points: Vec<Point>,
    lines: Vec<laman::model::Line>,
    rounds: Vec<laman::model::Round>,
    entities: Vec<EntityId>,
    constraints: Vec<laman::model::ConstraintId>,
}

fn pick<T: Copy>(of: &[T], i: usize) -> Option<T> {
    if of.is_empty() {
        None
    } else {
        of.get(i % of.len()).copied()
    }
}

fn apply(sk: &mut Sketch, built: &mut Built, op: &Op) {
    // A refused operation is skipped: the builder is the validator, and a
    // sketch that survives it is well-formed.
    let _ = try_apply(sk, built, op);
}

fn try_apply(sk: &mut Sketch, built: &mut Built, op: &Op) -> Option<()> {
    match op {
        Op::Point(x, y) => {
            let p = sk.point(*x, *y).ok()?;
            built.points.push(p);
            built.entities.push(p.id());
        }
        Op::Line(a, b) => {
            let l = sk
                .line(pick(&built.points, *a)?, pick(&built.points, *b)?)
                .ok()?;
            built.lines.push(l);
            built.entities.push(l.id());
        }
        Op::Circle(c, r) => {
            let circle = sk.circle(pick(&built.points, *c)?, *r).ok()?;
            built.rounds.push(circle.into());
            built.entities.push(circle.id());
        }
        Op::Arc(c, s, e) => {
            let arc = sk
                .arc(
                    pick(&built.points, *c)?,
                    pick(&built.points, *s)?,
                    pick(&built.points, *e)?,
                )
                .ok()?;
            built.rounds.push(arc.into());
            built.entities.push(arc.id());
        }
        Op::Constrain {
            kind,
            picks: [i, j, k],
            value,
            flag,
        } => {
            let p = |n: usize| pick(&built.points, n);
            let l = |n: usize| pick(&built.lines, n);
            let r = |n: usize| pick(&built.rounds, n);
            let id = match kind {
                ConstraintKind::Coincident => sk.coincident(p(*i)?, p(*j)?),
                ConstraintKind::PointOnLine => sk.point_on_line(p(*i)?, l(*j)?),
                ConstraintKind::PointOnCircle => sk.point_on_circle(p(*i)?, r(*j)?),
                ConstraintKind::Horizontal if *flag => sk.horizontal((p(*i)?, p(*j)?)),
                ConstraintKind::Horizontal => sk.horizontal(l(*i)?),
                ConstraintKind::Vertical if *flag => sk.vertical((p(*i)?, p(*j)?)),
                ConstraintKind::Vertical => sk.vertical(l(*i)?),
                ConstraintKind::Parallel => sk.parallel(l(*i)?, l(*j)?),
                ConstraintKind::Perpendicular => sk.perpendicular(l(*i)?, l(*j)?),
                ConstraintKind::EqualLength => sk.equal_length(l(*i)?, l(*j)?),
                ConstraintKind::EqualRadius => sk.equal_radius(r(*i)?, r(*j)?),
                ConstraintKind::TangentLineCircle => sk.tangent_line_circle(l(*i)?, r(*j)?),
                ConstraintKind::TangentCircles => sk.tangent_circles(r(*i)?, r(*j)?),
                ConstraintKind::Midpoint => sk.midpoint(p(*i)?, l(*j)?),
                ConstraintKind::Symmetric => sk.symmetric(p(*i)?, p(*j)?, l(*k)?),
                ConstraintKind::Fix => sk.fix(p(*i)?, *value, *value),
                ConstraintKind::Distance => sk.distance(p(*i)?, p(*j)?, *value),
                ConstraintKind::PointLineDistance => sk.point_line_distance(p(*i)?, l(*j)?, *value),
                ConstraintKind::Radius => sk.radius(r(*i)?, *value),
                ConstraintKind::Angle => sk.angle(l(*i)?, l(*j)?, *value),
            }
            .ok()?;
            built.constraints.push(id);
        }
        Op::Fixed(i, y) => {
            let id = pick(&built.entities, *i)?;
            let coordinate = match sk.entity(id)?.kind() {
                EntityKind::Point if *y => Coordinate::Y,
                EntityKind::Point => Coordinate::X,
                EntityKind::Circle => Coordinate::R,
                EntityKind::Line | EntityKind::Arc => return None,
            };
            sk.set_fixed(id, coordinate, true).ok()?;
        }
        Op::Construction(i) => {
            sk.set_construction(pick(&built.entities, *i)?, true).ok()?;
        }
        Op::Tag(i, t) => sk.tag(pick(&built.entities, *i)?, t).ok()?,
        Op::Payload(i, bytes) => sk
            .payload(pick(&built.entities, *i)?, Payload(bytes.clone()))
            .ok()?,
        Op::ConstraintTag(i, t) => sk.tag_constraint(pick(&built.constraints, *i)?, t).ok()?,
        Op::ConstraintPayload(i, bytes) => sk
            .payload_constraint(pick(&built.constraints, *i)?, Payload(bytes.clone()))
            .ok()?,
    }
    Some(())
}

/// Well-formed sketches of up to `max_ops` builder operations: points,
/// lines, circles, arcs, every constraint kind, fixed parameters,
/// construction flags, tags and payloads.
///
/// ```
/// use laman_debug::strategy::sketch;
/// use proptest::prelude::*;
/// use proptest::strategy::ValueTree;
/// use proptest::test_runner::TestRunner;
///
/// let mut runner = TestRunner::deterministic();
/// let drawn = sketch(20).new_tree(&mut runner).unwrap().current();
/// assert!(drawn.revision() <= 20 + 40);
/// ```
pub fn sketch(max_ops: usize) -> impl Strategy<Value = Sketch> {
    vec(op(), 0..=max_ops).prop_map(|ops| {
        let (mut sk, mut built) = (Sketch::new(), Built::default());
        for op in &ops {
            apply(&mut sk, &mut built, op);
        }
        sk
    })
}

/// Whether the sketch holds an entity of this kind; a strategy test uses it
/// to know the draw reached a kind.
///
/// ```
/// use laman::model::{EntityKind, Sketch};
/// use laman_debug::strategy::has_kind;
/// let mut sk = Sketch::new();
/// sk.point(0.0, 0.0).unwrap();
/// assert!(has_kind(&sk, EntityKind::Point));
/// assert!(!has_kind(&sk, EntityKind::Arc));
/// ```
pub fn has_kind(sketch: &Sketch, kind: EntityKind) -> bool {
    sketch
        .entities()
        .any(|(_, e): (_, &Entity)| e.kind() == kind)
}
