#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Branched cores split by construction: loop counts, edge order, areas and
//! bridges from the closed forms of the sketches drawn.
use super::*;
use crate::model::Point;
use crate::profile::topology::topology;

fn run(sk: &Sketch) -> Split {
    let values = sk.coordinates();
    let topo = topology(sk);
    split(sk, &values, &Tolerances::at(sk, &values), &topo.cores).unwrap()
}

fn walk_of(l: &Candidate) -> Vec<(u32, bool)> {
    l.edges.iter().map(|t| (t.entity.0, t.reversed)).collect()
}

fn areas(p: &Piece) -> Vec<f64> {
    p.loops.iter().map(|l| l.area).collect()
}

/// Lines through `pts` in order, closed or not, as ids.
fn chain(sk: &mut Sketch, pts: &[Point], close: bool) -> Vec<EntityId> {
    let n = pts.len();
    let last = if close { n } else { n - 1 };
    (0..last)
        .map(|i| sk.line(pts[i], pts[(i + 1) % n]).unwrap().id())
        .collect()
}

fn points(sk: &mut Sketch, xy: &[(f64, f64)]) -> Vec<Point> {
    xy.iter().map(|&(x, y)| sk.point(x, y).unwrap()).collect()
}

#[test]
fn shared_wall_is_two_cells_and_an_outline() {
    let mut sk = Sketch::new();
    let p = points(
        &mut sk,
        &[
            (0., 0.),
            (60., 0.),
            (120., 0.),
            (120., 60.),
            (60., 60.),
            (0., 60.),
        ],
    );
    let mut e = chain(&mut sk, &p, true);
    e.push(sk.line(p[1], p[4]).unwrap().id());
    let s = run(&sk);
    assert!(s.bridges.is_empty());
    assert_eq!(s.pieces.len(), 1);
    let piece = &s.pieces[0];
    assert_eq!(piece.loops.len(), 3);
    let id = |k: usize| e[k].0;
    // Loops are ordered by smallest edge id, as drawn before reversed.
    assert_eq!(
        walk_of(&piece.loops[0]),
        vec![
            (id(0), false),
            (id(6), false),
            (id(4), false),
            (id(5), false)
        ]
    );
    assert_eq!(
        walk_of(&piece.loops[1]),
        vec![
            (id(0), true),
            (id(5), true),
            (id(4), true),
            (id(3), true),
            (id(2), true),
            (id(1), true)
        ]
    );
    assert_eq!(
        walk_of(&piece.loops[2]),
        vec![
            (id(1), false),
            (id(2), false),
            (id(3), false),
            (id(6), true)
        ]
    );
    assert_eq!(areas(piece), vec![3600.0, -7200.0, 3600.0]);
    assert_eq!(piece.outline, 1);
}

#[test]
fn window_has_a_degree_four_join_and_four_panes() {
    let mut sk = Sketch::new();
    let p = points(
        &mut sk,
        &[
            (0., 0.),
            (1., 0.),
            (2., 0.),
            (2., 1.),
            (2., 2.),
            (1., 2.),
            (0., 2.),
            (0., 1.),
            (1., 1.),
        ],
    );
    chain(&mut sk, &p[..8], true);
    for k in [1, 3, 5, 7] {
        sk.line(p[k], p[8]).unwrap();
    }
    let s = run(&sk);
    assert!(s.bridges.is_empty());
    assert_eq!(s.pieces.len(), 1);
    let piece = &s.pieces[0];
    assert_eq!(piece.loops.len(), 5);
    let mut a = areas(piece);
    a.sort_by(f64::total_cmp);
    assert_eq!(a, vec![-4.0, 1.0, 1.0, 1.0, 1.0]);
    for (k, l) in piece.loops.iter().enumerate() {
        assert_eq!(l.edges.len(), if k == piece.outline { 8 } else { 4 });
    }
}

#[test]
fn figure_eight_is_two_lobes_and_an_outline_through_the_join_twice() {
    let mut sk = Sketch::new();
    let p = points(
        &mut sk,
        &[(0., 0.), (2., -1.), (2., 1.), (-2., 1.), (-2., -1.)],
    );
    let right = chain(&mut sk, &[p[0], p[1], p[2]], true);
    let left = chain(&mut sk, &[p[0], p[3], p[4]], true);
    let s = run(&sk);
    assert!(s.bridges.is_empty());
    assert_eq!(s.pieces.len(), 1);
    let piece = &s.pieces[0];
    assert_eq!(piece.loops.len(), 3);
    let mut a = areas(piece);
    a.sort_by(f64::total_cmp);
    assert_eq!(a, vec![-4.0, 2.0, 2.0]);
    assert_eq!(piece.loops[piece.outline].edges.len(), 6);
    let lobe =
        |ids: &[EntityId]| -> Vec<(u32, bool)> { ids.iter().map(|e| (e.0, false)).collect() };
    assert!(piece.loops.iter().any(|l| walk_of(l) == lobe(&right)));
    assert!(piece.loops.iter().any(|l| walk_of(l) == lobe(&left)));
}

#[test]
fn two_rectangles_joined_by_a_line_are_two_pieces_and_a_bridge() {
    let mut sk = Sketch::new();
    let a = points(&mut sk, &[(0., 0.), (2., 0.), (2., 2.), (0., 2.)]);
    let b = points(&mut sk, &[(5., 0.), (7., 0.), (7., 2.), (5., 2.)]);
    chain(&mut sk, &a, true);
    chain(&mut sk, &b, true);
    let bridge = sk.line(a[1], b[0]).unwrap().id();
    let s = run(&sk);
    assert_eq!(s.bridges, vec![bridge]);
    assert_eq!(s.pieces.len(), 2);
    for piece in &s.pieces {
        assert_eq!(piece.loops.len(), 2);
        assert_eq!(areas(piece), vec![4.0, -4.0]);
        assert!(
            piece
                .loops
                .iter()
                .all(|l| l.edges.iter().all(|t| t.entity != bridge))
        );
        assert_eq!(piece.outline, 1);
    }
}

#[test]
fn two_edges_between_the_same_joins_are_not_bridges() {
    // A digon (two lines between the same two points) tied to a rectangle:
    // only the tie is a bridge.
    let mut sk = Sketch::new();
    let a = points(&mut sk, &[(0., 0.), (2., 0.)]);
    let r = points(&mut sk, &[(5., 0.), (7., 0.), (7., 2.), (5., 2.)]);
    let d1 = sk.line(a[0], a[1]).unwrap().id();
    let d2 = sk.line(a[1], a[0]).unwrap().id();
    chain(&mut sk, &r, true);
    let bridge = sk.line(a[1], r[0]).unwrap().id();
    let s = run(&sk);
    assert_eq!(s.bridges, vec![bridge]);
    assert!(!s.bridges.contains(&d1) && !s.bridges.contains(&d2));
    assert_eq!(s.pieces.len(), 2);
}
