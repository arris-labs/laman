//! `Sketch::drag` and `DragSession`: refusals, frames, the non-finite
//! target, warm starts and `finish`. Every solution is checked
//! independently.
#![allow(clippy::unwrap_used, clippy::panic)]

use laman::model::{Coordinate, EntityId, ModelError, Sketch};
use laman::solution::{Algorithm, SolverOptions, Status};
use laman_check::check;

const ALGORITHMS: [Algorithm; 2] = [Algorithm::LevenbergMarquardt, Algorithm::DogLeg];

fn options(algorithm: Algorithm) -> SolverOptions {
    let mut o = SolverOptions::default();
    o.algorithm = algorithm;
    o
}

fn at(coordinates: &laman::model::Coordinates, id: EntityId) -> [f64; 2] {
    [
        coordinates[&(id, Coordinate::X)],
        coordinates[&(id, Coordinate::Y)],
    ]
}

/// A rod of length 5 from the origin to `p`, and a bystander `q` linked to
/// nothing.
fn rod() -> (Sketch, EntityId, EntityId) {
    let mut sk = Sketch::new();
    let o = sk.point_fixed(0.0, 0.0).unwrap();
    let p = sk.point(3.0, 4.0).unwrap();
    let q = sk.point(100.0, -7.0).unwrap();
    sk.distance(o, p, 5.0).unwrap();
    (sk, p.id(), q.id())
}

#[test]
fn the_refusals_name_the_id() {
    let mut sk = Sketch::new();
    let ground = sk.point_fixed(1.0, 2.0).unwrap();
    let a = sk.point(0.0, 0.0).unwrap();
    let l = sk.line(ground, a).unwrap();
    let o = SolverOptions::default();
    assert!(matches!(
        sk.drag(ground.id(), &o),
        Err(ModelError::FixedPoint { id }) if id == ground.id()
    ));
    assert!(matches!(
        sk.drag(l.id(), &o),
        Err(ModelError::WrongKind { id, .. }) if id == l.id()
    ));
    assert!(matches!(
        sk.drag(EntityId(99), &o),
        Err(ModelError::UnknownId { id: EntityId(99) })
    ));
}

#[test]
fn a_rod_follows_the_target_around_its_circle_under_both_algorithms() {
    for algorithm in ALGORITHMS {
        let (sk, p, q) = rod();
        let q_before = at(&sk.coordinates(), q);
        let mut drag = sk.drag(p, &options(algorithm)).unwrap();
        for target in [[0.0, 10.0], [-20.0, 0.0], [0.0, -5.0], [10.0, 0.0]] {
            let step = drag.to(target);
            let [x, y] = at(&step.coordinates, p);
            let norm = libm::hypot(target[0], target[1]);
            let want = [5.0 * target[0] / norm, 5.0 * target[1] / norm];
            assert!(
                (x - want[0]).abs() < 1e-9 && (y - want[1]).abs() < 1e-9,
                "{algorithm:?} {target:?}: {x} {y}"
            );
            assert!(check(&sk, &step.coordinates).is_clean(), "{algorithm:?}");
            assert_eq!(step.status, Status::UnderConstrained);
            assert_eq!(step.diagnostics.dof, 3); // the rod's one, the bystander's two
            assert!(step.diagnostics.flips.is_empty());
            // The bystander shares no equation with the dragged point.
            let now = at(&step.coordinates, q);
            assert_eq!(now.map(f64::to_bits), q_before.map(f64::to_bits));
        }
    }
}

#[test]
fn an_unconstrained_point_lands_on_every_target() {
    let mut sk = Sketch::new();
    let p = sk.point(1.0, 1.0).unwrap();
    let mut drag = sk.drag(p.id(), &SolverOptions::default()).unwrap();
    for target in [[4.0, 2.0], [-1e6, 3.5], [0.0, 0.0]] {
        let step = drag.to(target);
        assert_eq!(at(&step.coordinates, p.id()), target);
    }
}

#[test]
fn a_point_with_one_fixed_coordinate_drags_along_the_other() {
    let mut sk = Sketch::new();
    let p = sk.point(1.0, 2.0).unwrap();
    sk.set_fixed(p.id(), Coordinate::Y, true).unwrap();
    let mut drag = sk.drag(p.id(), &SolverOptions::default()).unwrap();
    let step = drag.to([9.0, 40.0]);
    assert_eq!(at(&step.coordinates, p.id()), [9.0, 2.0]);
    assert!(check(&sk, &step.coordinates).is_clean());
}

#[test]
fn a_target_that_is_not_finite_keeps_the_last_frame_and_the_session() {
    let (sk, p, _) = rod();
    let mut drag = sk.drag(p, &SolverOptions::default()).unwrap();
    let good = drag.to([0.0, 10.0]);
    for bad in [
        [f64::NAN, 0.0],
        [0.0, f64::INFINITY],
        [f64::NEG_INFINITY, 1.0],
    ] {
        let step = drag.to(bad);
        assert_eq!(step.status, Status::NotConverged);
        assert_eq!(step.coordinates, good.coordinates);
    }
    // The session is intact: the next finite target is one frame as usual.
    let next = drag.to([-10.0, 0.0]);
    assert!((at(&next.coordinates, p)[0] + 5.0).abs() < 1e-9);
}

#[test]
fn each_frame_starts_from_the_last_and_finish_applies_to_the_unedited_sketch() {
    let (mut sk, p, q) = rod();
    let before = sk.coordinates();
    let mut drag = sk.drag(p, &SolverOptions::default()).unwrap();
    let first = drag.to([0.0, 10.0]);
    let second = drag.to([-10.0, 0.0]);
    // The caller's sketch has not moved.
    assert_eq!(sk.coordinates(), before);
    let done = drag.finish();
    assert_eq!(done.coordinates, second.coordinates);
    // Any step of the session, the last included, applies to the sketch
    // as it was opened.
    let mut other = sk.clone();
    other.apply(&first).unwrap();
    assert!((at(&other.coordinates(), p)[1] - 5.0).abs() < 1e-9);
    sk.apply(&done).unwrap();
    assert!((at(&sk.coordinates(), p)[0] + 5.0).abs() < 1e-9);
    assert_eq!(at(&sk.coordinates(), q), [100.0, -7.0]);
    // An edited sketch refuses it.
    let mut edited = other.clone();
    edited.point(0.0, 0.0).unwrap();
    assert!(matches!(edited.apply(&done), Err(ModelError::Stale { .. })));
}

#[test]
fn finish_before_any_step_is_the_static_solution() {
    let (sk, p, _) = rod();
    let drag = sk.drag(p, &SolverOptions::default()).unwrap();
    assert_eq!(drag.finish(), sk.solve(&SolverOptions::default()));
}

/// A fixed horizontal line, and a line from the fixed (0, 1) to a free tip
/// at (3, 1) held parallel to it: the tip slides along y = 1 and crossing
/// x = 0 reverses the line (`tests/fixtures/drag/bisect-keeps`).
fn tip() -> (Sketch, EntityId) {
    let mut sk = Sketch::new();
    let o = sk.point_fixed(0.0, 0.0).unwrap();
    let e = sk.point_fixed(1.0, 0.0).unwrap();
    let a = sk.line(o, e).unwrap();
    let s = sk.point_fixed(0.0, 1.0).unwrap();
    let p = sk.point(3.0, 1.0).unwrap();
    let b = sk.line(s, p).unwrap();
    let c = sk.parallel(a, b).unwrap();
    sk.tag_constraint(c, "same-way").unwrap();
    (sk, p.id())
}

#[test]
fn without_bisections_a_flipping_frame_stays_put_and_the_session_recovers() {
    let (sk, p) = tip();
    let mut options = SolverOptions::default();
    options.max_bisections = 0;
    let mut drag = sk.drag(p, &options).unwrap();
    let stuck = drag.to([-1.0, 1.0]);
    assert_eq!(at(&stuck.coordinates, p), [3.0, 1.0]);
    assert_eq!(stuck.diagnostics.flips.len(), 1);
    // A refusal is marked, never read as a frame solved at its own target.
    let mark = stuck.diagnostics.bisection.unwrap();
    assert_eq!((mark.halvings, mark.reached), (0, [-1.0, 1.0]));
    let flip = &stuck.diagnostics.flips[0];
    assert!(flip.was > 0.0 && flip.now < 0.0, "{flip:?}");
    // The session is still on its branch: a target on the near side lands.
    let back = drag.to([2.0, 1.0]);
    assert!((at(&back.coordinates, p)[0] - 2.0).abs() < 1e-9);
    assert!(back.diagnostics.flips.is_empty());
}

#[test]
fn an_unbounded_number_of_bisections_still_terminates() {
    let (sk, p) = tip();
    let mut options = SolverOptions::default();
    options.max_bisections = usize::MAX;
    let mut drag = sk.drag(p, &options).unwrap();
    // The boundary is crossed in one frame; the nearest keeping halving is
    // taken, however many it needs.
    for _ in 0..3 {
        let step = drag.to([-1.0, 1.0]);
        assert!(at(&step.coordinates, p)[0] > 0.0);
        assert!(step.diagnostics.flips.is_empty());
    }
}
