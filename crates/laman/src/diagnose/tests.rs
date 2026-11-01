#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use super::*;
use crate::model::{Constraint, Sketch};
use crate::solve::{Options, solve};
use laman_debug::fixture::Fixture;
use std::path::PathBuf;

/// How far a free direction may leave the regularized Jacobian's null space,
/// and a basis its orthonormality: rounding of order-one sums over the C1
/// sizes, far below the rank threshold that decides them.
const NULL_RESIDUAL: f64 = 1e-12;

/// The fixture's sketch in this crate's types (the test links two `laman`s),
/// and its expected coordinates laid over the sketch's guesses: the closed
/// form, never laman's output.
fn load(slug: &str) -> (Sketch, Coordinates, Fixture) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/c1")
        .join(slug);
    let fixture = Fixture::load(&path).unwrap();
    let sk: Sketch =
        serde_json::from_str(&serde_json::to_string(&fixture.sketch).unwrap()).unwrap();
    let mut values = sk.coordinates();
    for ((id, coordinate), value) in &fixture.expected.coordinates {
        // The other crate's Coordinate is not nameable here; its Debug is.
        let coordinate = match format!("{coordinate:?}").as_str() {
            "X" => Coordinate::X,
            "Y" => Coordinate::Y,
            "R" => Coordinate::R,
            other => panic!("unknown coordinate {other}"),
        };
        values.insert((EntityId(id.0), coordinate), *value);
    }
    (sk, values, fixture)
}

/// Every direction is unit, orthogonal to the others and in the null space
/// of the regularized Jacobian, and its first component is positive.
fn assert_null_basis(sk: &Sketch, values: &Coordinates, f: &Freedom) {
    let system = System::new(sk);
    let dense = |d: &[(EntityId, Coordinate, f64)]| -> BTreeMap<Param, f64> {
        d.iter().map(|(id, c, x)| ((*id, *c), *x)).collect()
    };
    for (i, d) in f.directions.iter().enumerate() {
        assert!(d.first().unwrap().2 > 0.0, "{d:?}");
        let di = dense(d);
        for e in &f.directions[..=i] {
            let ej = dense(e);
            let p = di
                .iter()
                .fold(0.0, |s, (k, x)| s + x * ej.get(k).copied().unwrap_or(0.0));
            let want = if std::ptr::eq(d, e) { 1.0 } else { 0.0 };
            assert!((p - want).abs() < NULL_RESIDUAL, "{p} {d:?} {e:?}");
        }
        for component in &system.components {
            let e = system.evaluate(component, values);
            for row in e.jacobian.into_iter().filter_map(unit_row) {
                let j = component
                    .params
                    .iter()
                    .zip(&row)
                    .fold(0.0, |s, (p, r)| s + r * di.get(p).copied().unwrap_or(0.0));
                assert!(j.abs() < NULL_RESIDUAL, "{j} {d:?}");
            }
        }
    }
}

#[test]
fn c1_bases_and_twins_have_their_construction_dof() {
    for slug in [
        "sss-triangle",
        "belt",
        "slotted-plate",
        "sss-triangle-redundant",
        "belt-redundant",
        "slotted-plate-redundant",
        "slotted-plate-drag",
    ] {
        let (sk, values, fixture) = load(slug);
        let want = fixture.expected.dof as usize;
        let f = freedom(&sk, &values);
        assert_eq!(f.dof, want, "{slug}");
        assert_eq!(f.directions.len(), want, "{slug}");
        assert_null_basis(&sk, &values, &f);
        if want == 0 {
            assert!(f.entities.values().all(|d| *d == 0), "{slug}: {f:?}");
        }
        // The solver's own solution has the same freedom as the closed form.
        let out = solve(&sk, Options::default());
        assert!(out.converged, "{slug}");
        let solved = freedom(&sk, &out.coordinates);
        assert_eq!(solved.dof, want, "{slug}");
        assert_eq!(solved.entities, f.entities, "{slug}");
    }
}

#[test]
fn drag_plate_frees_its_width_and_the_stray_point() {
    // ROADMAP fixture 4 "Before": the plate's width and Q's two.
    let (sk, values, _) = load("slotted-plate-drag");
    let f = freedom(&sk, &values);
    let q = EntityId(21);
    assert_eq!(f.entities[&q], 2);
    // P1, P2 and the midpoints follow the width; the slot is centred on
    // the axis, so all six of its points follow it too. Origin and P3 do not.
    for id in [0, 3] {
        assert_eq!(f.entities[&EntityId(id)], 0, "{id}");
    }
    for id in [1, 2, 8, 9, 11, 12, 13, 15, 16, 17] {
        assert_eq!(f.entities[&EntityId(id)], 1, "{id}");
    }
    assert!(f.directions.contains(&vec![(q, Coordinate::X, 1.0)]));
    assert!(f.directions.contains(&vec![(q, Coordinate::Y, 1.0)]));
    // The width direction is a pure x motion: P1 and P2 by w, the midpoints
    // and the slot by w / 2.
    let width = f.directions.iter().find(|d| d[0].0 != q).unwrap();
    assert!(
        width.iter().all(|(_, c, _)| *c == Coordinate::X),
        "{width:?}"
    );
    let along = |id: u32| width.iter().find(|(e, _, _)| *e == EntityId(id)).unwrap().2;
    assert_eq!(width.len(), 10, "{width:?}");
    assert!((along(1) - along(2)).abs() < NULL_RESIDUAL);
    for id in [8, 9, 11, 12, 13, 15, 16, 17] {
        assert!((along(1) - 2.0 * along(id)).abs() < NULL_RESIDUAL, "{id}");
    }
}

#[test]
fn original_tangents_overcount_but_are_necessary() {
    // ADR-0002: at a structural contact, raw Jacobian nullity overcounts
    // (belt 12 - 8, plate 22 - 18) although every tangent is essential.
    for (slug, raw) in [("sss-triangle", 0), ("belt", 4), ("slotted-plate", 4)] {
        let (sk, values, _) = load(slug);
        let mut system = System::new(&sk);
        for eq in &mut system.equations {
            eq.contact = None;
        }
        assert_eq!(freedom_of(&system, &values).dof, raw, "{slug}");
        // Removing any one constraint (contacts recompiled) frees exactly
        // its equations: none, a tangent included, is redundant.
        for (id, c) in sk.constraints() {
            let k = crate::residual::equations(&sk, &values, id)
                .unwrap()
                .as_slice()
                .len();
            let mut less = sk.clone();
            less.remove_constraint(id).unwrap();
            let f = freedom(&less, &values);
            assert_eq!(f.dof, k, "{slug}: without {id:?} {c:?}");
            assert_null_basis(&less, &values, &f);
        }
    }
}

#[test]
fn removing_membership_or_tangency_restores_freedom() {
    // Belt: drop t1's membership of c1 and the contact is no longer
    // recognized; the original tangent is regular there and t1 slides along
    // the line (one DoF). Drop both membership and tangency of t1: two.
    let (sk, values, _) = load("belt");
    let member = sk
        .constraints()
        .find(|(_, c)| matches!(c, Constraint::PointOnCircle { .. }))
        .unwrap()
        .0;
    let tangent = sk
        .constraints()
        .find(|(_, c)| matches!(c, Constraint::TangentLineCircle { .. }))
        .unwrap()
        .0;
    let mut a = sk.clone();
    a.remove_constraint(member).unwrap();
    assert!(
        System::new(&a)
            .equations
            .iter()
            .filter(|e| e.contact.is_some())
            .count()
            < System::new(&sk)
                .equations
                .iter()
                .filter(|e| e.contact.is_some())
                .count()
    );
    assert_eq!(freedom(&a, &values).dof, 1);
    let mut b = sk.clone();
    b.remove_constraint(tangent).unwrap();
    assert_eq!(freedom(&b, &values).dof, 1);
    let Some(Constraint::TangentLineCircle { line, round, .. }) = sk.constraint(tangent).copied()
    else {
        panic!("not a tangent")
    };
    let Some(crate::model::Entity::Line { a: la, b: lb }) = sk.entity(line.id()).copied() else {
        panic!("not a line")
    };
    let m = sk
        .constraints()
        .find_map(|(id, c)| {
            matches!(*c, Constraint::PointOnCircle { p, on } if on == round && (p == la || p == lb))
                .then_some(id)
        })
        .unwrap();
    let mut c = sk.clone();
    c.remove_constraint(tangent).unwrap();
    c.remove_constraint(m).unwrap();
    assert_eq!(freedom(&c, &values).dof, 2);
}

#[test]
fn ungrounded_rigid_shapes_and_isolated_points() {
    assert_eq!(
        freedom(&Sketch::new(), &Coordinates::new()),
        Freedom {
            dof: 0,
            entities: BTreeMap::new(),
            directions: vec![]
        }
    );
    let mut sk = Sketch::new();
    let p = sk.point(3.0, -4.0).unwrap();
    let fixed = sk.point_fixed(1.0, 1.0).unwrap();
    let f = freedom(&sk, &sk.coordinates());
    assert_eq!(f.dof, 2);
    assert_eq!(
        f.directions,
        [
            vec![(p.id(), Coordinate::X, 1.0)],
            vec![(p.id(), Coordinate::Y, 1.0)]
        ]
    );
    assert_eq!(f.entities[&p.id()], 2);
    assert_eq!(f.entities[&fixed.id()], 0);

    // A 3-4-5 triangle with no ground: the plane's three motions, at every
    // scale and translation; each point 2, each line 3, a free circle 3 and
    // a dimensioned one 2.
    for (scale, shift) in [(1e-6, 0.0), (1.0, 0.0), (1.0, 1e4), (1e6, -3e9)] {
        let mut sk = Sketch::new();
        let pts: Vec<_> = [(0.0, 0.0), (3.0, 0.0), (0.0, 4.0)]
            .into_iter()
            .map(|(x, y)| sk.point(shift + x * scale, shift + y * scale).unwrap())
            .collect();
        for (i, j, d) in [(0, 1, 3.0), (1, 2, 5.0), (2, 0, 4.0)] {
            sk.distance(pts[i], pts[j], d * scale).unwrap();
        }
        let line = sk.line(pts[0], pts[1]).unwrap();
        let free = sk.circle(pts[2], scale).unwrap();
        let c = sk.point(shift, shift + 9.0 * scale).unwrap();
        let sized = sk.circle(c, scale).unwrap();
        sk.radius(sized, 2.0 * scale).unwrap();
        let values = {
            let mut v = sk.coordinates();
            v.insert((sized.id(), Coordinate::R), 2.0 * scale);
            v
        };
        let f = freedom(&sk, &values);
        assert_eq!(f.dof, 3 + 1 + 2, "{scale} {shift}");
        for p in &pts {
            assert_eq!(f.entities[&p.id()], 2);
        }
        assert_eq!(f.entities[&line.id()], 3);
        assert_eq!(f.entities[&free.id()], 3);
        assert_eq!(f.entities[&sized.id()], 2);
        assert_null_basis(&sk, &values, &f);
        // Deterministic: the same input gives the same bits.
        assert_eq!(freedom(&sk, &values), f);
    }
}

#[test]
fn extremes_return_finite_freedom() {
    let mut sk = Sketch::new();
    let a = sk.point_fixed(0.0, 0.0).unwrap();
    let b = sk.point(3e300, 4e300).unwrap();
    sk.distance(a, b, 5e300).unwrap();
    let f = freedom(&sk, &sk.coordinates());
    assert_eq!(f.dof, 1);
    assert!(f.directions.iter().flatten().all(|(_, _, x)| x.is_finite()));
    // Unrepresentable geometry contributes no rank; no panic, finite output.
    let mut sk = Sketch::new();
    let a = sk.point_fixed(-f64::MAX, 0.0).unwrap();
    let b = sk.point(f64::MAX, 0.0).unwrap();
    sk.distance(a, b, 1.0).unwrap();
    let f = freedom(&sk, &sk.coordinates());
    assert_eq!(f.dof, 2);
    assert!(f.directions.iter().flatten().all(|(_, _, x)| x.is_finite()));
}

#[test]
fn rank_threshold_ties_and_sign() {
    // ADR-0003: a direction 100 times above the rank threshold counts, 100
    // times below does not; a zero row adds nothing; a tiny row is unit first.
    let rank =
        |rows: Vec<Vec<f64>>| Qr::new(rows.into_iter().filter_map(unit_row).collect(), 2).rank;
    assert_eq!(rank(vec![vec![1., 0.], vec![1., 100.0 * RANK]]), 2);
    assert_eq!(rank(vec![vec![1., 0.], vec![1., RANK / 100.0]]), 1);
    assert_eq!(rank(vec![vec![0., 0.]]), 0);
    assert_eq!(rank(vec![vec![1e-300, 0.]]), 1);
    // Equal pivots go to the lowest column; the null vector comes from the
    // later one, signed so its first component is positive.
    let qr = Qr::new(vec![unit_row(vec![1., 1.]).unwrap()], 2);
    assert_eq!(qr.perm[0], 0);
    let basis = qr.null_basis();
    let h = std::f64::consts::FRAC_1_SQRT_2;
    assert_eq!(basis.len(), 1);
    assert!((basis[0][0] - h).abs() < NULL_RESIDUAL && (basis[0][1] + h).abs() < NULL_RESIDUAL);
    // Canonical zeros are +0.
    let qr = Qr::new(vec![vec![0., -1.]], 2);
    assert_eq!(qr.null_basis(), [vec![1.0, 0.0]]);
    assert_eq!(qr.null_basis()[0][1].to_bits(), 0.0f64.to_bits());
}
