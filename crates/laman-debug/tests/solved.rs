//! Solving, held to what the construction says. Each property builds a
//! sketch whose degrees of freedom, redundancy or contradiction are known
//! from how it was made (never from laman's output), solves it with both
//! algorithms from perturbed guesses, and checks the answer with the
//! independent checker. A case that fails here is shrunk and committed under
//! `tests/fixtures/regression/` (`.agents/skills/inspect`).
#![allow(clippy::unwrap_used, clippy::expect_used)] // tests: a failing unwrap is the test failing (clippy.toml)

#[path = "support/staged.rs"]
mod staged;

use laman::model::{Coordinate, Point, Sketch};
use laman::solution::{Algorithm, SolverOptions, Status};
use laman_check::{Source, check};
use laman_debug::strategy::sketch;
use proptest::prelude::*;

/// Magnitude up to which the checker's products stay finite (well under
/// sqrt(f64::MAX), about 1.3e154). Past it the checker itself overflows: that
/// is the gap of the regression fixture `extreme-point-on-line`, not a
/// property of the solver; its fix lifts this bound.
const CHECKABLE: f64 = 1e100;

const ALGORITHMS: [Algorithm; 2] = [Algorithm::LevenbergMarquardt, Algorithm::DogLeg];

fn options(algorithm: Algorithm) -> SolverOptions {
    let mut options = SolverOptions::default();
    options.algorithm = algorithm;
    options
}

/// One point of a Henneberg-style construction: where it truly is, how far
/// the guess is from there, and the earlier points it is tied to (0, 1 or 2
/// distances).
#[derive(Debug, Clone)]
struct Step {
    truth: (f64, f64),
    nudge: (f64, f64),
    ties: Vec<usize>,
}

fn step() -> impl Strategy<Value = Step> {
    (
        (-100.0..100.0_f64, -100.0..100.0_f64),
        (-0.1..0.1_f64, -0.1..0.1_f64),
        proptest::collection::vec(0..64_usize, 0..=2),
    )
        .prop_map(|(truth, nudge, ties)| Step { truth, nudge, ties })
}

/// What a construction made: the sketch, the true positions, and its
/// degrees of freedom by count (a point with `k` distances to distinct
/// earlier points, in general position, adds `2 - k`).
struct Built {
    sketch: Sketch,
    points: Vec<Point>,
    truth: Vec<(f64, f64)>,
    dof: usize,
}

/// Distance of the second fixed point from the origin.
const ANCHOR: f64 = 70.0;

/// Two fixed points, then each step's point tied by distance to distinct
/// earlier points at their true distance. `None` when the draw is not in
/// general position (coincident or nearly collinear anchors), which would
/// make the count above not hold.
fn build(steps: &[Step], force_two: bool) -> Option<Built> {
    let mut sk = Sketch::new();
    let origin = sk.point_fixed(0.0, 0.0).ok()?;
    let anchor = sk.point_fixed(ANCHOR, 0.0).ok()?;
    let mut points = vec![origin, anchor];
    let mut truth = vec![(0.0, 0.0), (ANCHOR, 0.0)];
    let mut dof = 0;
    for s in steps {
        let n = points.len();
        let mut ties: Vec<usize> = s.ties.iter().map(|t| t % n).collect();
        ties.sort_unstable();
        ties.dedup();
        if force_two {
            // Two distinct earlier points: the first draw, and the next
            // one around the ring.
            let first = s.ties.first().map_or(0, |t| t % n);
            ties = vec![
                first,
                (first + 1 + s.ties.get(1).map_or(0, |t| t % (n - 1).max(1))) % n,
            ];
            ties.sort_unstable();
            ties.dedup();
            if ties.len() < 2 {
                return None;
            }
        }
        for &t in &ties {
            let d = dist(truth[t], s.truth);
            if d < 5.0 {
                return None;
            }
        }
        if let [a, b] = ties[..] {
            let (u, v) = (sub(truth[a], s.truth), sub(truth[b], s.truth));
            let cross = u.0 * v.1 - u.1 * v.0;
            if cross.abs() < 0.3 * norm(u) * norm(v) {
                return None; // near-collinear anchors: a near-singular point
            }
        }
        let p = sk
            .point(s.truth.0 + s.nudge.0, s.truth.1 + s.nudge.1)
            .ok()?;
        for &t in &ties {
            sk.distance(points[t], p, dist(truth[t], s.truth)).ok()?;
        }
        dof += 2 - ties.len();
        points.push(p);
        truth.push(s.truth);
    }
    Some(Built {
        sketch: sk,
        points,
        truth,
        dof,
    })
}

fn sub(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    (a.0 - b.0, a.1 - b.1)
}

fn norm(v: (f64, f64)) -> f64 {
    libm::hypot(v.0, v.1)
}

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    norm(sub(a, b))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// The count of a construction is the solve's DoF; the answer is
    /// satisfied by the checker's measure; a count of 0 is fully constrained
    /// and nothing is redundant or in conflict. The staged freedom equals
    /// the whole-component reference's.
    #[test]
    fn a_construction_has_the_dof_it_was_built_with(steps in proptest::collection::vec(step(), 1..8)) {
        let Some(built) = build(&steps, false) else { return Ok(()); };
        for algorithm in ALGORITHMS {
            let solution =
                staged::assert_same_freedom("construction", &built.sketch, &options(algorithm));
            let report = check(&built.sketch, &solution.coordinates);
            prop_assert!(report.is_clean(), "{algorithm:?}: {report:?}");
            prop_assert_eq!(solution.diagnostics.dof, built.dof, "{:?}", algorithm);
            let want = if built.dof == 0 { Status::FullyConstrained } else { Status::UnderConstrained };
            prop_assert_eq!(solution.status, want, "{:?}", algorithm);
            prop_assert!(solution.diagnostics.conflicts.is_empty());
            prop_assert!(solution.diagnostics.redundant.is_empty());
            prop_assert_eq!(solution.diagnostics.free_directions.len(), built.dof);
        }
    }

    /// A fully constrained construction lands on the points it was built at.
    #[test]
    fn a_rigid_construction_lands_on_the_truth(steps in proptest::collection::vec(step(), 1..8)) {
        let Some(built) = build(&steps, true) else { return Ok(()); };
        prop_assert_eq!(built.dof, 0);
        for algorithm in ALGORITHMS {
            let solution = built.sketch.solve(&options(algorithm));
            prop_assert_eq!(solution.status, Status::FullyConstrained);
            for (p, want) in built.points.iter().zip(&built.truth) {
                let got = (
                    solution.coordinates[&(p.id(), laman::model::Coordinate::X)],
                    solution.coordinates[&(p.id(), laman::model::Coordinate::Y)],
                );
                prop_assert!(dist(got, *want) < 1e-6, "{algorithm:?}: {got:?} vs {want:?}");
            }
        }
    }

    /// An extra distance between two points of a rigid construction, true to
    /// the construction, is redundant and the status stays fully constrained;
    /// a false one is never satisfied: unsatisfiable with every conflict
    /// naming it, or an inconclusive analysis.
    #[test]
    fn an_extra_distance_is_redundant_when_true_and_a_conflict_when_false(
        steps in proptest::collection::vec(step(), 3..8),
        pick in (0..64_usize, 0..64_usize),
        false_by in 1.2..3.0_f64,
    ) {
        let Some(built) = build(&steps, true) else { return Ok(()); };
        prop_assert_eq!(built.dof, 0);
        let (i, j) = (pick.0 % built.points.len(), pick.1 % built.points.len());
        prop_assume!(i != j);
        let d = dist(built.truth[i], built.truth[j]);
        prop_assume!(d > 5.0);
        for algorithm in ALGORITHMS {
            let mut sk = built.sketch.clone();
            sk.distance(built.points[i], built.points[j], d).unwrap();
            let solution = staged::assert_same_freedom("extra distance", &sk, &options(algorithm));
            prop_assert!(check(&sk, &solution.coordinates).is_clean());
            prop_assert_eq!(solution.status, Status::FullyConstrained);
            prop_assert!(
                !solution.diagnostics.redundant.is_empty(),
                "{algorithm:?}: the true extra distance left nothing redundant"
            );

            let mut sk = built.sketch.clone();
            let stretched = sk
                .distance(built.points[i], built.points[j], d * false_by)
                .unwrap();
            let solution = staged::assert_same_freedom("false distance", &sk, &options(algorithm));
            // The conflict analysis may be inconclusive (regression
            // `framework-conflict-inconclusive`):
            // NotConverged, no conflict named. It must never call the
            // contradictory sketch satisfied.
            let proven = solution.status == Status::Unsatisfiable;
            let inconclusive = solution.status == Status::NotConverged;
            prop_assert!(proven || inconclusive, "{:?}: {:?}", algorithm, solution.status);
            // Every genuine conflict contains the stretched distance: the
            // rest hold together at the construction.
            prop_assert_eq!(proven, !solution.diagnostics.conflicts.is_empty());
            // The checker, independent of the solve, must see the
            // contradiction: the other constraints pin every point, so no
            // coordinates satisfy the stretched distance as well.
            let report = check(&sk, &solution.coordinates);
            prop_assert!(
                !report.violations.is_empty(),
                "{:?}: {:?} but the checker finds nothing violated",
                algorithm,
                solution.status
            );
            // A proven conflict is unsatisfiable, so the best effort
            // violates at least one of its members.
            for set in &solution.diagnostics.conflicts {
                prop_assert!(
                    report.violations.iter().any(|v| match v.id {
                        Source::Constraint(id) => set.iter().any(|c| c.id == id),
                        Source::Arc(_) => false,
                    }),
                    "{:?}: no member of {:?} violated: {:?}",
                    algorithm,
                    set,
                    report
                );
            }
            for set in &solution.diagnostics.conflicts {
                prop_assert!(
                    set.iter().any(|c| c.id == stretched),
                    "{:?}: a conflict without the stretched distance: {:?}",
                    algorithm,
                    set
                );
            }
            prop_assert!(solution.coordinates.values().all(|v| v.is_finite()));
        }
    }

    /// Any well-formed sketch, near the extremes of f64 included, solves to
    /// a complete answer without a panic under both algorithms, and the
    /// checker judges every solve: a satisfied status has no violation, a
    /// proven conflict has a violated member. Above `CHECKABLE` the checker
    /// overflows (regression `extreme-point-on-line`) and is skipped. The
    /// staged freedom equals the whole-component reference's, at any
    /// magnitude.
    #[test]
    fn any_well_formed_sketch_solves_to_a_complete_answer(sk in sketch(20)) {
        for algorithm in ALGORITHMS {
            let solution = staged::assert_same_freedom("well-formed", &sk, &options(algorithm));
            prop_assert_eq!(solution.coordinates.len(), sk.coordinates().len());
            for (key, value) in &solution.coordinates {
                prop_assert!(key.1 != Coordinate::R || *value >= 0.0, "{algorithm:?}: radius {:?} = {value}", key.0);
            }
            let mut applied = sk.clone();
            prop_assert!(applied.apply(&solution).is_ok(), "{algorithm:?}: apply refused");
            let checkable = sk.coordinates().values().chain(solution.coordinates.values()).all(|v| v.abs() <= CHECKABLE);
            if !checkable {
                continue;
            }
            let report = check(&sk, &solution.coordinates);
            match solution.status {
                Status::FullyConstrained | Status::UnderConstrained => {
                    prop_assert!(report.violations.is_empty(), "{algorithm:?}: {report:?}");
                }
                Status::Unsatisfiable => {
                    for set in &solution.diagnostics.conflicts {
                        prop_assert!(
                            report.violations.iter().any(|v| match v.id {
                                Source::Constraint(id) => set.iter().any(|c| c.id == id),
                                Source::Arc(_) => false,
                            }),
                            "{algorithm:?}: no member of {set:?} violated: {report:?}"
                        );
                    }
                }
                // Nothing is proven: the checker's report is taken, no claim made.
                Status::NotConverged => {}
            }
        }
    }
}
