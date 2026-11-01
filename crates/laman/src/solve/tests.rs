#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use super::*;
use crate::model::Coordinate;

use laman_debug::{dump, fixture::Fixture};
use std::path::PathBuf;

const COORDINATE_ERROR: f64 = 1e-9; // Closed-form fixture acceptance, independent of solver thresholds.

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
    let mut sk = Sketch::new();
    let a = sk.point(0.0, 0.0).unwrap();
    let b = sk.point(3.0, 4.0).unwrap();
    sk.distance(a, b, 10.0).unwrap();
    let stray = sk.point(-0.0, 200.0).unwrap();
    let out = lm(&sk, Options::default());
    assert!(out.converged, "{out:?}");
    assert!(check(&sk, &out.coordinates).is_clean());
    for (id, xy) in [(a.id(), [-1.5, -2.0]), (b.id(), [4.5, 6.0])] {
        for (c, v) in [Coordinate::X, Coordinate::Y].into_iter().zip(xy) {
            assert!((out.coordinates[&(id, c)] - v).abs() < COORDINATE_ERROR);
        }
    }
    assert_eq!(
        out.coordinates[&(stray.id(), Coordinate::X)].to_bits(),
        (-0.0f64).to_bits()
    );
    inspect(&sk, &out, "lm-distance");
    let mut solved = Sketch::new();
    let a = solved.point(-1.5, -2.0).unwrap();
    let b = solved.point(4.5, 6.0).unwrap();
    solved.distance(a, b, 10.0).unwrap();
    let out = lm(&solved, Options::default());
    assert!(out.converged);
    assert!(check(&solved, &out.coordinates).is_clean());
    assert_eq!(out.iterations, 0);
    assert_eq!(out.coordinates, solved.coordinates());
}

#[test]
fn c1_contacts_from_guesses() {
    for slug in ["sss-triangle", "belt", "slotted-plate"] {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/c1")
            .join(slug);
        let fixture = Fixture::load(&path).unwrap();
        let sk: Sketch =
            serde_json::from_str(&serde_json::to_string(&fixture.sketch).unwrap()).unwrap();
        let out = lm(&sk, Options::default());
        inspect(&sk, &out, &format!("lm-{slug}"));
        eprintln!(
            "{slug}: iterations={} evaluations={} rejected={}",
            out.iterations, out.evaluations, out.rejected
        );
        assert!(out.converged, "{slug}: {out:?}");
        assert!(
            check(&sk, &out.coordinates).is_clean(),
            "{slug}: {:?}",
            check(&sk, &out.coordinates)
        );
        let solved = reseed(&sk, &out.coordinates);
        let again = lm(&solved, Options::default());
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

#[test]
fn budgets_fixed_only_and_finite_extremes() {
    let mut sk = Sketch::new();
    let o = sk.point_fixed(0.0, 0.0).unwrap();
    let p = sk.point(3.0, 4.0).unwrap();
    let id = sk.distance(o, p, 10.0).unwrap();
    for options in [
        Options {
            max_iterations: 0,
            ..Options::default()
        },
        Options {
            max_evaluations: 0,
            ..Options::default()
        },
        Options {
            max_evaluations: 2,
            ..Options::default()
        },
    ] {
        let out = lm(&sk, options);
        assert!(!out.converged);
        assert!(out.evaluations <= options.max_evaluations);
        assert!(out.iterations <= options.max_iterations);
        let report = check(&sk, &out.coordinates);
        assert_eq!(constraint_ids(report), [id.0]);
        assert!(out.coordinates.values().all(|x| x.is_finite()));
        assert!(
            (libm::hypot(
                out.coordinates[&(p.id(), Coordinate::X)],
                out.coordinates[&(p.id(), Coordinate::Y)]
            ) - 10.0)
                .abs()
                <= 5.0
        );
    }
    for scale in [1e-300, 1e300] {
        let mut sk = Sketch::new();
        let a = sk.point_fixed(0.0, 0.0).unwrap();
        let b = sk.point(3.0 * scale, 4.0 * scale).unwrap();
        sk.distance(a, b, 5.0 * scale).unwrap();
        let out = lm(&sk, Options::default());
        assert!(out.converged);
        assert!(check(&sk, &out.coordinates).is_clean());
        assert_eq!(out.iterations, 0);
    }
    let mut sk = Sketch::new();
    let a = sk.point_fixed(-f64::MAX, 0.0).unwrap();
    let b = sk.point_fixed(f64::MAX, 0.0).unwrap();
    let id = sk.distance(a, b, 1.0).unwrap();
    let out = lm(&sk, Options::default());
    assert!(!out.converged);
    assert_eq!(out.coordinates, sk.coordinates());
    assert_eq!(constraint_ids(check(&sk, &out.coordinates)), [id.0]);
    let out = lm(&Sketch::new(), Options::default());
    assert!(out.converged);
    assert!(check(&Sketch::new(), &out.coordinates).is_clean());
}

#[test]
fn rejects_steps_and_retains_finite_best_effort() {
    let mut sk = Sketch::new();
    let p = sk.point(0.0, 0.0).unwrap();
    let a = sk.fix(p, 1.0, 0.0).unwrap();
    let b = sk.fix(p, 2.0, 0.0).unwrap();
    let out = lm(&sk, Options::default());
    eprintln!(
        "conflicting fixes: iterations={} evaluations={} rejected={}",
        out.iterations, out.evaluations, out.rejected
    );
    assert!(!out.converged);
    assert!(out.rejected > 0);
    assert!(out.iterations <= MAX_ITERATIONS);
    assert!((out.coordinates[&(p.id(), Coordinate::X)] - 1.5).abs() < COORDINATE_ERROR);
    assert_eq!(constraint_ids(check(&sk, &out.coordinates)), [a.0, b.0]);
    inspect(&sk, &out, "lm-conflicting-fixes");
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
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/regression/lm-coupled-stopping");
    let f = Fixture::load(&path).unwrap();
    let sk: Sketch = serde_json::from_str(&serde_json::to_string(&f.sketch).unwrap()).unwrap();
    let out = lm(&sk, Options::default());
    assert!(out.converged);
    assert!(check(&sk, &out.coordinates).is_clean());
    let actual = fixture_at(&sk, &out.coordinates);
    for (key, want) in &f.expected.coordinates {
        assert!((actual.expected.coordinates[key] - want).abs() <= f.expected.tolerance);
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
    let out = lm(&sk, Options::default());
    assert!(!out.converged);
    assert_eq!(out.coordinates, sk.coordinates());
    assert_eq!(constraint_ids(check(&sk, &out.coordinates)), [id.0]);
    inspect(&sk, &out, "lm-wrong-sense");
}

#[test]
fn extreme_nonzero_steps_and_nonfinite_transform() {
    let mut sk = Sketch::new();
    let a = sk.point_fixed(0.0, 0.0).unwrap();
    let b = sk.point(3e300, 4e300).unwrap();
    sk.distance(a, b, 1e301).unwrap();
    let out = lm(&sk, Options::default());
    assert!(out.converged);
    assert!(out.iterations > 0);
    assert!(check(&sk, &out.coordinates).is_clean());
    assert!(out.coordinates.values().all(|x| x.is_finite()));
    let mut sk = Sketch::new();
    let a = sk.point_fixed(-f64::MAX, 0.0).unwrap();
    let b = sk.point(f64::MAX, 0.0).unwrap();
    let id = sk.distance(a, b, 1.0).unwrap();
    let out = lm(&sk, Options::default());
    assert!(!out.converged);
    assert_eq!(out.coordinates, sk.coordinates());
    assert_eq!(constraint_ids(check(&sk, &out.coordinates)), [id.0]);
}

#[test]
fn vector_acceptance_is_not_two_scalar_acceptances() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/regression/lm-vector-stop");
    let f = Fixture::load(&path).unwrap();
    let sk: Sketch = serde_json::from_str(&serde_json::to_string(&f.sketch).unwrap()).unwrap();
    let out = lm(&sk, Options::default());
    assert!(out.converged);
    assert!(check(&sk, &out.coordinates).is_clean());
    assert!(out.iterations > 0);
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
        let out=lm(&sk,Options::default());
        proptest::prop_assert!(out.converged);
        proptest::prop_assert!(check(&sk,&out.coordinates).is_clean());
        for (coordinate,want) in [(Coordinate::X,x*scale*multiplier),(Coordinate::Y,y*scale*multiplier)] {
            proptest::prop_assert!((out.coordinates[&(b.id(),coordinate)]-want).abs()<COORDINATE_ERROR*scale.max(1.0));
        }
    }
}
