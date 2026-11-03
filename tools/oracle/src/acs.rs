//! `acs-solver` 0.1.19 on a C1 fixture: `translate` laman's sketch into its
//! JSON request, prove the translation at the expected coordinates, solve
//! from the fixture's guesses, and `diagnose` its answer into a report.

use std::collections::{BTreeMap, BTreeSet};

use laman::model::{
    Constraint as L, ConstraintId, Contact, Coordinate, Coordinates, Entity as LEntity, EntityId,
    Round, Span,
};
use serde_json::{Value, json};

use crate::fixture::Fixture;
use crate::report::{Outcome, Report, Status, Unsupported};

const SOLVER: &str = "acs-solver";

/// How far the translated equations may be from zero at the expected
/// coordinates, and how far the solver may move a coordinate off them, in
/// sketch units. The C1 expectations are written to 13 digits on sketches of
/// size ~100; a wrong equation is off by whole units.
const TRANSLATION_TOLERANCE: f64 = 1e-9;

/// A laman sketch as `acs-solver`'s request.
struct Translation {
    request: Value,
    /// The laman constraint behind each constraint id of the request.
    constraints: BTreeMap<String, ConstraintId>,
    unsupported: Vec<Unsupported>,
}

fn eid(id: EntityId) -> String {
    format!("e{}", id.0)
}

fn kid(id: ConstraintId) -> String {
    format!("k{}", id.0)
}

#[allow(
    clippy::disallowed_methods,
    reason = "the oracle is not the solver: no determinism claim"
)]
fn angle_of(guess: &Coordinates, c: EntityId, p: EntityId) -> Option<(f64, f64)> {
    let at = |id: EntityId, k: Coordinate| guess.get(&(id, k)).copied();
    let dx = at(p, Coordinate::X)? - at(c, Coordinate::X)?;
    let dy = at(p, Coordinate::Y)? - at(c, Coordinate::Y)?;
    Some((dx.hypot(dy), dy.atan2(dx)))
}

/// Translate the fixture's sketch, every point and circle guessed at the
/// value `guess` gives it, leaving out the constraints in `skip`, with at most
/// `max_iterations` solver iterations.
fn translate(
    fx: &Fixture,
    guess: &Coordinates,
    skip: &BTreeSet<ConstraintId>,
    max_iterations: u32,
) -> Result<Translation, String> {
    let at = |id: EntityId, c: Coordinate| guess.get(&(id, c)).copied();
    let mut t = Translation {
        request: Value::Null,
        constraints: BTreeMap::new(),
        unsupported: Vec::new(),
    };
    let mut prims: Vec<Value> = Vec::new();
    for (id, e) in fx.sketch.entities() {
        let mut p = match e {
            LEntity::Point { .. } => {
                let (fx_, fy) = (
                    fx.sketch.is_fixed(id, Coordinate::X),
                    fx.sketch.is_fixed(id, Coordinate::Y),
                );
                if fx_ != fy {
                    t.unsupported.push(Unsupported {
                        kind: "fixed coordinate".into(),
                        constraint: format!("point {}", id.0),
                        approximation: None,
                    });
                }
                json!({
                    "type": "point", "id": eid(id),
                    "x": at(id, Coordinate::X), "y": at(id, Coordinate::Y),
                    "fixed": fx_ && fy,
                })
            }
            LEntity::Line { a, b } => json!({
                "type": "line", "id": eid(id), "p1_id": eid(a.id()), "p2_id": eid(b.id()),
            }),
            LEntity::Circle { c, .. } => json!({
                "type": "circle", "id": eid(id), "c_id": eid(c.id()),
                "radius": at(id, Coordinate::R),
            }),
            // Counter-clockwise from `s` to `e`, as laman's; the radius and
            // the two angles are the arc's variables in acs-solver.
            LEntity::Arc { c, s, e } => {
                let (r, a0) = angle_of(guess, c.id(), s.id())
                    .ok_or_else(|| format!("arc {} has no coordinates", id.0))?;
                let (_, a1) = angle_of(guess, c.id(), e.id())
                    .ok_or_else(|| format!("arc {} has no coordinates", id.0))?;
                json!({
                    "type": "arc", "id": eid(id), "c_id": eid(c.id()),
                    "start_id": eid(s.id()), "end_id": eid(e.id()),
                    "radius": r, "start_angle": a0, "end_angle": a1,
                })
            }
        };
        if fx.sketch.is_construction(id) {
            p["isConstruction"] = json!(true);
        }
        prims.push(p);
    }
    for (id, c) in fx.sketch.constraints() {
        if skip.contains(&id) {
            continue;
        }
        let k = kid(id);
        let e = |x: EntityId| eid(x);
        let round = |r: Round| match r {
            Round::Circle(c) => eid(c.id()),
            Round::Arc(a) => eid(a.id()),
        };
        let p = match *c {
            L::Coincident { p, q } => {
                json!({"type": "coincident", "id": k, "a": e(p.id()), "b": e(q.id())})
            }
            // The whole line, as laman's; `on` alone is the segment.
            L::PointOnLine { p, line } => json!({
                "type": "on", "id": k, "point": e(p.id()), "curve": e(line.id()),
                "extension": true,
            }),
            L::PointOnCircle { p, on } => {
                json!({"type": "on", "id": k, "point": e(p.id()), "curve": round(on)})
            }
            L::Horizontal(Span::Line(l)) => {
                json!({"type": "horizontal", "id": k, "line": e(l.id())})
            }
            L::Horizontal(Span::Points(a, b)) => {
                json!({"type": "horizontal", "id": k, "a": e(a.id()), "b": e(b.id())})
            }
            L::Vertical(Span::Line(l)) => json!({"type": "vertical", "id": k, "line": e(l.id())}),
            L::Vertical(Span::Points(a, b)) => {
                json!({"type": "vertical", "id": k, "a": e(a.id()), "b": e(b.id())})
            }
            L::Parallel { a, b } => {
                json!({"type": "parallel", "id": k, "a": e(a.id()), "b": e(b.id())})
            }
            L::Perpendicular { a, b } => {
                json!({"type": "perpendicular", "id": k, "a": e(a.id()), "b": e(b.id())})
            }
            L::EqualLength { a, b } => {
                json!({"type": "equal", "id": k, "a": e(a.id()), "b": e(b.id())})
            }
            L::EqualRadius { a, b } => {
                json!({"type": "equal", "id": k, "a": round(a), "b": round(b)})
            }
            // The side comes from the drawing; laman's `sense` is not
            // passed. The residual proof checks they agree. The infinite
            // line for a circle, as laman's; an arc is tangent as acs
            // defines it.
            L::TangentLineCircle { line, round: r, .. } => {
                let mut v = json!({"type": "tangent", "id": k, "a": e(line.id()), "b": round(r)});
                if matches!(r, Round::Circle(_)) {
                    v["extension"] = json!(true);
                }
                v
            }
            L::TangentCircles { a, b, contact } => json!({
                "type": "tangent", "id": k, "a": round(a), "b": round(b),
                "internal": matches!(contact, Contact::Internal(_)),
            }),
            L::Midpoint { p, line } => json!({
                "type": "midpoint", "id": k, "entities": [e(p.id()), e(line.id())],
            }),
            L::Symmetric { p, q, axis } => json!({
                "type": "mirror", "id": k, "source": e(p.id()), "image": e(q.id()),
                "axis": e(axis.id()),
            }),
            L::Fix { p, x, y } => {
                // One laman constraint, two equations: two acs constraints
                // that map back to it.
                for (axis, v) in [("x", x), ("y", y)] {
                    let sub = format!("{k}{axis}");
                    prims.push(json!({"type": axis, "id": sub, "point": e(p.id()), "value": v}));
                    t.constraints.insert(sub, id);
                }
                continue;
            }
            L::Distance { p, q, value, .. } => json!({
                "type": "distance", "id": k, "a": e(p.id()), "b": e(q.id()), "value": value,
            }),
            // To the infinite line, on the side the drawing shows.
            L::PointLineDistance { p, line, value, .. } => json!({
                "type": "distance", "id": k, "a": e(p.id()), "b": e(line.id()),
                "value": value, "extension": true,
            }),
            L::Radius { round: r, value } => {
                json!({"type": "radius", "id": k, "curve": round(r), "value": value})
            }
            // Between the rays a -> b, a single root, as laman's directed angle.
            L::Angle { a, b, value } => json!({
                "type": "angle", "id": k, "a": e(a.id()), "b": e(b.id()), "value": value,
                "reverseA": false, "reverseB": false,
            }),
        };
        prims.push(p);
        t.constraints.insert(k, id);
    }
    t.request = json!({
        "version": 1,
        "primitives": prims,
        "maxIterations": max_iterations,
    });
    Ok(t)
}

/// `acs-solver`'s answer, parsed.
fn solve(t: &Translation) -> Result<Value, String> {
    let raw = acs::sketch_solve::solve_sketch_json(&t.request.to_string())
        .map_err(|e| format!("acs-solver refused the request: {e}"))?;
    let v: Value = serde_json::from_str(&raw).map_err(|e| format!("unreadable answer: {e}"))?;
    if v["status"] == "invalid" {
        return Err(format!("acs-solver refused the request: {}", v["error"]));
    }
    Ok(v)
}

/// The largest residual the solver reports for a solve, if it reports one.
fn residual(v: &Value) -> Option<f64> {
    let s = &v["stats"];
    let mut worst: Option<f64> = None;
    for key in ["initialError", "finalError"] {
        if let Some(x) = s[key].as_f64() {
            worst = Some(worst.map_or(x, |w| w.max(x)));
        }
    }
    worst
}

/// The largest distance between the solver's points and circle radii and the
/// expected ones.
fn moved_by(v: &Value, want: &Coordinates) -> Result<f64, String> {
    let mut worst = 0.0f64;
    let prims = v["primitives"]
        .as_array()
        .ok_or("no primitives in answer")?;
    for p in prims {
        let Some(id) = p["id"]
            .as_str()
            .and_then(|s| s.strip_prefix('e'))
            .and_then(|s| s.parse::<u32>().ok())
        else {
            continue;
        };
        let id = EntityId(id);
        for (key, c) in [
            ("x", Coordinate::X),
            ("y", Coordinate::Y),
            ("radius", Coordinate::R),
        ] {
            if p["type"] == "arc" {
                continue;
            }
            if let (Some(got), Some(w)) = (p[key].as_f64(), want.get(&(id, c))) {
                worst = worst.max((got - w).abs());
            }
        }
    }
    Ok(worst)
}

fn names(fx: &Fixture, t: &Translation, v: &Value) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for s in v.as_array().into_iter().flatten().filter_map(Value::as_str) {
        let n = match t.constraints.get(s) {
            Some(id) => fx.constraint_name(*id),
            None => format!("acs-solver constraint {s}"),
        };
        if !out.contains(&n) {
            out.push(n);
        }
    }
    out
}

/// Run `acs-solver` on one fixture.
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
        // The translation proof: at the expected coordinates, with no
        // iteration asked for (acs-solver still takes one step in a component
        // that is not already within 1e-10, which `moved` and the initial
        // error would show), the solver's own residuals are zero and nothing
        // moves, except exactly where the checker finds laman's constraints
        // violated (a conflicting twin's).
        let proof = translate(fx, &expected, &violated, 0)?;
        if let Some(u) = proof.unsupported.first() {
            let mut r = fail(
                Outcome::Unsupported,
                format!("{} has no translation", u.kind),
            );
            r.unsupported = proof.unsupported;
            return Ok(r);
        }
        let hold = solve(&proof)?;
        let held = residual(&hold).ok_or("acs-solver reports no residual")?;
        let moved = moved_by(&hold, &expected)?;
        if hold["status"] != "converged"
            || held > TRANSLATION_TOLERANCE
            || moved > TRANSLATION_TOLERANCE
        {
            return Ok(fail(
                Outcome::Error,
                format!(
                    "translation not proven: status {}, residual {held:e}, moved {moved:e} at the expected coordinates",
                    hold["status"]
                ),
            ));
        }
        // And each violated constraint, put back alone, fails: the solver
        // finds violated what the checker does.
        for v in &violated {
            let mut skip = violated.clone();
            skip.remove(v);
            let one = solve(&translate(fx, &expected, &skip, 0)?)?;
            if one["status"] != "failed" {
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
        let t = translate(fx, &guesses, &BTreeSet::new(), 100)?;
        let ans = solve(&t)?;
        let mut r = diagnose(fx, &t, &ans);
        r.translation_residual = Some(held);
        r.translation_unchecked = violated.iter().map(|id| fx.constraint_name(*id)).collect();
        Ok(r)
    };
    go().unwrap_or_else(|e| fail(Outcome::Error, e))
}

/// `acs-solver`'s answer in laman's words.
fn diagnose(fx: &Fixture, t: &Translation, v: &Value) -> Report {
    let mut r = Report::without_numbers(SOLVER, &fx.name, Outcome::Solved, "");
    r.detail = None;
    r.unsupported = t.unsupported.clone();
    r.solver_status = v["status"].as_str().map(str::to_owned);
    r.dof = v["dof"].as_u64().and_then(|d| u32::try_from(d).ok());
    let conflicting = names(fx, t, &v["conflicting"]);
    r.status = match (v["status"].as_str(), r.dof, conflicting.is_empty()) {
        (Some("converged"), Some(0), _) => Some(Status::FullyConstrained),
        (Some("converged"), Some(_), _) => Some(Status::UnderConstrained),
        (Some("failed"), _, false) => Some(Status::Unsatisfiable),
        (Some("failed"), _, true) => Some(Status::NotConverged),
        _ => None,
    };
    r.redundant = Some(names(fx, t, &v["redundant"]));
    // One flat list: one set, both sides of the conflict.
    r.conflicts = Some(if conflicting.is_empty() {
        Vec::new()
    } else {
        vec![conflicting]
    });
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
    /// and left in, fails at the expected coordinates (an angle, so
    /// the residual is 0.12 radians).
    #[test]
    fn the_residual_proof_sees_a_violated_constraint() {
        let fx = load("sss-triangle-conflicting");
        let expected = fx.expected_coordinates().expect("coordinates");
        let t = translate(&fx, &expected, &BTreeSet::new(), 0).expect("translates");
        let v = solve(&t).expect("solves");
        assert_eq!(v["status"], "failed");
        assert!(residual(&v).is_some_and(|x| x > 0.01), "{v}");
    }
}
