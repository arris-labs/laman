#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Topology of small sketches built by construction.
use super::*;
use crate::model::Point;

fn pts(sk: &mut Sketch, n: usize) -> Vec<Point> {
    (0..n)
        .map(|i| sk.point(i as f64, (i * i) as f64).unwrap())
        .collect()
}

fn ids(v: &[EntityId]) -> Vec<u32> {
    v.iter().map(|e| e.0).collect()
}

fn walk_of(l: &EdgeLoop) -> Vec<(u32, bool)> {
    l.edges.iter().map(|t| (t.entity.0, t.reversed)).collect()
}

/// A closed polygon over `p`, edge `i` from `p[i]` to `p[i + 1]`.
fn polygon(sk: &mut Sketch, p: &[Point]) -> Vec<EntityId> {
    (0..p.len())
        .map(|i| sk.line(p[i], p[(i + 1) % p.len()]).unwrap().id())
        .collect()
}

#[test]
fn rectangle_is_one_loop_in_edge_order() {
    let mut sk = Sketch::new();
    let p = pts(&mut sk, 4);
    let e = polygon(&mut sk, &p);
    let t = topology(&sk);
    assert_eq!(t.loops.len(), 1);
    assert_eq!(
        walk_of(&t.loops[0]),
        e.iter().map(|x| (x.0, false)).collect::<Vec<_>>()
    );
    assert!(t.loops[0].joins.is_empty());
    assert!(t.open_chains.is_empty() && t.dangling.is_empty() && t.branched.is_empty());
}

#[test]
fn loop_follows_edges_drawn_against_it() {
    let mut sk = Sketch::new();
    let p = pts(&mut sk, 3);
    let e0 = sk.line(p[0], p[1]).unwrap().id();
    let e1 = sk.line(p[2], p[1]).unwrap().id(); // against
    let e2 = sk.line(p[2], p[0]).unwrap().id(); // against after e1
    let t = topology(&sk);
    assert_eq!(
        walk_of(&t.loops[0]),
        vec![(e0.0, false), (e1.0, true), (e2.0, false)]
    );
}

#[test]
fn coincident_chain_joins_and_is_named() {
    let mut sk = Sketch::new();
    let p = pts(&mut sk, 8);
    // Four lines each with own end points, corners joined by coincident.
    let e: Vec<_> = (0..4)
        .map(|i| sk.line(p[2 * i], p[2 * i + 1]).unwrap().id())
        .collect();
    let mut c = Vec::new();
    for i in 0..4 {
        c.push(sk.coincident(p[2 * i + 1], p[(2 * i + 2) % 8]).unwrap());
    }
    let t = topology(&sk);
    assert_eq!(t.loops.len(), 1);
    assert_eq!(
        walk_of(&t.loops[0]),
        e.iter().map(|x| (x.0, false)).collect::<Vec<_>>()
    );
    assert_eq!(t.loops[0].joins, c);
}

#[test]
fn chained_coincidents_through_a_stray_point_join() {
    let mut sk = Sketch::new();
    let p = pts(&mut sk, 5);
    let e0 = sk.line(p[0], p[1]).unwrap().id();
    let e1 = sk.line(p[2], p[0]).unwrap().id();
    // p1 ~ p4 ~ p2: a two-line loop through a stray point.
    let c0 = sk.coincident(p[1], p[4]).unwrap();
    let c1 = sk.coincident(p[4], p[2]).unwrap();
    let t = topology(&sk);
    assert_eq!(walk_of(&t.loops[0]), vec![(e0.0, false), (e1.0, false)]);
    assert_eq!(t.loops[0].joins, vec![c0, c1]);
}

#[test]
fn open_chain_is_reported_whole() {
    let mut sk = Sketch::new();
    let p = pts(&mut sk, 4);
    let e = [
        sk.line(p[0], p[1]).unwrap().id(),
        sk.line(p[1], p[2]).unwrap().id(),
        sk.line(p[2], p[3]).unwrap().id(),
    ];
    let t = topology(&sk);
    assert!(t.loops.is_empty() && t.dangling.is_empty());
    assert_eq!(t.open_chains, vec![e.to_vec()]);
}

#[test]
fn dangling_edge_hangs_from_a_loop() {
    let mut sk = Sketch::new();
    let p = pts(&mut sk, 5);
    polygon(&mut sk, &p[..4]);
    let tail = sk.line(p[0], p[4]).unwrap().id();
    let t = topology(&sk);
    assert_eq!(t.loops.len(), 1);
    assert_eq!(t.loops[0].edges.len(), 4);
    assert_eq!(t.dangling, vec![tail]);
    assert!(t.open_chains.is_empty() && t.branched.is_empty());
}

#[test]
fn dangling_tree_is_pruned_to_the_leaves() {
    let mut sk = Sketch::new();
    let p = pts(&mut sk, 6);
    polygon(&mut sk, &p[..3]);
    let a = sk.line(p[0], p[3]).unwrap().id();
    let b = sk.line(p[3], p[4]).unwrap().id();
    let c = sk.line(p[3], p[5]).unwrap().id();
    let t = topology(&sk);
    assert_eq!(ids(&t.dangling), vec![a.0, b.0, c.0]);
    assert_eq!(t.loops.len(), 1);
}

#[test]
fn rectangles_sharing_an_edge_are_branched() {
    let mut sk = Sketch::new();
    let p = pts(&mut sk, 6);
    // Squares p0 p1 p4 p5 and p1 p2 p3 p4 share the edge p1-p4.
    let e = [
        sk.line(p[0], p[1]).unwrap().id(),
        sk.line(p[1], p[2]).unwrap().id(),
        sk.line(p[2], p[3]).unwrap().id(),
        sk.line(p[3], p[4]).unwrap().id(),
        sk.line(p[4], p[1]).unwrap().id(),
        sk.line(p[4], p[5]).unwrap().id(),
        sk.line(p[5], p[0]).unwrap().id(),
    ];
    let t = topology(&sk);
    assert!(t.loops.is_empty());
    assert_eq!(t.branched.len(), 1);
    assert_eq!(t.branched[0].edges, e.to_vec());
    assert_eq!(ids(&t.branched[0].joins), vec![p[1].id().0, p[4].id().0]);
}

#[test]
fn figure_eight_is_branched() {
    let mut sk = Sketch::new();
    let p = pts(&mut sk, 5);
    // Two triangles at p0.
    let mut e = polygon(&mut sk, &[p[0], p[1], p[2]]);
    e.extend(polygon(&mut sk, &[p[0], p[3], p[4]]));
    let t = topology(&sk);
    assert!(t.loops.is_empty());
    assert_eq!(t.branched[0].edges, e);
    assert_eq!(ids(&t.branched[0].joins), vec![p[0].id().0]);
}

#[test]
fn self_joined_line_is_degenerate() {
    let mut sk = Sketch::new();
    let p = pts(&mut sk, 2);
    let e = sk.line(p[0], p[1]).unwrap().id();
    sk.coincident(p[0], p[1]).unwrap();
    let t = topology(&sk);
    assert_eq!(t.degenerate, vec![e]);
    assert!(t.loops.is_empty() && t.open_chains.is_empty());
}

#[test]
fn construction_edges_and_circles() {
    let mut sk = Sketch::new();
    let p = pts(&mut sk, 5);
    let e = polygon(&mut sk, &p[..4]);
    let diag = sk.line(p[0], p[2]).unwrap().id();
    sk.set_construction(diag, true).unwrap();
    let c = sk.circle(p[4], 1.0).unwrap().id();
    let k = sk.circle(p[4], 2.0).unwrap().id();
    sk.set_construction(k, true).unwrap();
    let t = topology(&sk);
    assert_eq!(t.loops.len(), 1);
    assert_eq!(t.loops[0].edges.len(), e.len());
    assert_eq!(t.circles, vec![c]);
    assert!(t.branched.is_empty() && t.dangling.is_empty());
}

#[test]
fn arcs_join_by_start_and_end() {
    let mut sk = Sketch::new();
    let p = pts(&mut sk, 3);
    // A half-disc: arc about p0 from p1 to p2, closed by a line p2 -> p1.
    let a = sk.arc(p[0], p[1], p[2]).unwrap().id();
    let l = sk.line(p[1], p[2]).unwrap().id();
    let t = topology(&sk);
    assert_eq!(walk_of(&t.loops[0]), vec![(a.0, false), (l.0, true)]);
}

#[test]
fn two_disjoint_loops_are_ordered_by_smallest_edge() {
    let mut sk = Sketch::new();
    let p = pts(&mut sk, 8);
    let first = polygon(&mut sk, &p[..4]);
    let second = polygon(&mut sk, &p[4..]);
    let t = topology(&sk);
    assert_eq!(t.loops.len(), 2);
    assert_eq!(t.loops[0].edges[0].entity, first[0]);
    assert_eq!(t.loops[1].edges[0].entity, second[0]);
}
