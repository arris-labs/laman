//! `ezpz` 0.2.29 on a C1 fixture: `translate` laman's sketch into its
//! constraint requests, prove the translation at the expected coordinates,
//! solve from the fixture's guesses, and `diagnose` its answer into a report.
//!
//! `ezpz` names no degrees of freedom and no sets: it says which constraint
//! indices stay unsatisfied and which variables are underconstrained, and the
//! report keeps exactly that.

use std::collections::{BTreeMap, BTreeSet};

use ezpz::datatypes::inputs::{
    DatumCircle, DatumCircularArc, DatumDistance, DatumLineSegment, DatumPoint,
};
use ezpz::datatypes::{Angle, AngleKind};
use ezpz::{
    CircleSide, Config, Constraint as S, ConstraintRequest, Id, IdGenerator, LineSide,
    SolveOutcomeFreedomAnalysis, solve_analysis,
};
use laman::model::{
    Constraint as L, ConstraintId, Contact, Coordinate, Coordinates, Entity as LEntity, EntityId,
    Round, Span,
};

use crate::fixture::Fixture;
use crate::report::{Outcome, Report, Status, Unsupported};

const SOLVER: &str = "ezpz";

/// How far the translated equations may be from zero at the expected
/// coordinates, and how far the solver may move a coordinate off them, in
/// sketch units. The C1 expectations are written to 13 digits on sketches of
/// size ~100; a wrong equation is off by whole units. Also handed to the
/// solver as its convergence tolerance, so that "satisfied" means within it.
const TRANSLATION_TOLERANCE: f64 = 1e-9;

/// What a request of the translation is, in laman's terms.
#[derive(Clone, Copy, Debug)]
enum Origin {
    /// A laman constraint (a `fix` is two requests of the same one).
    Constraint(ConstraintId),
    /// The two coordinates of a point fixed in both.
    FixedPoint,
    /// An arc's own equation, or the circle built from it.
    Arc(EntityId),
}

/// A laman sketch as `ezpz` requests.
struct Translation {
    requests: Vec<ConstraintRequest>,
    /// Behind each request, in order.
    origins: Vec<Origin>,
    /// One guess per variable.
    guesses: Vec<(Id, f64)>,
    /// The point variables behind each laman point, to read an answer back.
    points: BTreeMap<EntityId, DatumPoint>,
    /// The radius variable behind each laman circle.
    radii: BTreeMap<EntityId, DatumDistance>,
    unsupported: Vec<Unsupported>,
}

impl Translation {
    fn push(&mut self, c: S, origin: Origin) {
        self.requests.push(ConstraintRequest::highest_priority(c));
        self.origins.push(origin);
    }
}

/// Translate the fixture's sketch, every point and circle guessed at the
/// value `guess` gives it, leaving out the constraints in `skip`.
fn translate(
    fx: &Fixture,
    guess: &Coordinates,
    skip: &BTreeSet<ConstraintId>,
) -> Result<Translation, String> {
    let at = |id: EntityId, c: Coordinate| {
        guess
            .get(&(id, c))
            .copied()
            .ok_or_else(|| format!("no value for {} {c:?}", id.0))
    };
    let mut ids = IdGenerator::default();
    let mut t = Translation {
        requests: Vec::new(),
        origins: Vec::new(),
        guesses: Vec::new(),
        points: BTreeMap::new(),
        radii: BTreeMap::new(),
        unsupported: Vec::new(),
    };
    let mut lines: BTreeMap<EntityId, DatumLineSegment> = BTreeMap::new();
    let mut circles: BTreeMap<EntityId, DatumCircle> = BTreeMap::new();
    let mut arcs: BTreeMap<EntityId, DatumCircularArc> = BTreeMap::new();
    let mut fixes: Vec<(EntityId, DatumPoint)> = Vec::new();
    let point = |t: &Translation, p: EntityId| {
        t.points
            .get(&p)
            .copied()
            .ok_or_else(|| format!("{} is used before it is defined", p.0))
    };
    for (id, e) in fx.sketch.entities() {
        match e {
            LEntity::Point { .. } => {
                let p = DatumPoint::new(&mut ids);
                t.guesses.push((p.id_x(), at(id, Coordinate::X)?));
                t.guesses.push((p.id_y(), at(id, Coordinate::Y)?));
                t.points.insert(id, p);
                let (fix_x, fix_y) = (
                    fx.sketch.is_fixed(id, Coordinate::X),
                    fx.sketch.is_fixed(id, Coordinate::Y),
                );
                if fix_x && fix_y {
                    fixes.push((id, p));
                } else if fix_x || fix_y {
                    t.unsupported.push(Unsupported {
                        kind: "fixed coordinate".into(),
                        constraint: format!("point {}", id.0),
                        approximation: None,
                    });
                }
            }
            LEntity::Line { a, b } => {
                let seg = DatumLineSegment::new(point(&t, a.id())?, point(&t, b.id())?);
                lines.insert(id, seg);
            }
            LEntity::Circle { c, .. } => {
                let radius = DatumDistance::new(ids.next_id());
                t.guesses.push((radius.id, at(id, Coordinate::R)?));
                t.radii.insert(id, radius);
                circles.insert(
                    id,
                    DatumCircle {
                        center: point(&t, c.id())?,
                        radius,
                    },
                );
            }
            // Counter-clockwise from `s` to `e`, as laman's, plus the one
            // equation that makes it an arc.
            LEntity::Arc { c, s, e } => {
                let arc = DatumCircularArc {
                    center: point(&t, c.id())?,
                    start: point(&t, s.id())?,
                    end: point(&t, e.id())?,
                };
                arcs.insert(id, arc);
                t.push(S::Arc(arc), Origin::Arc(id));
            }
        }
    }
    for (id, p) in fixes {
        for (v, c) in [(p.id_x(), Coordinate::X), (p.id_y(), Coordinate::Y)] {
            let value = fx
                .sketch
                .coordinates()
                .get(&(id, c))
                .copied()
                .ok_or("a fixed point without a value")?;
            t.push(S::Fixed(v, value), Origin::FixedPoint);
        }
    }
    // An arc as a circle (`ezpz` has no arc with a radius to constrain
    // against a circle's): one new radius unknown tied to the start point by
    // one new equation, so the degrees of freedom do not change.
    let mut built: BTreeMap<EntityId, DatumCircle> = BTreeMap::new();
    #[allow(
        clippy::disallowed_methods,
        reason = "the oracle is not the solver: no determinism claim"
    )]
    let mut round = |t: &mut Translation, ids: &mut IdGenerator, r: Round| match r {
        Round::Circle(c) => circles
            .get(&c.id())
            .copied()
            .ok_or_else(|| format!("circle {} is not defined", c.id().0)),
        Round::Arc(a) => {
            let id = a.id();
            if let Some(c) = built.get(&id) {
                return Ok(*c);
            }
            let arc = arcs
                .get(&id)
                .copied()
                .ok_or_else(|| format!("arc {} is not defined", id.0))?;
            let radius = DatumDistance::new(ids.next_id());
            let dx =
                at_point(&t.guesses, arc.start.id_x())? - at_point(&t.guesses, arc.center.id_x())?;
            let dy =
                at_point(&t.guesses, arc.start.id_y())? - at_point(&t.guesses, arc.center.id_y())?;
            t.guesses.push((radius.id, dx.hypot(dy)));
            let circle = DatumCircle {
                center: arc.center,
                radius,
            };
            t.push(
                S::DistanceVar(arc.start, arc.center, radius),
                Origin::Arc(id),
            );
            built.insert(id, circle);
            Ok(circle)
        }
    };
    for (id, c) in fx.sketch.constraints() {
        if skip.contains(&id) {
            continue;
        }
        let o = Origin::Constraint(id);
        let line = |l: laman::model::Line| {
            lines
                .get(&l.id())
                .copied()
                .ok_or_else(|| format!("line {} is not defined", l.id().0))
        };
        let span = |s: Span, t: &Translation| match s {
            Span::Line(l) => line(l),
            Span::Points(a, b) => Ok(DatumLineSegment::new(point(t, a.id())?, point(t, b.id())?)),
        };
        match *c {
            L::Coincident { p, q } => {
                let c = S::PointsCoincident(point(&t, p.id())?, point(&t, q.id())?);
                t.push(c, o);
            }
            L::PointOnLine { .. } => t.unsupported.push(Unsupported {
                kind: "point_on_line".into(),
                constraint: fx.constraint_name(id),
                approximation: None,
            }),
            // Distance from the point to the centre, tied to the radius.
            L::PointOnCircle { p, on } => {
                let circle = round(&mut t, &mut ids, on)?;
                let c = S::DistanceVar(point(&t, p.id())?, circle.center, circle.radius);
                t.push(c, o);
            }
            L::Horizontal(s) => {
                let c = S::Horizontal(span(s, &t)?);
                t.push(c, o);
            }
            L::Vertical(s) => {
                let c = S::Vertical(span(s, &t)?);
                t.push(c, o);
            }
            L::Parallel { a, b } => {
                t.push(S::LinesAtAngle(line(a)?, line(b)?, AngleKind::Parallel), o);
            }
            L::Perpendicular { a, b } => {
                t.push(
                    S::LinesAtAngle(line(a)?, line(b)?, AngleKind::Perpendicular),
                    o,
                );
            }
            L::EqualLength { a, b } => t.push(S::LinesEqualLength(line(a)?, line(b)?), o),
            L::EqualRadius { a, b } => {
                let (a, b) = (round(&mut t, &mut ids, a)?, round(&mut t, &mut ids, b)?);
                t.push(S::ScalarEqual(a.radius.id, b.radius.id), o);
            }
            // The side is inferred from the guesses, as laman's from the
            // drawing; `sense` is not passed. The residual proof checks they
            // agree.
            L::TangentLineCircle {
                line: l, round: r, ..
            } => {
                let circle = round(&mut t, &mut ids, r)?;
                t.push(
                    S::LineTangentToCircle(line(l)?, circle, LineSide::Undefined),
                    o,
                );
            }
            L::TangentCircles { a, b, contact } => {
                let (a, b) = (round(&mut t, &mut ids, a)?, round(&mut t, &mut ids, b)?);
                let side = match contact {
                    Contact::External => CircleSide::Exterior,
                    Contact::Internal(_) => CircleSide::Interior,
                };
                t.push(S::CircleTangentToCircle(a, b, side), o);
            }
            L::Midpoint { p, line: l } => t.push(S::Midpoint(line(l)?, point(&t, p.id())?), o),
            L::Symmetric { p, q, axis } => {
                let c = S::Symmetric(line(axis)?, point(&t, p.id())?, point(&t, q.id())?);
                t.push(c, o);
            }
            L::Fix { p, x, y } => {
                let p = point(&t, p.id())?;
                t.push(S::Fixed(p.id_x(), x), o);
                t.push(S::Fixed(p.id_y(), y), o);
            }
            L::Distance { p, q, value, .. } => {
                let c = S::Distance(point(&t, p.id())?, point(&t, q.id())?, value);
                t.push(c, o);
            }
            // Perpendicular distance to the line.
            L::PointLineDistance {
                p, line: l, value, ..
            } => {
                t.push(S::PointLineDistance(point(&t, p.id())?, line(l)?, value), o);
            }
            L::Radius { round: r, value } => match r {
                Round::Circle(c) => {
                    let circle = circles
                        .get(&c.id())
                        .copied()
                        .ok_or_else(|| format!("circle {} is not defined", c.id().0))?;
                    t.push(S::CircleRadius(circle, value), o);
                }
                Round::Arc(a) => {
                    let arc = arcs
                        .get(&a.id())
                        .copied()
                        .ok_or_else(|| format!("arc {} is not defined", a.id().0))?;
                    t.push(S::ArcRadius(arc, value), o);
                }
            },
            L::Angle { a, b, value } => {
                let kind = AngleKind::Other(Angle::from_radians(value));
                t.push(S::LinesAtAngle(line(a)?, line(b)?, kind), o);
            }
        }
    }
    Ok(t)
}

/// A variable's guess, among those pushed so far.
fn at_point(guesses: &[(Id, f64)], id: Id) -> Result<f64, String> {
    guesses
        .iter()
        .find(|(i, _)| *i == id)
        .map(|(_, v)| *v)
        .ok_or_else(|| format!("no guess for variable {id}"))
}

/// What a request is called in laman's terms.
fn name(fx: &Fixture, t: &Translation, index: usize) -> String {
    match t.origins.get(index) {
        Some(Origin::Constraint(id)) => fx.constraint_name(*id),
        Some(Origin::FixedPoint) => format!("fixed point (request {index})"),
        Some(Origin::Arc(id)) => fx.arc_name(*id),
        None => format!("ezpz request {index}"),
    }
}

fn config() -> Config {
    Config::default().with_convergence_tolerance(TRANSLATION_TOLERANCE)
}

fn solve(t: &Translation) -> Result<SolveOutcomeFreedomAnalysis, String> {
    solve_analysis(&t.requests, t.guesses.clone(), config())
        .map_err(|f| format!("ezpz refused the system: {}", f.error))
}

/// The largest distance between the solver's answer and the guesses it was
/// started from, over every variable.
fn moved_by(t: &Translation, out: &ezpz::SolveOutcome) -> f64 {
    let mut worst = 0.0f64;
    for (id, v) in &t.guesses {
        let got = out.final_values().get(*id as usize).copied();
        worst = worst.max(got.map_or(f64::INFINITY, |g| (g - v).abs()));
    }
    worst
}

/// Run `ezpz` on one fixture.
pub fn run(fx: &Fixture) -> Report {
    let fail = |outcome, detail: String| Report::without_numbers(SOLVER, &fx.name, outcome, detail);
    let go = || -> Result<Report, String> {
        let expected = fx.expected_coordinates()?;
        let violated = fx.violated_at_expected()?;
        if !violated.is_empty() && fx.expected["status"] != "Unsatisfiable" {
            return Ok(fail(
                Outcome::Error,
                format!(
                    "the checker finds the expected coordinates violate {:?}",
                    violated
                        .iter()
                        .map(|id| fx.constraint_name(*id))
                        .collect::<Vec<_>>()
                ),
            ));
        }
        // The translation proof: started at the expected coordinates, the
        // solver finds every equation within its tolerance and moves nothing,
        // except exactly where the checker finds laman's constraints violated
        // (a conflicting twin's). `ezpz` cannot be asked for one residual, so
        // the number reported is how far it moved.
        let proof = translate(fx, &expected, &violated)?;
        if let Some(u) = proof.unsupported.first() {
            let mut r = fail(
                Outcome::Unsupported,
                format!("{} has no translation", u.kind),
            );
            r.unsupported = proof.unsupported;
            return Ok(r);
        }
        let hold = solve(&proof)?;
        let moved = moved_by(&proof, &hold.outcome);
        if !hold.outcome.unsatisfied().is_empty() || moved > TRANSLATION_TOLERANCE {
            return Ok(fail(
                Outcome::Error,
                format!(
                    "translation not proven: moved {moved:e} at the expected coordinates (unsatisfied: {:?})",
                    hold.outcome
                        .unsatisfied()
                        .iter()
                        .map(|i| name(fx, &proof, *i))
                        .collect::<Vec<_>>()
                ),
            ));
        }
        // And each violated constraint, put back alone, is unsatisfied: the
        // solver finds violated what the checker does.
        for v in &violated {
            let mut skip = violated.clone();
            skip.remove(v);
            let one = translate(fx, &expected, &skip)?;
            let unmet: BTreeSet<String> = match solve(&one) {
                Ok(o) => o
                    .outcome
                    .unsatisfied()
                    .iter()
                    .map(|i| name(fx, &one, *i))
                    .collect(),
                // A refusal is not a pass.
                Err(_) => BTreeSet::from([fx.constraint_name(*v)]),
            };
            if unmet.is_empty() {
                return Ok(fail(
                    Outcome::Error,
                    format!(
                        "translation disagrees with the checker: {} holds for the solver",
                        fx.constraint_name(*v)
                    ),
                ));
            }
        }
        // The run: from the fixture's own guesses.
        let guesses = fx.sketch.coordinates();
        let t = translate(fx, &guesses, &BTreeSet::new())?;
        let mut r = diagnose(
            fx,
            &t,
            solve_analysis(&t.requests, t.guesses.clone(), config()),
        );
        r.translation_residual = Some(moved);
        r.translation_unchecked = violated.iter().map(|id| fx.constraint_name(*id)).collect();
        Ok(r)
    };
    go().unwrap_or_else(|e| fail(Outcome::Error, e))
}

/// `ezpz`'s answer in laman's words: DoF, redundant and conflict sets are
/// `null` (it names none); the unsatisfied constraints are kept in the
/// solver's own status.
fn diagnose(
    fx: &Fixture,
    t: &Translation,
    ans: Result<SolveOutcomeFreedomAnalysis, ezpz::FailureOutcome>,
) -> Report {
    let mut r = Report::without_numbers(SOLVER, &fx.name, Outcome::Solved, "");
    r.detail = None;
    r.unsupported = t.unsupported.clone();
    match ans {
        Err(f) => r.solver_status = Some(format!("failed: {}", f.error)),
        Ok(a) if a.outcome.is_satisfied() => {
            r.solver_status = Some("satisfied".into());
            r.status = Some(if a.analysis.is_underconstrained() {
                Status::UnderConstrained
            } else {
                Status::FullyConstrained
            });
        }
        // Not satisfied is one word for a conflict and for not converging.
        Ok(a) => {
            let mut unmet: Vec<String> = Vec::new();
            for n in a.outcome.unsatisfied().iter().map(|i| name(fx, t, *i)) {
                if !unmet.contains(&n) {
                    unmet.push(n);
                }
            }
            r.solver_status = Some(format!("unsatisfied: {}", unmet.join(", ")));
        }
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn load(name: &str) -> Fixture {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/c1")
            .join(name);
        crate::fixture::load(&dir).expect("fixture loads")
    }

    /// Every C1 sketch's translation is proven at the expected coordinates,
    /// and the checker and the solver agree on what is violated there.
    #[test]
    fn all_ten_translations_are_proven() {
        for name in [
            "belt",
            "belt-conflicting",
            "belt-redundant",
            "slotted-plate",
            "slotted-plate-conflicting",
            "slotted-plate-drag",
            "slotted-plate-redundant",
            "sss-triangle",
            "sss-triangle-conflicting",
            "sss-triangle-redundant",
        ] {
            let r = run(&load(name));
            assert_eq!(r.outcome, Outcome::Solved, "{name}: {:?}", r.detail);
            assert!(
                r.translation_residual
                    .is_some_and(|x| x <= TRANSLATION_TOLERANCE),
                "{name}"
            );
        }
    }

    /// A coordinate moved off the construction is refused: the expectation
    /// is wrong, and no numbers come of it.
    #[test]
    fn a_wrong_expectation_is_an_error_not_numbers() {
        let mut fx = load("sss-triangle");
        fx.expected["coordinates"][0]["value"] = serde_json::json!(41.0);
        let r = run(&fx);
        assert_eq!(r.outcome, Outcome::Error);
        assert!(r.status.is_none() && r.dof.is_none());
    }

    /// The proof is live: the conflicting twin's `apex`, translated and left
    /// in, is unsatisfied at the expected coordinates.
    #[test]
    fn the_proof_sees_a_violated_constraint() {
        let fx = load("sss-triangle-conflicting");
        let expected = fx.expected_coordinates().expect("coordinates");
        let t = translate(&fx, &expected, &BTreeSet::new()).expect("translates");
        let unmet = match solve(&t) {
            Ok(o) => !o.outcome.is_satisfied(),
            Err(_) => true,
        };
        assert!(unmet);
    }
}
