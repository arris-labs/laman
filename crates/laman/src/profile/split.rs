//! A branched core split at its joins (ADR-0017): its bridges, its pieces,
//! the loops of each piece and the outline among them.
//!
//! Bridges (D1) are found from the declared joins alone. The order of the
//! edge ends at each join is read from the coordinates by [`super::join`]
//! (D2, D3); the walk (D4) takes, at each join, the end next clockwise from
//! the edge it arrives by, so every directed edge lies in exactly one loop
//! and each loop keeps its area on its left. The outline of a piece (D5) is
//! its loop of most negative area.

use super::join::{End, order};
use super::topology::{Classes, Core, Edge as CoreEdge, joins_of};
use super::{Candidate, ProfileError, directed, measure};
use crate::model::profile::Traversal;
use crate::model::tolerance::Tolerances;
use crate::model::{Coordinates, EntityId, Sketch};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

/// A connected part of a core without its bridges.
#[derive(Debug)]
pub(crate) struct Piece {
    /// Every loop of the piece, ordered by smallest edge id (an edge's
    /// traversal as drawn before the same edge reversed).
    pub loops: Vec<Candidate>,
    /// The index in `loops` of the outline, the loop of most negative area.
    pub outline: usize,
}

/// What splitting the branched cores gives.
#[derive(Debug, Default)]
pub(crate) struct Split {
    /// Edges whose removal disconnects their core, ascending.
    pub bridges: Vec<EntityId>,
    /// The pieces, ordered by smallest edge id.
    pub pieces: Vec<Piece>,
}

/// Splits each of `cores` at `values` (ADR-0017). Refuses only values that
/// lack, or hold a non-finite, coordinate an edge uses.
pub(crate) fn split(
    sketch: &Sketch,
    values: &Coordinates,
    tolerances: &Tolerances,
    cores: &[Core],
) -> Result<Split, ProfileError> {
    let mut out = Split::default();
    for core in cores {
        let mut classes = Classes::of(sketch);
        let bridged = bridges(&core.edges);
        let keep: Vec<usize> = (0..core.edges.len()).filter(|&i| !bridged[i]).collect();
        out.bridges.extend(
            (0..core.edges.len())
                .filter(|&i| bridged[i])
                .map(|i| core.edges[i].id),
        );
        let rings = rings(sketch, values, tolerances, &core.edges, &keep)?;
        let loops = walk(&core.edges, &keep, &rings);

        // Group the loops into pieces by the component of their first edge.
        let mut parts = Classes::default();
        for &i in &keep {
            parts.union(core.edges[i].classes[0], core.edges[i].classes[1]);
        }
        let mut grouped: BTreeMap<EntityId, Vec<Vec<(usize, usize)>>> = BTreeMap::new();
        for l in loops {
            let Some(&(i, _)) = l.first() else { continue };
            grouped
                .entry(parts.find(core.edges[i].classes[0]))
                .or_default()
                .push(l);
        }
        for list in grouped.into_values() {
            let mut loops = Vec::with_capacity(list.len());
            for l in list {
                let traversals: Vec<Traversal> = l
                    .iter()
                    .map(|&(i, d)| Traversal {
                        entity: core.edges[i].id,
                        reversed: d == 1,
                    })
                    .collect();
                let members: Vec<usize> = l.iter().map(|&(i, _)| i).collect();
                let joins = joins_of(&core.edges, &members, sketch, &mut classes);
                loops.push(measure(sketch, values, traversals, joins)?);
            }
            loops.sort_by_key(first_key);
            let outline = outline_of(&loops);
            out.pieces.push(Piece { loops, outline });
        }
    }
    out.bridges.sort();
    out.pieces
        .sort_by_key(|p| p.loops.iter().map(first_key).min());
    Ok(out)
}

pub(super) fn first_key(l: &Candidate) -> Option<(EntityId, bool)> {
    l.edges.first().map(|t| (t.entity, t.reversed))
}

/// Which of `edges` are bridges: one iterative depth-first search over the
/// join classes with low links, from the smallest class, following each
/// class's edges in id order. Parallel edges are told apart by index, so
/// neither of two joining the same classes is a bridge.
fn bridges(edges: &[CoreEdge]) -> Vec<bool> {
    let mut adjacent: BTreeMap<EntityId, Vec<(usize, EntityId)>> = BTreeMap::new();
    for (i, e) in edges.iter().enumerate() {
        adjacent
            .entry(e.classes[0])
            .or_default()
            .push((i, e.classes[1]));
        adjacent
            .entry(e.classes[1])
            .or_default()
            .push((i, e.classes[0]));
    }
    let mut found = vec![false; edges.len()];
    let mut disc: BTreeMap<EntityId, usize> = BTreeMap::new();
    let mut low: BTreeMap<EntityId, usize> = BTreeMap::new();
    for &root in adjacent.keys() {
        if disc.contains_key(&root) {
            continue;
        }
        let at = disc.len();
        disc.insert(root, at);
        low.insert(root, at);
        // (class, the edge it was entered by, the next adjacency to try)
        let mut stack: Vec<(EntityId, Option<usize>, usize)> = vec![(root, None, 0)];
        while let Some(top) = stack.last_mut() {
            let (class, via) = (top.0, top.1);
            let list = adjacent.get(&class).map_or(&[][..], Vec::as_slice);
            if let Some(&(i, other)) = list.get(top.2) {
                top.2 += 1;
                if Some(i) == via {
                    continue;
                }
                if let Some(&d) = disc.get(&other) {
                    let l = low.get(&class).copied().unwrap_or(d).min(d);
                    low.insert(class, l);
                } else {
                    let at = disc.len();
                    disc.insert(other, at);
                    low.insert(other, at);
                    stack.push((other, Some(i), 0));
                }
            } else {
                stack.pop();
                if let (Some(parent), Some(i)) = (stack.last().map(|p| p.0), via) {
                    let (lc, dp) = (low.get(&class).copied(), disc.get(&parent).copied());
                    if let (Some(lc), Some(dp)) = (lc, dp) {
                        if lc > dp {
                            found[i] = true;
                        }
                        let lp = low.get(&parent).copied().unwrap_or(lc).min(lc);
                        low.insert(parent, lp);
                    }
                }
            }
        }
    }
    found
}

/// The ends at each join of the kept edges in counter-clockwise order, and
/// where each end stands in its ring.
struct Rings {
    ring: BTreeMap<EntityId, Vec<(usize, usize)>>,
    slot: BTreeMap<(usize, usize), usize>,
}

fn rings(
    sketch: &Sketch,
    values: &Coordinates,
    tolerances: &Tolerances,
    edges: &[CoreEdge],
    keep: &[usize],
) -> Result<Rings, ProfileError> {
    let mut at: BTreeMap<EntityId, Vec<(usize, usize)>> = BTreeMap::new();
    for &i in keep {
        for end in 0..2 {
            at.entry(edges[i].classes[end]).or_default().push((i, end));
        }
    }
    let mut out = Rings {
        ring: BTreeMap::new(),
        slot: BTreeMap::new(),
    };
    for (class, list) in at {
        let mut ends = Vec::with_capacity(list.len());
        for &(i, end) in &list {
            let traversal = Traversal {
                entity: edges[i].id,
                reversed: end == 1,
            };
            let Some(&leaving) = directed(sketch, values, &[traversal])?.first() else {
                continue;
            };
            ends.push(End {
                edge: edges[i].id,
                reversed: end == 1,
                leaving,
                tolerance: tolerances.entity(edges[i].id),
            });
        }
        let ordered: Vec<(usize, usize)> = order(&ends).into_iter().map(|k| list[k]).collect();
        for (s, &key) in ordered.iter().enumerate() {
            out.slot.insert(key, s);
        }
        out.ring.insert(class, ordered);
    }
    Ok(out)
}

/// Every loop of the kept edges as `(edge index, leaving end)` pairs, each
/// from its smallest edge id; each directed edge is in exactly one.
fn walk(edges: &[CoreEdge], keep: &[usize], rings: &Rings) -> Vec<Vec<(usize, usize)>> {
    let mut seen: BTreeSet<(usize, usize)> = BTreeSet::new();
    let mut loops = Vec::new();
    for &i in keep {
        for d in 0..2 {
            if seen.contains(&(i, d)) {
                continue;
            }
            let mut l = Vec::new();
            let mut at = (i, d);
            while seen.insert(at) {
                l.push(at);
                // Arrives at the other end, and leaves by the end next
                // clockwise from it: the one before it in the ring.
                let arrive = (at.0, 1 - at.1);
                let ring = rings.ring.get(&edges[at.0].classes[arrive.1]);
                let slot = rings.slot.get(&arrive).copied();
                match (ring, slot) {
                    (Some(ring), Some(s)) if !ring.is_empty() => {
                        at = ring[(s + ring.len() - 1) % ring.len()];
                    }
                    _ => break,
                }
            }
            if let Some(first) = (0..l.len()).min_by_key(|&k| (edges[l[k].0].id, l[k].1)) {
                l.rotate_left(first);
            }
            loops.push(l);
        }
    }
    loops
}

/// The loop of most negative area, ties by the smallest edge id (ADR-0017
/// D5). Areas are compared unsaturated, so they order at any extent.
fn outline_of(loops: &[Candidate]) -> usize {
    let mut best = 0;
    for k in 1..loops.len() {
        if compare(&loops[k], &loops[best]) == Ordering::Less {
            best = k;
        }
    }
    best
}

fn compare(a: &Candidate, b: &Candidate) -> Ordering {
    cmp_exact(a.exact, b.exact)
}

/// The magnitudes of two areas, unsaturated.
pub(super) fn compare_magnitude(a: &Candidate, b: &Candidate) -> Ordering {
    cmp_exact((a.exact.0.abs(), a.exact.1), (b.exact.0.abs(), b.exact.1))
}

/// Two areas given as `(twice, exponent)`, compared in the frame of the
/// larger exponent, so they order where each alone saturates.
fn cmp_exact((ta, ea): (f64, i32), (tb, eb): (f64, i32)) -> Ordering {
    let top = if ta == 0.0 {
        eb
    } else if tb == 0.0 {
        ea
    } else {
        ea.max(eb)
    };
    libm::scalbn(ta, ea - top).total_cmp(&libm::scalbn(tb, eb - top))
}

#[cfg(test)]
mod tests;
