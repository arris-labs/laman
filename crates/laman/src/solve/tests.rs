#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use super::*;
use crate::model::Coordinate;

use laman_debug::{dump, fixture::Fixture};
use std::path::PathBuf;

const COORDINATE_ERROR: f64 = 1e-9; // Closed-form fixture acceptance, independent of solver thresholds.

/// Every analytic case runs under both engines (plans/m2 step 4).
const ALGORITHMS: [Algorithm; 2] = [Algorithm::LevenbergMarquardt, Algorithm::DogLeg];

fn with(algorithm: Algorithm) -> Options {
    Options {
        algorithm,
        ..Options::default()
    }
}

// Unit tests exercise the private engine. The checker links the normal library,
// so cross the test/library type boundary through the public serde form.
fn fixture_at(sketch: &Sketch, values: &Coordinates) -> Fixture {
    let mut fixture = Fixture::parse(
        &serde_json::to_string(sketch).unwrap(),
        r#"{"status":"UnderConstrained","dof":0,"coordinates":[]}"#,
        None,
    )
    .unwrap();
    fixture.expected.coordinates = fixture.sketch.coordinates();
    assert_eq!(fixture.expected.coordinates.len(), values.len());
    for (to, from) in fixture
        .expected
        .coordinates
        .values_mut()
        .zip(values.values())
    {
        *to = *from;
    }
    fixture
}
fn check(sketch: &Sketch, values: &Coordinates) -> laman_check::Report {
    let f = fixture_at(sketch, values);
    laman_check::check(&f.sketch, &f.expected.coordinates)
}
fn inspect(sketch: &Sketch, out: &Outcome, slug: &str) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/inspect")
        .join(slug);
    std::fs::create_dir_all(&path).unwrap();
    let f = fixture_at(sketch, &out.coordinates);
    let report = laman_check::check(&f.sketch, &f.expected.coordinates);
    std::fs::write(
        path.join("solved.txt"),
        dump::text(&f.sketch, &f.expected.coordinates, &report),
    )
    .unwrap();
    std::fs::write(
        path.join("solved.svg"),
        dump::svg(&f.sketch, &f.expected.coordinates, &report),
    )
    .unwrap();
}
fn constraint_ids(report: laman_check::Report) -> Vec<u32> {
    report
        .violations
        .iter()
        .map(|v| match v.id {
            laman_check::Source::Constraint(id) => id.0,
            laman_check::Source::Arc(_) => panic!("unexpected intrinsic violation"),
        })
        .collect()
}

#[test]
fn analytic_distance_minimum_motion_and_zero_step() {
    for algorithm in ALGORITHMS {
        let mut sk = Sketch::new();
        let a = sk.point(0.0, 0.0).unwrap();
        let b = sk.point(3.0, 4.0).unwrap();
        sk.distance(a, b, 10.0).unwrap();
        let stray = sk.point(-0.0, 200.0).unwrap();
        let out = solve(&sk, with(algorithm));
        assert!(out.converged, "{out:?}");
        assert!(check(&sk, &out.coordinates).is_clean());
        for (id, xy) in [(a.id(), [-1.5, -2.0]), (b.id(), [4.5, 6.0])] {
            for (c, v) in [Coordinate::X, Coordinate::Y].into_iter().zip(xy) {
                assert!(
                    (out.coordinates[&(id, c)] - v).abs() < COORDINATE_ERROR,
                    "{algorithm:?} {out:?}"
                );
            }
        }
        assert_eq!(
            out.coordinates[&(stray.id(), Coordinate::X)].to_bits(),
            (-0.0f64).to_bits()
        );
        inspect(&sk, &out, &format!("{algorithm:?}-distance"));
        let mut solved = Sketch::new();
        let a = solved.point(-1.5, -2.0).unwrap();
        let b = solved.point(4.5, 6.0).unwrap();
        solved.distance(a, b, 10.0).unwrap();
        let out = solve(&solved, with(algorithm));
        assert!(out.converged);
        assert!(check(&solved, &out.coordinates).is_clean());
        assert_eq!(out.iterations, 0);
        assert_eq!(out.coordinates, solved.coordinates());
    }
}

#[test]
fn c1_contacts_from_guesses() {
    for algorithm in ALGORITHMS {
        for slug in ["sss-triangle", "belt", "slotted-plate"] {
            let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/c1")
                .join(slug);
            let fixture = Fixture::load(&path).unwrap();
            let sk: Sketch =
                serde_json::from_str(&serde_json::to_string(&fixture.sketch).unwrap()).unwrap();
            let out = solve(&sk, with(algorithm));
            inspect(&sk, &out, &format!("{algorithm:?}-{slug}"));
            eprintln!(
                "{algorithm:?} {slug}: iterations={} evaluations={} rejected={}",
                out.iterations, out.evaluations, out.rejected
            );
            assert!(out.converged, "{slug}: {out:?}");
            assert!(
                check(&sk, &out.coordinates).is_clean(),
                "{slug}: {:?}",
                check(&sk, &out.coordinates)
            );
            let solved = reseed(&sk, &out.coordinates);
            let again = solve(&solved, with(algorithm));
            assert!(again.converged);
            assert!(check(&solved, &again.coordinates).is_clean());
            assert_eq!(again.iterations, 0);
            assert_eq!(
                out.coordinates
                    .values()
                    .map(|x| x.to_bits())
                    .collect::<Vec<_>>(),
                again
                    .coordinates
                    .values()
                    .map(|x| x.to_bits())
                    .collect::<Vec<_>>()
            );
            let public = fixture_at(&sk, &out.coordinates);
            for (key, want) in &fixture.expected.coordinates {
                assert!(
                    (public.expected.coordinates[key] - want).abs() <= fixture.expected.tolerance,
                    "{slug}: {key:?}: {} != {want}",
                    public.expected.coordinates[key]
                );
            }
        }
    }
}

/// Fixed o = (0, 0) and q = (10, 0); p at distance 6 from o and 8 from q,
/// so p = (3.6, ±4.8) (a 6-8-10 right triangle), guessed at (x, y).
fn trilateration(x: f64, y: f64) -> (Sketch, crate::model::Point, [u32; 2]) {
    let mut sk = Sketch::new();
    let o = sk.point_fixed(0.0, 0.0).unwrap();
    let q = sk.point_fixed(10.0, 0.0).unwrap();
    let p = sk.point(x, y).unwrap();
    let a = sk.distance(o, p, 6.0).unwrap();
    let b = sk.distance(q, p, 8.0).unwrap();
    (sk, p, [a.0, b.0])
}

fn residual_norm(sk: &Sketch, values: &Coordinates, p: crate::model::Point) -> f64 {
    let (x, y) = (
        values[&(p.id(), Coordinate::X)],
        values[&(p.id(), Coordinate::Y)],
    );
    let _ = sk;
    libm::hypot(libm::hypot(x, y) - 6.0, libm::hypot(x - 10.0, y) - 8.0)
}

#[test]
fn budgets_fixed_only_and_finite_extremes() {
    for algorithm in ALGORITHMS {
        // Trilateration, nonlinear for both engines: from (3, 4) no single
        // step reaches the construction's (3.6, 4.8), so a two-evaluation
        // budget (the initial one and one trial) is exhausted, not met.
        let (sk, p, ids) = trilateration(3.0, 4.0);
        let start = residual_norm(&sk, &sk.coordinates(), p);
        for options in [
            Options {
                max_iterations: 0,
                ..with(algorithm)
            },
            Options {
                max_evaluations: 0,
                ..with(algorithm)
            },
            Options {
                max_evaluations: 2,
                ..with(algorithm)
            },
        ] {
            let out = solve(&sk, options);
            assert!(!out.converged, "{algorithm:?} {options:?}");
            assert!(out.evaluations <= options.max_evaluations);
            assert!(out.iterations <= options.max_iterations);
            let flagged = constraint_ids(check(&sk, &out.coordinates));
            assert!(!flagged.is_empty() && flagged.iter().all(|id| ids.contains(id)));
            assert!(out.coordinates.values().all(|x| x.is_finite()));
            // Best effort is never worse than the start.
            assert!(residual_norm(&sk, &out.coordinates, p) <= start);
        }
        for scale in [1e-300, 1e300] {
            let mut sk = Sketch::new();
            let a = sk.point_fixed(0.0, 0.0).unwrap();
            let b = sk.point(3.0 * scale, 4.0 * scale).unwrap();
            sk.distance(a, b, 5.0 * scale).unwrap();
            let out = solve(&sk, with(algorithm));
            assert!(out.converged);
            assert!(check(&sk, &out.coordinates).is_clean());
            assert_eq!(out.iterations, 0);
        }
        let mut sk = Sketch::new();
        let a = sk.point_fixed(-f64::MAX, 0.0).unwrap();
        let b = sk.point_fixed(f64::MAX, 0.0).unwrap();
        let id = sk.distance(a, b, 1.0).unwrap();
        let out = solve(&sk, with(algorithm));
        assert!(!out.converged);
        assert_eq!(out.coordinates, sk.coordinates());
        assert_eq!(constraint_ids(check(&sk, &out.coordinates)), [id.0]);
        let out = solve(&Sketch::new(), with(algorithm));
        assert!(out.converged);
        assert!(check(&Sketch::new(), &out.coordinates).is_clean());
    }
}

#[test]
fn rejects_steps_and_retains_finite_best_effort() {
    for algorithm in ALGORITHMS {
        let mut sk = Sketch::new();
        let p = sk.point(0.0, 0.0).unwrap();
        let a = sk.fix(p, 1.0, 0.0).unwrap();
        let b = sk.fix(p, 2.0, 0.0).unwrap();
        let out = solve(&sk, with(algorithm));
        eprintln!(
            "{algorithm:?} conflicting fixes: iterations={} evaluations={} rejected={}",
            out.iterations, out.evaluations, out.rejected
        );
        assert!(!out.converged);
        assert!(out.rejected > 0);
        assert!(out.iterations <= MAX_ITERATIONS);
        assert!(
            (out.coordinates[&(p.id(), Coordinate::X)] - 1.5).abs() < COORDINATE_ERROR,
            "{:?}",
            out.coordinates
        );
        assert_eq!(constraint_ids(check(&sk, &out.coordinates)), [a.0, b.0]);
        inspect(&sk, &out, &format!("{algorithm:?}-conflicting-fixes"));
    }
}

#[test]
fn damped_step_has_no_null_motion() {
    let e = Evaluation {
        values: vec![-2.0],
        jacobian: vec![vec![1.0, 1.0]],
        valid: true,
        satisfied: false,
    };
    let step = damped_step(&e, INITIAL_DAMPING).unwrap();
    assert!((step[0] - step[1]).abs() < COORDINATE_ERROR);
    assert!((step[0] - 2.0 / (2.0 + INITIAL_DAMPING)).abs() < COORDINATE_ERROR);
    // Matrix-level test does not solve a sketch; geometry checks are above.
}

#[test]
fn stopping_regression() {
    for algorithm in ALGORITHMS {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/regression/lm-coupled-stopping");
        let f = Fixture::load(&path).unwrap();
        let sk: Sketch = serde_json::from_str(&serde_json::to_string(&f.sketch).unwrap()).unwrap();
        let out = solve(&sk, with(algorithm));
        assert!(out.converged);
        assert!(check(&sk, &out.coordinates).is_clean());
        let actual = fixture_at(&sk, &out.coordinates);
        for (key, want) in &f.expected.coordinates {
            assert!((actual.expected.coordinates[key] - want).abs() <= f.expected.tolerance);
        }
    }
}

fn reseed(sk: &Sketch, values: &Coordinates) -> Sketch {
    let mut form = serde_json::to_value(sk).unwrap();
    for entity in form["entities"].as_array_mut().unwrap() {
        let id = crate::model::EntityId(entity["id"].as_u64().unwrap() as u32);
        for (coordinate, key) in [
            (Coordinate::X, "x"),
            (Coordinate::Y, "y"),
            (Coordinate::R, "r"),
        ] {
            if let Some(value) = values.get(&(id, coordinate)) {
                entity[key] = serde_json::json!(value);
            }
        }
    }
    serde_json::from_value(form).unwrap()
}

#[test]
fn original_tangent_sense_is_required() {
    for algorithm in ALGORITHMS {
        use crate::model::{Constraint, Sense};
        let mut sk = Sketch::new();
        let c = sk.point_fixed(0.0, 1.0).unwrap();
        let p = sk.point(0.0, 0.0).unwrap();
        let q = sk.point(2.0, 0.0).unwrap();
        let line = sk.line(p, q).unwrap();
        let circle = sk.circle(c, 1.0).unwrap();
        sk.point_on_circle(p, circle).unwrap();
        let id = sk
            .add_constraint(Constraint::TangentLineCircle {
                line,
                round: circle.into(),
                sense: Sense::Negative,
            })
            .unwrap();
        let out = solve(&sk, with(algorithm));
        assert!(!out.converged);
        assert_eq!(out.coordinates, sk.coordinates());
        assert_eq!(constraint_ids(check(&sk, &out.coordinates)), [id.0]);
        inspect(&sk, &out, &format!("{algorithm:?}-wrong-sense"));
    }
}

#[test]
fn extreme_nonzero_steps_and_nonfinite_transform() {
    for algorithm in ALGORITHMS {
        let mut sk = Sketch::new();
        let a = sk.point_fixed(0.0, 0.0).unwrap();
        let b = sk.point(3e300, 4e300).unwrap();
        sk.distance(a, b, 1e301).unwrap();
        let out = solve(&sk, with(algorithm));
        assert!(out.converged);
        assert!(out.iterations > 0);
        assert!(check(&sk, &out.coordinates).is_clean());
        assert!(out.coordinates.values().all(|x| x.is_finite()));
        let mut sk = Sketch::new();
        let a = sk.point_fixed(-f64::MAX, 0.0).unwrap();
        let b = sk.point(f64::MAX, 0.0).unwrap();
        let id = sk.distance(a, b, 1.0).unwrap();
        let out = solve(&sk, with(algorithm));
        assert!(!out.converged);
        assert_eq!(out.coordinates, sk.coordinates());
        assert_eq!(constraint_ids(check(&sk, &out.coordinates)), [id.0]);
    }
}

#[test]
fn vector_acceptance_is_not_two_scalar_acceptances() {
    for algorithm in ALGORITHMS {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/regression/lm-vector-stop");
        let f = Fixture::load(&path).unwrap();
        let sk: Sketch = serde_json::from_str(&serde_json::to_string(&f.sketch).unwrap()).unwrap();
        let out = solve(&sk, with(algorithm));
        assert!(out.converged);
        assert!(check(&sk, &out.coordinates).is_clean());
        assert!(out.iterations > 0);
    }
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config::with_cases(128))]
    #[test]
    fn radial_construction_property(x in 1.0f64..10.0, y in 1.0f64..10.0,
                                    multiplier in 0.5f64..2.0, exponent in -6i32..7) {
        let scale=libm::pow(10.0,exponent as f64);
        let mut sk=Sketch::new();
        let a=sk.point_fixed(0.0,0.0).unwrap();
        let b=sk.point(x*scale,y*scale).unwrap();
        sk.distance(a,b,multiplier*libm::hypot(x*scale,y*scale)).unwrap();
        for algorithm in ALGORITHMS {
        let out=solve(&sk,with(algorithm));
        proptest::prop_assert!(out.converged);
        proptest::prop_assert!(check(&sk,&out.coordinates).is_clean());
        for (coordinate,want) in [(Coordinate::X,x*scale*multiplier),(Coordinate::Y,y*scale*multiplier)] {
            proptest::prop_assert!((out.coordinates[&(b.id(),coordinate)]-want).abs()<COORDINATE_ERROR*scale.max(1.0));
        }
        }
    }
}

#[test]
fn far_guess_does_not_loosen_acceptance() {
    for algorithm in ALGORITHMS {
        // check-guess-scale: a length-1 distance guessed 1e6 away. The frozen
        // input scale would accept a 1e-6 error; acceptance at the iterate's own
        // scale must reach the construction's (1, 0) as the checker measures it.
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/regression/check-guess-scale");
        let f = Fixture::load(&path).unwrap();
        let sk: Sketch = serde_json::from_str(&serde_json::to_string(&f.sketch).unwrap()).unwrap();
        let out = solve(&sk, with(algorithm));
        assert!(out.converged, "{out:?}");
        assert!(check(&sk, &out.coordinates).is_clean());
        let actual = fixture_at(&sk, &out.coordinates);
        for (key, want) in &f.expected.coordinates {
            let got = actual.expected.coordinates[key];
            assert!(
                (got - want).abs() <= f.expected.tolerance,
                "{key:?}: {got} != {want}"
            );
        }
        inspect(&sk, &out, &format!("{algorithm:?}-check-guess-scale"));
    }
}

#[test]
fn trust_region_rejects_overshoot() {
    // From (3.6, 1e-3), p is nearly on the line through o and q, where both
    // distance gradients are almost horizontal: the Gauss–Newton step is
    // about 1e3 long in y. A trust region of 1e3 component scales admits it,
    // the objective grows, and the step is rejected until the region fits.
    let (sk, p, _) = trilateration(3.6, 1e-3);
    let options = Options {
        algorithm: Algorithm::DogLeg,
        initial_radius: 1e3,
        ..Options::default()
    };
    let out = solve(&sk, options);
    eprintln!(
        "overshoot: iterations={} evaluations={} rejected={}",
        out.iterations, out.evaluations, out.rejected
    );
    assert!(out.converged, "{out:?}");
    assert!(out.rejected > 0);
    assert!(check(&sk, &out.coordinates).is_clean());
    // The branch the guess is on: y > 0.
    for (c, want) in [(Coordinate::X, 3.6), (Coordinate::Y, 4.8)] {
        assert!((out.coordinates[&(p.id(), c)] - want).abs() < COORDINATE_ERROR);
    }
    for algorithm in ALGORITHMS {
        let out = solve(&sk, with(algorithm));
        assert!(out.converged, "{algorithm:?} {out:?}");
        assert!(check(&sk, &out.coordinates).is_clean());
        assert!((out.coordinates[&(p.id(), Coordinate::Y)] - 4.8).abs() < COORDINATE_ERROR);
    }
}

#[test]
fn singular_start_is_stagnation_not_convergence() {
    // At (4, 0) both residuals are -2 (|p| = 4 against 6, |p - q| = 6 against
    // 8) and the two gradients are opposite and horizontal: J J^T is singular,
    // J^T f = 0 and neither branch is preferred. No step can be justified;
    // the start is returned bit for bit as best effort.
    let (sk, _, ids) = trilateration(4.0, 0.0);
    for algorithm in ALGORITHMS {
        let out = solve(&sk, with(algorithm));
        assert!(!out.converged, "{algorithm:?}");
        assert_eq!(out.coordinates, sk.coordinates());
        assert!(out.iterations <= MAX_ITERATIONS);
        assert_eq!(constraint_ids(check(&sk, &out.coordinates)), ids);
    }
}

#[test]
fn dog_leg_branches_stay_in_the_region() {
    // f = (1, 1), J = diag(1, 10): GN step (-1, -0.1), length 1.005;
    // g = (1, 10), Cauchy step length |g|^3 / |Jg|^2 = 101^1.5 / 10001 ≈ 0.1005.
    let e = Evaluation {
        values: vec![1.0, 1.0],
        jacobian: vec![vec![1.0, 0.0], vec![0.0, 10.0]],
        valid: true,
        satisfied: false,
    };
    let gn = gauss_newton(&e, GN_CUTOFF).unwrap();
    for radius in [2.0, 0.5, 0.05] {
        let h = dog_leg(&e, radius).unwrap();
        let length = norm(&h);
        if radius > norm(&gn) {
            assert!(
                (h[0] - gn[0]).abs() < COORDINATE_ERROR && (h[1] - gn[1]).abs() < COORDINATE_ERROR
            );
        } else {
            assert!(
                (length - radius).abs() < COORDINATE_ERROR * radius,
                "{radius}: {length}"
            );
        }
        assert!(predicted_decrease(&e, &h) > 0.0);
    }
    // Below the Cauchy length the step is steepest descent: parallel to -g.
    let h = dog_leg(&e, 0.05).unwrap();
    assert!((h[1] / h[0] - 10.0).abs() < COORDINATE_ERROR);
    // A rank-deficient Jacobian: two identical rows (two fixes of one x).
    // The pseudo-inverse lands on their midpoint and moves nothing outside
    // the row space (y untouched).
    let e = Evaluation {
        values: vec![-1.0, -2.0],
        jacobian: vec![vec![1.0, 0.0], vec![1.0, 0.0]],
        valid: true,
        satisfied: false,
    };
    let h = gauss_newton(&e, GN_CUTOFF).unwrap();
    const ROUNDING: f64 = 1e-15; // a few ulps of an order-one step
    assert!(h[1].abs() <= ROUNDING, "{h:?}");
    assert!((h[0] - 1.5).abs() <= ROUNDING, "{h:?}");
}

#[test]
fn c1_twins_under_both_engines() {
    // Redundant twins are consistent: they solve to their base's coordinates.
    // Conflicting twins cannot: finite best effort, not converged, and the
    // checker flags something. Conflict sets are step 6's.
    for slug in [
        "sss-triangle-redundant",
        "belt-redundant",
        "slotted-plate-redundant",
        "sss-triangle-conflicting",
        "belt-conflicting",
        "slotted-plate-conflicting",
    ] {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/c1")
            .join(slug);
        let fixture = Fixture::load(&path).unwrap();
        let sk: Sketch =
            serde_json::from_str(&serde_json::to_string(&fixture.sketch).unwrap()).unwrap();
        for algorithm in ALGORITHMS {
            let out = solve(&sk, with(algorithm));
            eprintln!(
                "{algorithm:?} {slug}: converged={} iterations={} evaluations={} rejected={}",
                out.converged, out.iterations, out.evaluations, out.rejected
            );
            assert!(out.coordinates.values().all(|x| x.is_finite()));
            let report = check(&sk, &out.coordinates);
            if slug.ends_with("redundant") {
                assert!(out.converged, "{algorithm:?} {slug}");
                assert!(report.is_clean(), "{algorithm:?} {slug}: {report:?}");
                let public = fixture_at(&sk, &out.coordinates);
                for (key, want) in &fixture.expected.coordinates {
                    assert!(
                        (public.expected.coordinates[key] - want).abs()
                            <= fixture.expected.tolerance,
                        "{algorithm:?} {slug}: {key:?}"
                    );
                }
            } else {
                assert!(!out.converged, "{algorithm:?} {slug}");
                assert!(!report.violations.is_empty());
            }
        }
    }
}
