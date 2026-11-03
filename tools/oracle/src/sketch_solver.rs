//! `sketch-solver` 0.1.0 on a C1 fixture: `translate` laman's sketch into
//! its `Sketch`, prove the translation at the expected coordinates, solve
//! from the fixture's guesses, and `diagnose` its answer into a report.

use std::collections::{BTreeMap, BTreeSet};

use laman::model::{
    Constraint as L, ConstraintId, Coordinate, Entity as LEntity, EntityId, Round, Span,
};
use sketch_solver::{
    Along, Constraint as S, EntityId as SId, Pair, Sketch, Solution, SolveOptions, Source,
    Status as SStatus,
};

use crate::fixture::Fixture;
use crate::report::{Outcome, Report, Status, Unsupported};

const SOLVER: &str = "sketch-solver";

/// How far the translated equations may be from zero at the expected
/// coordinates, and how far the solver may move a coordinate off them, in
/// sketch units. The C1 expectations are written to 13 digits on sketches of
/// size ~100; a wrong equation is off by whole units.
const TRANSLATION_TOLERANCE: f64 = 1e-9;

/// A laman sketch in `sketch-solver`'s terms.
struct Translation {
    sketch: Sketch,
    /// `sketch-solver`'s id of each laman entity.
    ids: BTreeMap<EntityId, SId>,
    /// The laman constraint behind each `sketch-solver` constraint, in order.
    /// (`None` for the `Fix` that a fixed point becomes).
    constraints: Vec<Option<ConstraintId>>,
    unsupported: Vec<Unsupported>,
}

/// Translate the fixture's sketch, every point and circle guessed at the
/// value `guess` gives it, leaving out the constraints in `skip`.
fn translate(
    fx: &Fixture,
    guess: &laman::model::Coordinates,
    skip: &BTreeSet<ConstraintId>,
) -> Result<Translation, String> {
    let at = |id: EntityId, c: Coordinate| guess.get(&(id, c)).copied();
    let mut t = Translation {
        sketch: Sketch::new(),
        ids: BTreeMap::new(),
        constraints: Vec::new(),
        unsupported: Vec::new(),
    };
    let model = |e: sketch_solver::ModelError| format!("sketch-solver refused the sketch: {e:?}");
    let mut fixes = Vec::new();
    for (id, e) in fx.sketch.entities() {
        let lookup = |t: &Translation, p: EntityId| {
            t.ids
                .get(&p)
                .copied()
                .ok_or_else(|| format!("{p} is used before it is defined"))
        };
        let sid = match e {
            LEntity::Point { .. } => {
                let (x, y) = (at(id, Coordinate::X), at(id, Coordinate::Y));
                let sid = t
                    .sketch
                    .point(x.zip(y).map(|(x, y)| [x, y]))
                    .map_err(model)?;
                for c in [Coordinate::X, Coordinate::Y] {
                    if fx.sketch.is_fixed(id, c) {
                        fixes.push((id, c, sid));
                    }
                }
                sid
            }
            LEntity::Line { a, b } => {
                let (a, b) = (lookup(&t, a.id())?, lookup(&t, b.id())?);
                t.sketch.line(a, b).map_err(model)?
            }
            LEntity::Circle { c, .. } => {
                let c = lookup(&t, c.id())?;
                t.sketch.circle(c, at(id, Coordinate::R)).map_err(model)?
            }
            // Counter-clockwise from `s` to `e`, as laman's.
            LEntity::Arc { c, s, e } => {
                let (c, s, e) = (
                    lookup(&t, c.id())?,
                    lookup(&t, s.id())?,
                    lookup(&t, e.id())?,
                );
                t.sketch.arc(c, s, e, false).map_err(model)?
            }
        };
        if fx.sketch.is_construction(id) {
            t.sketch.set_construction(sid, true).map_err(model)?;
        }
        t.ids.insert(id, sid);
    }
    // A point fixed in both coordinates is `Fix`; one fixed in a single
    // coordinate (or a fixed radius) has no counterpart.
    let mut by_point: BTreeMap<EntityId, Vec<Coordinate>> = BTreeMap::new();
    for (id, c, _) in &fixes {
        by_point.entry(*id).or_default().push(*c);
    }
    for (id, cs) in by_point {
        if cs.len() == 2 {
            let sid = t.ids[&id];
            t.sketch
                .add(S::Fix {
                    entity: sid,
                    at: None,
                })
                .map_err(model)?;
            t.constraints.push(None);
        } else {
            t.unsupported.push(Unsupported {
                kind: "fixed coordinate".into(),
                constraint: format!("point {}", id.0),
                approximation: None,
            });
        }
    }
    for (id, c) in fx.sketch.constraints() {
        if skip.contains(&id) {
            continue;
        }
        let e = |x: EntityId| t.ids[&x];
        let round = |r: Round| match r {
            Round::Circle(c) => t.ids[&c.id()],
            Round::Arc(a) => t.ids[&a.id()],
        };
        let pair = |s: Span| match s {
            Span::Line(l) => Pair::Line(t.ids[&l.id()]),
            Span::Points(a, b) => Pair::Points(t.ids[&a.id()], t.ids[&b.id()]),
        };
        let s = match *c {
            L::Coincident { p, q } => S::Coincident(e(p.id()), e(q.id())),
            L::PointOnLine { p, line } => S::On {
                point: e(p.id()),
                curve: e(line.id()),
            },
            L::PointOnCircle { p, on } => S::On {
                point: e(p.id()),
                curve: round(on),
            },
            L::Horizontal(s) => S::Horizontal(pair(s)),
            L::Vertical(s) => S::Vertical(pair(s)),
            L::Parallel { a, b } => S::Parallel(e(a.id()), e(b.id())),
            L::Perpendicular { a, b } => S::Perpendicular(e(a.id()), e(b.id())),
            L::EqualLength { a, b } => S::Equal(e(a.id()), e(b.id())),
            L::EqualRadius { a, b } => S::Equal(round(a), round(b)),
            // The side comes from the drawing in sketch-solver; laman's
            // `sense` is not passed. The residual proof checks they agree.
            L::TangentLineCircle { line, round: r, .. } => S::Tangent(e(line.id()), round(r)),
            L::TangentCircles { a, b, .. } => S::Tangent(round(a), round(b)),
            L::Midpoint { p, line } => S::Midpoint {
                point: e(p.id()),
                line: e(line.id()),
            },
            L::Symmetric { p, q, axis } => S::Symmetric {
                a: e(p.id()),
                b: e(q.id()),
                about: e(axis.id()),
            },
            L::Fix { p, x, y } => S::Fix {
                entity: e(p.id()),
                at: Some([x, y]),
            },
            L::Distance { p, q, value, .. } => S::Distance {
                a: e(p.id()),
                b: e(q.id()),
                value,
                along: Along::Direct,
            },
            // To the infinite line, on the side the drawing shows.
            L::PointLineDistance { p, line, value, .. } => S::Distance {
                a: e(p.id()),
                b: e(line.id()),
                value,
                along: Along::Direct,
            },
            L::Radius { round: r, value } => S::Radius {
                curve: round(r),
                value,
            },
            L::Angle { a, b, value } => S::Angle {
                from: e(a.id()),
                to: e(b.id()),
                degrees: value.to_degrees(),
            },
        };
        t.sketch.add(s).map_err(model)?;
        t.constraints.push(Some(id));
    }
    Ok(t)
}

/// What a `sketch-solver` source is called in laman's terms.
fn name(fx: &Fixture, t: &Translation, s: Source) -> String {
    match s {
        Source::Constraint(c) => match t.constraints.get(c.index()) {
            Some(Some(id)) => fx.constraint_name(*id),
            Some(None) => format!("fixed point (constraint {})", c.index()),
            None => format!("sketch-solver constraint {}", c.index()),
        },
        Source::Arc(a) => match t.ids.iter().find(|(_, sid)| **sid == a) {
            Some((id, _)) => fx.arc_name(*id),
            None => format!("arc {}", a.index()),
        },
    }
}

/// Run `sketch-solver` on one fixture.
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
        // The translation proof: at the expected coordinates the solver's
        // own residuals are zero, except exactly where the checker finds
        // laman's constraints violated (a conflicting twin's).
        let proof = translate(fx, &expected, &violated)?;
        if let Some(u) = proof.unsupported.first() {
            let mut r = fail(
                Outcome::Unsupported,
                format!("{} has no translation", u.kind),
            );
            r.unsupported = proof.unsupported;
            return Ok(r);
        }
        let hold = proof.sketch.solve_with(&SolveOptions {
            max_iterations: 0,
            ..SolveOptions::default()
        });
        let hold = hold.map_err(|e| format!("{e}"))?;
        let moved = moved_by(fx, &proof, &hold, &expected);
        if hold.residual > TRANSLATION_TOLERANCE || moved > TRANSLATION_TOLERANCE {
            return Ok(fail(
                Outcome::Error,
                format!(
                    "translation not proven: residual {:e}, moved {:e} at the expected coordinates (unmet: {:?})",
                    hold.residual,
                    moved,
                    hold.unmet
                        .iter()
                        .map(|(s, r)| (name(fx, &proof, *s), *r))
                        .collect::<Vec<_>>()
                ),
            ));
        }
        // And the whole translation fails exactly where the checker does.
        let whole = translate(fx, &expected, &BTreeSet::new())?;
        let all = whole.sketch.solve_with(&SolveOptions {
            max_iterations: 0,
            ..SolveOptions::default()
        });
        let all = all.map_err(|e| format!("{e}"))?;
        let unmet: BTreeSet<String> = all
            .unmet
            .iter()
            .map(|(s, _)| name(fx, &whole, *s))
            .collect();
        let want: BTreeSet<String> = violated.iter().map(|id| fx.constraint_name(*id)).collect();
        if unmet != want {
            return Ok(fail(
                Outcome::Error,
                format!(
                    "translation disagrees with the checker: solver unmet {unmet:?}, checker violated {want:?}"
                ),
            ));
        }
        // The run: from the fixture's own guesses.
        let guesses = fx.sketch.coordinates();
        let t = translate(fx, &guesses, &BTreeSet::new())?;
        let sol = t.sketch.solve();
        let mut r = diagnose(fx, &t, &sol);
        r.translation_residual = Some(hold.residual);
        r.translation_unchecked = want.into_iter().collect();
        Ok(r)
    };
    go().unwrap_or_else(|e| fail(Outcome::Error, e))
}

/// The largest distance between the solver's coordinates and the expected
/// ones (the solver is allowed no movement from a solved start).
fn moved_by(
    fx: &Fixture,
    t: &Translation,
    sol: &Solution,
    want: &laman::model::Coordinates,
) -> f64 {
    let mut worst = 0.0f64;
    for (id, e) in fx.sketch.entities() {
        let Some(sid) = t.ids.get(&id) else { continue };
        match e {
            LEntity::Point { .. } => {
                if let Some([x, y]) = sol.point(*sid) {
                    for (c, v) in [(Coordinate::X, x), (Coordinate::Y, y)] {
                        if let Some(w) = want.get(&(id, c)) {
                            worst = worst.max((v - w).abs());
                        }
                    }
                }
            }
            LEntity::Circle { .. } => {
                if let (Some(v), Some(w)) = (sol.radius(*sid), want.get(&(id, Coordinate::R))) {
                    worst = worst.max((v - w).abs());
                }
            }
            LEntity::Line { .. } | LEntity::Arc { .. } => {}
        }
    }
    worst
}

/// `sketch-solver`'s answer in laman's words.
fn diagnose(fx: &Fixture, t: &Translation, sol: &Solution) -> Report {
    let n = |s: Source| name(fx, t, s);
    let mut r = Report::without_numbers(SOLVER, &fx.name, Outcome::Solved, "");
    r.detail = None;
    r.unsupported = t.unsupported.clone();
    r.solver_status = Some(format!("{:?}", sol.status));
    r.dof = u32::try_from(sol.dof).ok();
    r.status = Some(match (sol.status, sol.dof, sol.conflicts.is_empty()) {
        (SStatus::Solved, 0, _) => Status::FullyConstrained,
        (SStatus::Solved, _, _) => Status::UnderConstrained,
        (SStatus::NotConverged, _, false) => Status::Unsatisfiable,
        (SStatus::NotConverged, _, true) => Status::NotConverged,
    });
    r.redundant = Some(sol.redundant.iter().map(|d| n(d.source)).collect());
    r.conflicts = Some(
        sol.conflicts
            .iter()
            .map(|d| d.with.iter().copied().chain([d.source]).map(n).collect())
            .collect(),
    );
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

    /// The residual proof is live: the conflicting twin's `apex`, translated
    /// and left in, is far from zero at the expected coordinates.
    #[test]
    fn the_residual_proof_sees_a_violated_constraint() {
        let fx = load("sss-triangle-conflicting");
        let expected = fx.expected_coordinates().expect("coordinates");
        let t = translate(&fx, &expected, &BTreeSet::new()).expect("translates");
        let opt = SolveOptions {
            max_iterations: 0,
            ..SolveOptions::default()
        };
        let sol = t.sketch.solve_with(&opt).expect("solves");
        assert!(sol.residual > 1.0, "residual {}", sol.residual);
    }
}
