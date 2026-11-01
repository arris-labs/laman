//! Independent checker acceptance under the frozen model policy.
use laman::model::{
    Coordinate, Sketch,
    tolerance::{self, Tolerances},
};
use laman_check::check;

#[test]
fn scaled_translated_and_unrelated_geometry() {
    for scale in [1e-6, 1.0, 1e6, 1e300] {
        let mut sk = Sketch::new();
        let a = sk.point_fixed(10.0 * scale, 10.0 * scale).unwrap();
        let b = sk.point(13.0 * scale, 14.0 * scale).unwrap();
        let id = sk.distance(a, b, 5.0 * scale).unwrap();
        let policy = Tolerances::new(&sk);
        assert!(check(&sk, &sk.coordinates()).is_clean());
        let before = policy.constraint(id);
        sk.point(-1e300, 1e300).unwrap();
        assert_eq!(before, Tolerances::new(&sk).constraint(id));
        let mut values = sk.coordinates();
        *values.get_mut(&(b.id(), Coordinate::X)).unwrap() += 100.0 * before;
        assert_eq!(check(&sk, &values).violations.len(), 1);
    }
}

#[test]
fn fixed_targets_overflow_and_floor() {
    let mut sk = Sketch::new();
    let a = sk.point_fixed(-f64::MAX, 0.0).unwrap();
    let b = sk.point_fixed(f64::MAX, 0.0).unwrap();
    let id = sk.distance(a, b, 1.0).unwrap();
    assert_eq!(
        Tolerances::new(&sk).constraint(id),
        tolerance::length(f64::MAX)
    );
    assert_eq!(check(&sk, &sk.coordinates()).violations.len(), 1);
    let mut sk = Sketch::new();
    let p = sk.point(0.0, 0.0).unwrap();
    let id = sk.fix(p, 1e6, 0.0).unwrap();
    assert_eq!(Tolerances::new(&sk).constraint(id), tolerance::length(1e6));
    let mut sk = Sketch::new();
    let p = sk.point(1e-300, 0.0).unwrap();
    assert_eq!(Tolerances::new(&sk).entity(p.id()), tolerance::LENGTH_FLOOR);
}

#[test]
fn common_ground_does_not_join_independent_equations() {
    let mut sk = Sketch::new();
    let o = sk.point_fixed(0.0, 0.0).unwrap();
    let a = sk.point(1.0, 0.0).unwrap();
    let b = sk.point(1e6, 0.0).unwrap();
    let short = sk.distance(o, a, 1.0).unwrap();
    sk.distance(o, b, 1e6).unwrap();
    assert_eq!(
        Tolerances::new(&sk).constraint(short),
        tolerance::length(1.0)
    );
}

#[test]
fn checked_coordinates_not_guesses_set_the_scale() {
    let mut sk = Sketch::new();
    let o = sk.point_fixed(0.0, 0.0).unwrap();
    let far = sk.point(1e6, 0.0).unwrap();
    let other = sk.point(0.0, 1.0).unwrap();
    let unit = sk.distance(o, far, 1.0).unwrap();
    sk.distance(o, other, 1.0).unwrap();
    let mut values = sk.coordinates();
    values.insert((far.id(), Coordinate::X), 1.0);
    assert_eq!(
        Tolerances::new(&sk).constraint(unit),
        tolerance::length(1e6)
    );
    assert_eq!(
        Tolerances::at(&sk, &values).constraint(unit),
        tolerance::length(1.0)
    );
    // A non-finite value enters no extent: it cannot loosen its component.
    values.insert((far.id(), Coordinate::X), f64::INFINITY);
    assert_eq!(
        Tolerances::at(&sk, &values).constraint(unit),
        tolerance::length(1.0)
    );
    values.insert((other.id(), Coordinate::Y), 1.0 + 1e-6);
    let report = check(&sk, &values);
    assert_eq!(report.violations.len(), 2, "{report:?}");
}
