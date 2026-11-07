#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use super::*;
use crate::model::{Constraint, Sketch};
use crate::solve::{Options, solve};
use laman_debug::fixture::Fixture;
use proptest::strategy::Strategy as _;
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
        let solved = freedom(&sk, &out.map());
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

/// The staged freedom of `sk` at `values` next to the whole path's.
fn staged_and_whole(sk: &Sketch, values: &Coordinates) -> (Freedom, Freedom) {
    let system = System::new(sk);
    (
        freedom_on(&system, values, Path::Staged, &StagedRanks::default()),
        freedom_on(&system, values, Path::Whole, &StagedRanks::default()),
    )
}

#[test]
fn the_staged_path_runs_on_the_c1_bases() {
    // Not a vacuous comparison: every C1 base's components of more than one
    // part are factorized part by part at the expected coordinates, and give
    // the whole path's DoF and per-entity DoF.
    for slug in [
        "sss-triangle",
        "belt",
        "slotted-plate",
        "slotted-plate-drag",
    ] {
        let (sk, values, _) = load(slug);
        let system = System::new(&sk);
        let coordinates = merged(&sk, &values);
        let mut staged_components = 0;
        for component in &system.components {
            if crate::stage::decompose(&system, component).parts.len() < 2 {
                continue;
            }
            let (rank, _) = staged(&system, component, &coordinates, true)
                .unwrap_or_else(|| panic!("{slug}: fell back"));
            assert_eq!(rank, rank_of(&system, component, &coordinates), "{slug}");
            staged_components += 1;
        }
        assert!(staged_components > 0, "{slug}");
        let (s, w) = staged_and_whole(&sk, &values);
        assert_eq!((s.dof, &s.entities), (w.dof, &w.entities), "{slug}");
        assert_null_basis(&sk, &values, &s);
    }
}

#[test]
fn a_column_deficient_head_falls_back_to_the_whole_component() {
    // P on the x axis, 5, 5 and 15 from fixed (0,0), (10,0), (20,0): three
    // rows on P's two columns, all along x, so the head has rank 1. Q, 3
    // above P, reads P after it. By construction P.x and Q's distance are
    // held (rank 2 of 4) and P.y moves with Q.y; staged without the
    // fallback would pin P.y. The fallback is the whole path, bit for bit.
    let mut sk = Sketch::new();
    let a = sk.point_fixed(0.0, 0.0).unwrap();
    let b = sk.point_fixed(10.0, 0.0).unwrap();
    let c = sk.point_fixed(20.0, 0.0).unwrap();
    let p = sk.point(5.0, 0.0).unwrap();
    let q = sk.point(5.0, 3.0).unwrap();
    sk.distance(a, p, 5.0).unwrap();
    sk.distance(b, p, 5.0).unwrap();
    sk.distance(c, p, 15.0).unwrap();
    sk.distance(p, q, 3.0).unwrap();
    let values = sk.coordinates();
    let system = System::new(&sk);
    let component = system
        .components
        .iter()
        .find(|c| !c.sources.is_empty())
        .unwrap();
    let kinds: Vec<_> = crate::stage::decompose(&system, component)
        .parts
        .iter()
        .map(|part| part.kind)
        .collect();
    assert_eq!(
        kinds,
        [crate::stage::Kind::Head, crate::stage::Kind::Remainder]
    );
    assert_eq!(staged(&system, component, &values, true), None);
    let (s, w) = staged_and_whole(&sk, &values);
    assert_eq!(s, w);
    assert_eq!(s.dof, 2);
    assert_eq!(s.entities[&p.id()], 1);
    assert!(
        s.directions.iter().any(|d| d
            .iter()
            .any(|(id, c, _)| *id == p.id() && *c == Coordinate::Y)),
        "{:?}",
        s.directions
    );
    assert_null_basis(&sk, &values, &s);
}

#[test]
fn a_row_dependent_on_earlier_parts_adds_no_rank() {
    // The shrunk seed 226646 of `solved.rs`: L = (P2, P0) and M = (P2, P1)
    // vertical, P1 on L, P0.x fixed. The verticals put P2 and P1 on x = 0,
    // where P1 is on L whatever its y: the point-on-line row lies in the
    // verticals' span, so the DoF is 5 - 2 = 3. Its entries on the
    // remainder's own columns are rounding at the solver's answer from the
    // seed's guesses; a row normalized over them alone would count a rank
    // the whole path does not.
    let mut sk = Sketch::new();
    let p0 = sk.point(0.0, 0.0).unwrap();
    sk.set_fixed(p0.id(), Coordinate::X, true).unwrap();
    let p1 = sk.point(0.0, 0.0).unwrap();
    let p2 = sk.point(161.2587271685016, -605.1274985744651).unwrap();
    let l = sk.line(p2, p0).unwrap();
    let m = sk.line(p2, p1).unwrap();
    sk.point_on_line(p1, l).unwrap();
    sk.vertical(m).unwrap();
    sk.vertical(l).unwrap();
    let out = solve(&sk, Options::default());
    assert!(out.converged);
    let values = out.map();
    let (s, w) = staged_and_whole(&sk, &values);
    assert_eq!(s.dof, 3);
    assert_eq!((s.dof, &s.entities), (w.dof, &w.entities));
    assert_null_basis(&sk, &values, &s);
}

/// This crate's sketch from the debug crate's (the test links two `laman`s).
fn ours(sketch: &impl serde::Serialize) -> Sketch {
    serde_json::from_str(&serde_json::to_string(sketch).unwrap()).unwrap()
}

/// Every solution a C2 case's solve, warm-started edit and drag return,
/// both algorithms, as their `Debug` text: every coordinate's and every
/// diagnostic's bits.
fn frames(
    sketch: &Sketch,
    edit: Option<(&str, f64)>,
    drag: Option<(&Sketch, EntityId, &[[f64; 2]])>,
) -> Vec<String> {
    use crate::model::Constraint;
    use crate::solution::{Algorithm, SolverOptions};
    let mut out = Vec::new();
    for algorithm in [Algorithm::LevenbergMarquardt, Algorithm::DogLeg] {
        let options = SolverOptions {
            algorithm,
            ..SolverOptions::default()
        };
        let initial = sketch.solve(&options);
        out.push(format!("{initial:?}"));
        if let Some((tag, value)) = edit {
            let mut edited = sketch.clone();
            edited.apply(&initial).unwrap();
            let (id, constraint) = edited
                .constraints()
                .find(|(id, _)| edited.constraint_tag_of(*id) == Some(tag))
                .map(|(id, c)| (id, *c))
                .unwrap();
            let replacement = if let Constraint::Distance { p, q, .. } = constraint {
                Constraint::Distance { p, q, value }
            } else if let Constraint::PointLineDistance { p, line, sense, .. } = constraint {
                Constraint::PointLineDistance {
                    p,
                    line,
                    sense,
                    value,
                }
            } else {
                panic!("a corpus edit names a distance dimension")
            };
            edited.remove_constraint(id).unwrap();
            edited.add_constraint_with_id(id, replacement).unwrap();
            edited.tag_constraint(id, tag).unwrap();
            out.push(format!("{:?}", edited.solve(&options)));
        }
        if let Some((sketch, point, targets)) = drag {
            let mut session = sketch.drag(point, &options).unwrap();
            for target in targets {
                out.push(format!("{:?}", session.to(*target)));
            }
        }
    }
    out
}

/// The shared staged factorizations against the three-pass reference: the
/// same solutions, bit for bit (plans/m8-frame-budget step 2).
fn assert_shared_is_three_pass(name: &str, case: &laman_debug::corpus::Case) {
    let staged = shared_is_three_pass(name, case);
    assert!(
        staged > 0,
        "{name}: no component was staged, the comparison is vacuous"
    );
}

/// The comparison of [`assert_shared_is_three_pass`], returning how many
/// staged factorizations the shared run made.
fn shared_is_three_pass(name: &str, case: &laman_debug::corpus::Case) -> usize {
    let sketch = ours(&case.sketch);
    let drag = case.drag.as_ref().map(|d| {
        (
            ours(&d.sketch),
            EntityId(d.drag.dragged.0),
            d.drag.targets.clone(),
        )
    });
    let run = || {
        frames(
            &sketch,
            Some((&case.edit.tag, case.edit.value)),
            drag.as_ref().map(|(s, p, t)| (s, *p, t.as_slice())),
        )
    };
    let _ = three_pass::staged_made();
    let shared = run();
    let staged = three_pass::staged_made();
    let reference = three_pass::run(run);
    assert_eq!(shared.len(), reference.len(), "{name}");
    for (i, (s, r)) in shared.iter().zip(&reference).enumerate() {
        assert!(
            s == r,
            "{name}: frame {i} differs from the three-pass reference"
        );
    }
    staged
}

#[test]
fn shared_ranks_are_the_three_pass_ones_on_c1() {
    for slug in [
        "sss-triangle",
        "belt",
        "slotted-plate",
        "sss-triangle-redundant",
        "belt-redundant",
        "slotted-plate-redundant",
        "sss-triangle-conflicting",
        "belt-conflicting",
        "slotted-plate-conflicting",
        "slotted-plate-drag",
    ] {
        let (sk, _, fixture) = load(slug);
        let drag = fixture
            .drag
            .as_ref()
            .map(|d| (EntityId(d.dragged.0), d.targets.clone()));
        let run = || {
            frames(
                &sk,
                None,
                drag.as_ref().map(|(p, t)| (&sk, *p, t.as_slice())),
            )
        };
        let shared = run();
        let reference = three_pass::run(run);
        assert!(shared == reference, "{slug}");
    }
}

#[test]
fn shared_ranks_are_the_three_pass_ones_on_every_family_and_size() {
    use laman_debug::corpus::{rect_chain, slot_grid, triangle_strip};
    for (name, case) in [
        ("rect-chain-3", rect_chain(3)),
        ("rect-chain-19", rect_chain(19)),
        ("rect-chain-49", rect_chain(49)),
        ("rect-chain-99", rect_chain(99)),
        ("slot-grid-2x2", slot_grid(2, 2)),
        ("slot-grid-3x3", slot_grid(3, 3)),
        ("slot-grid-4x6", slot_grid(4, 6)),
        ("slot-grid-7x7", slot_grid(7, 7)),
        ("triangle-strip-6", triangle_strip(6)),
        ("triangle-strip-32", triangle_strip(32)),
        ("triangle-strip-82", triangle_strip(82)),
        ("triangle-strip-166", triangle_strip(166)),
    ] {
        assert_shared_is_three_pass(name, &case.unwrap());
    }
}

#[test]
fn shared_ranks_are_the_three_pass_ones_at_the_extremes_of_f64() {
    // The sparse rows (plans/m8-frame-floor step 2) against the dense
    // reference where a row's entries underflow, overflow and square to
    // nothing: sketches at 1e±150 and 1e±300.
    // A sketch at 1e-300 may underflow to a degenerate component the staged
    // path leaves whole; each scale must still stage one of its three.
    for scale in [1e-300, 1e-150, 1e150, 1e300] {
        let rects = laman_debug::corpus::rect_chain_with(3, 20.0 * scale, 10.0 * scale);
        let slots = laman_debug::corpus::slot_grid_with(2, 2, 40.0 * scale, 5.0 * scale);
        let tris = laman_debug::corpus::triangle_strip_with(6, 20.0 * scale);
        let staged = shared_is_three_pass(&format!("rectangles at {scale:e}"), &rects.unwrap())
            + shared_is_three_pass(&format!("slots at {scale:e}"), &slots.unwrap())
            + shared_is_three_pass(&format!("triangles at {scale:e}"), &tris.unwrap());
        assert!(
            staged > 0,
            "nothing staged at {scale:e}: a vacuous comparison"
        );
    }
}

proptest::proptest! {
    // No explicit case count: proptest's default and PROPTEST_CASES both apply.
    #[test]
    fn shared_ranks_are_the_three_pass_ones_on_random_rectangles(
        n in 1..=6_usize,
        width in 10.0..40.0_f64,
        height in 5.0..20.0_f64,
    ) {
        let case = laman_debug::corpus::rect_chain_with(n, width, height).unwrap();
        assert_shared_is_three_pass("property-rectangles", &case);
    }

    #[test]
    fn shared_ranks_are_the_three_pass_ones_on_random_slots(
        rows in 1..=2_usize,
        cols in 1..=2_usize,
        length in 20.0..60.0_f64,
        radius in 3.0..8.0_f64,
    ) {
        let case = laman_debug::corpus::slot_grid_with(rows, cols, length, radius).unwrap();
        assert_shared_is_three_pass("property-slots", &case);
    }

    #[test]
    fn shared_ranks_are_the_three_pass_ones_on_random_triangles(
        n in 1..=8_usize,
        side in 10.0..30.0_f64,
    ) {
        let case = laman_debug::corpus::triangle_strip_with(n, side).unwrap();
        assert_shared_is_three_pass("property-triangles", &case);
    }
}

#[test]
fn a_frame_factorizes_each_staged_component_once_at_its_coordinates() {
    // Not a vacuous comparison: the slot grid is one staged component. A
    // static solve's acceptance makes its factorization and the freedom and
    // the redundancy candidates read it. A drag frame's solve accepts the
    // staged result before pursuing the goal, at other coordinates, so the
    // frame makes two and reads one.
    let case = laman_debug::corpus::slot_grid(3, 3).unwrap();
    let options = crate::solution::SolverOptions::default();
    let _ = three_pass::reads();
    let _ = ours(&case.sketch).solve(&options);
    assert_eq!(three_pass::reads(), [1, 2]);
    let drag = case.drag.unwrap();
    let mut session = ours(&drag.sketch)
        .drag(EntityId(drag.drag.dragged.0), &options)
        .unwrap();
    let _ = three_pass::reads();
    let _ = session.to(drag.drag.targets[0]);
    assert_eq!(three_pass::reads(), [2, 1]);
}

#[test]
fn a_static_solve_tests_each_component_once_and_a_moved_drag_frame_again() {
    // The slot grid is one staged component. A static solve's acceptance
    // tests its satisfaction as it factorizes, and the diagnosis reads that
    // answer. A drag frame whose pursuit moved the coordinates misses the
    // key: its diagnosis tests afresh, and the freedom's factorization
    // there tests once more (plans/m8-dense-rank step 3).
    let case = laman_debug::corpus::slot_grid(3, 3).unwrap();
    let options = crate::solution::SolverOptions::default();
    let _ = three_pass::tested();
    let _ = ours(&case.sketch).solve(&options);
    assert_eq!(three_pass::tested(), 1);
    let drag = case.drag.unwrap();
    let mut session = ours(&drag.sketch)
        .drag(EntityId(drag.drag.dragged.0), &options)
        .unwrap();
    let _ = three_pass::tested();
    let _ = session.to(drag.drag.targets[0]);
    assert_eq!(three_pass::tested(), 3);
}

/// `Component::reads` is what a fresh `component_reads` gives, for the
/// tolerance components and the parts a solve splits them into
/// (plans/m8-dense-rank step 4).
fn assert_reads_are_fresh(name: &str, sk: &Sketch) {
    let system = System::new(sk);
    let tolerances = crate::model::tolerance::Tolerances::new(sk);
    for component in &tolerances.components {
        let fresh = tolerance::component_reads(sk, component);
        assert_eq!(*component.reads, *fresh, "{name}");
        for part in crate::solve::parts_of(&system, component).unwrap_or_default() {
            assert_eq!(
                *part.reads,
                *tolerance::component_reads(sk, &part),
                "{name}"
            );
        }
    }
}

#[test]
fn component_reads_are_the_fresh_ones_on_c1_and_every_family() {
    use laman_debug::corpus::{rect_chain, slot_grid, triangle_strip};
    for slug in [
        "sss-triangle",
        "belt",
        "slotted-plate",
        "sss-triangle-conflicting",
        "slotted-plate-drag",
    ] {
        assert_reads_are_fresh(slug, &load(slug).0);
    }
    let mut staged_parts = 0;
    for (name, case) in [
        ("rect-chain-19", rect_chain(19)),
        ("slot-grid-3x3", slot_grid(3, 3)),
        ("triangle-strip-32", triangle_strip(32)),
    ] {
        let sk = ours(&case.unwrap().sketch);
        assert_reads_are_fresh(name, &sk);
        let system = System::new(&sk);
        for c in &crate::model::tolerance::Tolerances::new(&sk).components {
            staged_parts += crate::solve::parts_of(&system, c).map_or(0, |p| p.len());
        }
    }
    assert!(
        staged_parts > 1,
        "no component split: the parts are untested"
    );
}

/// The dense factorization, kept as the reference the zero-skipping `Qr::new`
/// must equal bit for bit: every norm recomputed, every row and column visited.
fn dense_qr(mut a: Vec<Vec<f64>>, n: usize) -> Qr {
    let m = a.len();
    let mut perm: Vec<usize> = (0..n).collect();
    let mut rank = 0;
    for k in 0..m.min(n) {
        let mut best: Option<(f64, usize)> = None;
        for j in k..n {
            let norm = (k..m).fold(0.0, |s, i| s + a[i][j] * a[i][j]).sqrt();
            let better = match best {
                None => true,
                Some((b, bj)) => norm > b || (norm == b && perm[j] < perm[bj]),
            };
            if better {
                best = Some((norm, j));
            }
        }
        let Some((norm, j)) = best else { break };
        if norm.is_nan() || norm <= RANK {
            break;
        }
        perm.swap(k, j);
        for row in a.iter_mut() {
            row.swap(k, j);
        }
        let alpha = if a[k][k] > 0.0 { -norm } else { norm };
        let mut v: Vec<f64> = (k..m).map(|i| a[i][k]).collect();
        v[0] -= alpha;
        let vv = v.iter().fold(0.0, |s, x| s + x * x);
        #[allow(clippy::needless_range_loop)]
        for col in k + 1..n {
            let dot = (k..m).fold(0.0, |s, i| s + v[i - k] * a[i][col]);
            let f = 2.0 * dot / vv;
            for i in k..m {
                a[i][col] -= f * v[i - k];
            }
        }
        a[k][k] = alpha;
        for row in a.iter_mut().skip(k + 1) {
            row[k] = 0.0;
        }
        rank += 1;
    }
    Qr { a, perm, rank }
}

fn bits(rows: &[Vec<f64>]) -> Vec<Vec<u64>> {
    rows.iter()
        .map(|r| r.iter().map(|x| x.to_bits()).collect())
        .collect()
}

proptest::proptest! {
    #[test]
    fn zero_skipping_qr_equals_the_dense_one_by_bits(
        m in 0usize..9,
        n in 1usize..9,
        cells in proptest::collection::vec(
            proptest::prop_oneof![
                3 => proptest::strategy::Just(0.0),
                1 => -4.0f64..4.0,
                1 => (-3i32..4).prop_map(f64::from),
            ],
            64,
        ),
        dup in 0usize..3,
    ) {
        let mut rows: Vec<Vec<f64>> = (0..m)
            .map(|i| (0..n).map(|j| cells[(i * 8 + j) % 64]).collect())
            .collect();
        // Rank-deficient: repeat or sum earlier rows.
        for _ in 0..dup {
            if rows.len() >= 2 {
                let sum: Vec<f64> = rows[0].iter().zip(&rows[1]).map(|(x, y)| x + y).collect();
                rows.push(sum);
            }
        }
        let rows: Vec<Vec<f64>> = rows.into_iter().filter_map(unit_row).collect();
        let fast = Qr::new(rows.clone(), n);
        let dense = dense_qr(rows, n);
        proptest::prop_assert_eq!(fast.rank, dense.rank);
        proptest::prop_assert_eq!(&fast.perm, &dense.perm);
        proptest::prop_assert_eq!(bits(&fast.a), bits(&dense.a));
        proptest::prop_assert_eq!(bits(&fast.null_basis()), bits(&dense.null_basis()));
    }
}
