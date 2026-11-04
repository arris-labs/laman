//! Dragging, held to what the construction says. A rod (a point tied by a
//! distance to a fixed one) has one known free direction, the tangent of
//! its circle; a target on that circle is reachable, so a drag along it must
//! land on every target, keep the checker green and every branch, and leave
//! the geometry no equation links to the rod with its bits. The same holds
//! at sketch scales; at the extremes of f64 (1e+-300) the checker's products
//! overflow (regression `extreme-point-on-line`), so there the guarantee is
//! totality: a complete, finite answer, and untouched bits elsewhere. The
//! last property drags any well-formed sketch to any targets, extremes
//! included, as the fuzz target does.
#![allow(clippy::unwrap_used, clippy::expect_used)] // tests: a failing unwrap is the test failing (clippy.toml)

use laman::model::{Coordinate, EntityId, Sketch};
use laman::solution::{Algorithm, SolverOptions, Status};
use laman_check::check;
use laman_debug::strategy::sketch;
use proptest::prelude::*;

const ALGORITHMS: [Algorithm; 2] = [Algorithm::LevenbergMarquardt, Algorithm::DogLeg];

fn options(algorithm: Algorithm) -> SolverOptions {
    let mut options = SolverOptions::default();
    options.algorithm = algorithm;
    options
}

/// A rod from the origin, a chain of other rods that never meets it, and
/// the rod's circle.
struct Built {
    sketch: Sketch,
    rod: EntityId,
    /// The rod's radius and its angle at the start.
    radius: f64,
    angle: f64,
    /// Degrees of freedom by count: the rod's one, and one per chain point
    /// (two coordinates, one distance).
    dof: usize,
}

/// A fixed origin; the rod's point at `radius` from it; `chain` more points,
/// each tied by distance to the origin (the first) or to the one before,
/// their guesses nudged off the truth. `scale` multiplies every length.
///
/// With `arc` set, the rod's distance is replaced by an arc about the origin
/// from a fixed start, `arc` radians behind the rod, to the rod itself: the
/// arc's own equation holds the rod on its circle (the same one degree of
/// freedom) and its tracked sweep is an observed subject the drag moves.
fn build(scale: f64, rod_angle: f64, arc: Option<f64>, chain: &[(f64, f64)]) -> Option<Built> {
    let mut sk = Sketch::new();
    let origin = sk.point_fixed(0.0, 0.0).ok()?;
    let radius = 40.0 * scale;
    let rod = sk
        .point(radius * libm::cos(rod_angle), radius * libm::sin(rod_angle))
        .ok()?;
    if let Some(behind) = arc {
        let back = rod_angle - behind;
        let start = sk
            .point_fixed(radius * libm::cos(back), radius * libm::sin(back))
            .ok()?;
        sk.arc(origin, start, rod).ok()?;
    } else {
        sk.distance(origin, rod, radius).ok()?;
    }
    let mut previous = origin;
    let mut truth = (0.0, 0.0);
    for (k, &(angle, nudge)) in chain.iter().enumerate() {
        let r = scale * (20.0 + 10.0 * k as f64);
        let at = (r * libm::cos(angle), r * libm::sin(angle));
        let p = sk.point(at.0 + nudge * scale, at.1 - nudge * scale).ok()?;
        let d = libm::hypot(at.0 - truth.0, at.1 - truth.1);
        if d < 1e-3 * scale {
            return None;
        }
        sk.distance(previous, p, d).ok()?;
        previous = p;
        truth = at;
    }
    Some(Built {
        sketch: sk,
        rod: rod.id(),
        radius,
        angle: rod_angle,
        dof: 1 + chain.len(),
    })
}

fn chain() -> impl Strategy<Value = Vec<(f64, f64)>> {
    proptest::collection::vec((0.0..std::f64::consts::TAU, -0.05..0.05_f64), 0..4)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    /// At sketch scales: every target on the rod's circle is reached, every
    /// step checks clean with no flip, and the chain, which no equation ties
    /// to the rod, keeps the bits the session's first solve gave it.
    #[test]
    fn dragging_a_rod_along_its_circle_reaches_every_target(
        exponent in -2.0..3.0_f64,
        rod_angle in 0.0..std::f64::consts::TAU,
        chain in chain(),
        turns in proptest::collection::vec(-0.2..0.2_f64, 1..8),
    ) {
        let scale = libm::pow(10.0, exponent);
        let Some(built) = build(scale, rod_angle, None, &chain) else { return Ok(()); };
        for algorithm in ALGORITHMS {
            let mut session = built.sketch.drag(built.rod, &options(algorithm)).unwrap();
            let rest = built.sketch.solve(&options(algorithm));
            let mut angle = built.angle;
            for turn in &turns {
                angle += turn;
                let target = [built.radius * libm::cos(angle), built.radius * libm::sin(angle)];
                let step = session.to(target);
                let report = check(&built.sketch, &step.coordinates);
                prop_assert!(report.is_clean(), "{algorithm:?}: {report:?}");
                prop_assert!(step.diagnostics.flips.is_empty());
                prop_assert_eq!(step.status, Status::UnderConstrained);
                prop_assert_eq!(step.diagnostics.dof, built.dof);
                let at = |c| step.coordinates[&(built.rod, c)];
                let miss = libm::hypot(at(Coordinate::X) - target[0], at(Coordinate::Y) - target[1]);
                prop_assert!(miss <= 1e-9 * scale.max(1.0), "{algorithm:?}: missed by {miss}");
                for (key, value) in &step.coordinates {
                    if key.0 != built.rod {
                        prop_assert_eq!(value.to_bits(), rest.coordinates[key].to_bits(), "{:?} {:?}", algorithm, key);
                    }
                }
            }
        }
    }

    /// With an arc on the rod (its tracked sweep observed), every frame is
    /// either at its own target with no flip and no mark, or bisected and
    /// marked (`bisection` set, `halvings >= 1`); a bisected frame names no
    /// flip, or stayed where it was because every try flipped. Never a
    /// flip on a frame that claims its target, and always clean.
    #[test]
    fn dragging_a_rod_that_carries_an_arc_is_unflipped_or_marked_bisected(
        exponent in -2.0..3.0_f64,
        rod_angle in 0.0..std::f64::consts::TAU,
        behind in 0.4..5.8_f64,
        chain in chain(),
        turns in proptest::collection::vec(-0.2..0.2_f64, 1..8),
    ) {
        let scale = libm::pow(10.0, exponent);
        let Some(built) = build(scale, rod_angle, Some(behind), &chain) else { return Ok(()); };
        for algorithm in ALGORITHMS {
            let mut session = built.sketch.drag(built.rod, &options(algorithm)).unwrap();
            let mut previous = built.sketch.solve(&options(algorithm)).coordinates;
            let mut angle = built.angle;
            for turn in &turns {
                angle += turn;
                let target = [built.radius * libm::cos(angle), built.radius * libm::sin(angle)];
                let step = session.to(target);
                let report = check(&built.sketch, &step.coordinates);
                prop_assert!(report.is_clean(), "{algorithm:?}: {report:?}");
                prop_assert_eq!(step.diagnostics.dof, built.dof);
                match step.diagnostics.bisection {
                    None => {
                        prop_assert!(step.diagnostics.flips.is_empty(), "{algorithm:?}: {:?}", step.diagnostics.flips);
                        let at = |c| step.coordinates[&(built.rod, c)];
                        let miss = libm::hypot(at(Coordinate::X) - target[0], at(Coordinate::Y) - target[1]);
                        prop_assert!(miss <= 1e-9 * scale.max(1.0), "{algorithm:?}: missed by {miss}");
                    }
                    Some(bisection) => {
                        prop_assert!(bisection.halvings >= 1);
                        prop_assert!(
                            step.diagnostics.flips.is_empty() || step.coordinates == previous,
                            "{algorithm:?}: a moved frame names {:?}", step.diagnostics.flips
                        );
                    }
                }
                previous = step.coordinates;
                // A session that stayed put or stopped short follows the
                // rod from where it is: aim the next turn from the frame.
                angle = libm::atan2(
                    previous[&(built.rod, Coordinate::Y)],
                    previous[&(built.rod, Coordinate::X)],
                );
            }
        }
    }

    /// At the extremes of f64 the same drag returns a complete, finite
    /// answer at every step, applies back to the sketch, and leaves the
    /// chain's bits alone.
    #[test]
    fn dragging_a_rod_at_the_extremes_stays_complete_and_finite(
        exponent in prop_oneof![-300.0..-250.0_f64, 250.0..300.0_f64],
        rod_angle in 0.0..std::f64::consts::TAU,
        chain in chain(),
        turns in proptest::collection::vec(-0.2..0.2_f64, 1..8),
    ) {
        let scale = libm::pow(10.0, exponent);
        let Some(built) = build(scale, rod_angle, None, &chain) else { return Ok(()); };
        for algorithm in ALGORITHMS {
            let mut session = built.sketch.drag(built.rod, &options(algorithm)).unwrap();
            let rest = built.sketch.solve(&options(algorithm));
            let mut angle = built.angle;
            for turn in &turns {
                angle += turn;
                let step = session.to([built.radius * libm::cos(angle), built.radius * libm::sin(angle)]);
                prop_assert_eq!(step.coordinates.len(), built.sketch.coordinates().len());
                prop_assert!(step.coordinates.values().all(|v| v.is_finite()), "{:?}: {:?}", algorithm, step.coordinates);
                for (key, value) in &step.coordinates {
                    if key.0 != built.rod {
                        prop_assert_eq!(value.to_bits(), rest.coordinates[key].to_bits(), "{:?} {:?}", algorithm, key);
                    }
                }
            }
            let mut applied = built.sketch.clone();
            prop_assert!(applied.apply(&session.finish()).is_ok());
        }
    }

    /// Any well-formed sketch, any point of it, any targets (finite ones
    /// anywhere in f64's range, and not finite ones): a complete answer at
    /// every step, finite whenever the target is, and `finish` applies back.
    #[test]
    fn any_point_of_any_sketch_drags_to_any_targets(
        sk in sketch(20),
        pick in 0..64_usize,
        targets in proptest::collection::vec(
            (prop_oneof![-1e3..1e3_f64, -1e300..1e300_f64, Just(f64::NAN), Just(f64::INFINITY)],
             prop_oneof![-1e3..1e3_f64, -1e300..1e300_f64]),
            1..6,
        ),
    ) {
        let ids: Vec<EntityId> = sk.entities().map(|(id, _)| id).collect();
        let Some(&id) = (!ids.is_empty()).then(|| &ids[pick % ids.len()]) else { return Ok(()); };
        for algorithm in ALGORITHMS {
            let Ok(mut session) = sk.drag(id, &options(algorithm)) else { continue; };
            for (x, y) in &targets {
                let step = session.to([*x, *y]);
                prop_assert_eq!(step.coordinates.len(), sk.coordinates().len());
                if x.is_finite() && y.is_finite() {
                    prop_assert!(step.coordinates.values().all(|v| v.is_finite()), "{:?}: {:?}", algorithm, step.coordinates);
                    for (key, value) in &step.coordinates {
                        prop_assert!(key.1 != Coordinate::R || *value >= 0.0, "{:?}: radius {:?} = {}", algorithm, key.0, value);
                    }
                } else {
                    prop_assert_eq!(step.status, Status::NotConverged);
                }
            }
            let mut applied = sk.clone();
            let finished = session.finish();
            for (key, value) in &finished.coordinates {
                prop_assert!(key.1 != Coordinate::R || *value >= 0.0, "{:?}: radius {:?} = {}", algorithm, key.0, value);
            }
            prop_assert!(applied.apply(&finished).is_ok(), "{:?}", algorithm);
        }
    }
}
