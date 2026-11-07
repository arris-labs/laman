//! Which edges close into loops (ADR-0012 D1 and D2): join classes,
//! pruning, and the order of each loop. No coordinates are read.
//!
//! Joins come from the sketch alone: a shared point id, or a chain of
//! `coincident` constraints (union-find, each class named by its smallest
//! point id). Edges are pruned at degree-1 classes until none is left; what
//! remains of a component is its core, a loop when every class in it has
//! degree 2 and branched otherwise.

use crate::model::profile::{Branched, Traversal};
use crate::model::{Constraint, ConstraintId, Entity, EntityId, Sketch};
use std::collections::{BTreeMap, BTreeSet};

/// A closed loop of edges, before its geometry is measured.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EdgeLoop {
    /// The directed edges in order, from the smallest edge id, the first
    /// traversed as drawn.
    pub edges: Vec<Traversal>,
    /// The `coincident` constraints that join its ends, in id order.
    pub joins: Vec<ConstraintId>,
}

/// An edge end alone in its join: nothing else meets it (degree 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FreeEnd {
    /// The join, by its smallest point id.
    pub class: EntityId,
    /// The point as the edge draws it.
    pub point: EntityId,
    /// The edge.
    pub edge: EntityId,
}

/// The topology of a sketch's non-construction edges.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Topology {
    /// Loops of lines and arcs, ordered by smallest edge id.
    pub loops: Vec<EdgeLoop>,
    /// Non-construction circles, each a loop by itself, in id order.
    pub circles: Vec<EntityId>,
    /// Components pruned away entirely, each its edges in id order,
    /// ordered by smallest edge id.
    pub open_chains: Vec<Vec<EntityId>>,
    /// Pruned edges of components that keep a core, in id order.
    pub dangling: Vec<EntityId>,
    /// Cores with a join of degree above 2, ordered by smallest edge id.
    pub branched: Vec<Branched>,
    /// Edges with both ends in one join, in id order.
    pub degenerate: Vec<EntityId>,
    /// Every end alone in its join, whether or not pruning removed its
    /// edge, in order of join and then edge.
    pub free_ends: Vec<FreeEnd>,
}

/// Union-find over ids that names each class by its smallest id.
#[derive(Default)]
struct Classes {
    parent: BTreeMap<EntityId, EntityId>,
}

impl Classes {
    fn find(&mut self, x: EntityId) -> EntityId {
        let mut root = x;
        while let Some(&p) = self.parent.get(&root) {
            root = p;
        }
        let mut at = x;
        while let Some(&p) = self.parent.get(&at) {
            self.parent.insert(at, root);
            at = p;
        }
        root
    }

    fn union(&mut self, a: EntityId, b: EntityId) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra != rb {
            self.parent.insert(ra.max(rb), ra.min(rb));
        }
    }
}

/// A line or arc as the topology sees it.
struct Edge {
    id: EntityId,
    /// The end points as drawn: a line's `a` and `b`, an arc's `s` and `e`.
    points: [EntityId; 2],
    /// The join classes of those ends.
    classes: [EntityId; 2],
}

/// The topology of `sketch`'s non-construction edges.
pub(crate) fn topology(sketch: &Sketch) -> Topology {
    let mut classes = Classes::default();
    for (_, c) in sketch.constraints() {
        if let Constraint::Coincident { p, q } = c {
            classes.union(p.id(), q.id());
        }
    }

    let mut out = Topology::default();
    let mut edges: Vec<Edge> = Vec::new();
    for (id, e) in sketch.entities() {
        if sketch.is_construction(id) {
            continue;
        }
        let points = match e {
            Entity::Line { a, b } => [a.id(), b.id()],
            Entity::Arc { s, e, .. } => [s.id(), e.id()],
            Entity::Circle { .. } => {
                out.circles.push(id);
                continue;
            }
            Entity::Point { .. } => continue,
        };
        let classes = [classes.find(points[0]), classes.find(points[1])];
        if classes[0] == classes[1] {
            out.degenerate.push(id);
        } else {
            edges.push(Edge {
                id,
                points,
                classes,
            });
        }
    }

    // Edges at each class, as (edge index, end).
    let mut incident: BTreeMap<EntityId, Vec<(usize, usize)>> = BTreeMap::new();
    let mut parts = Classes::default();
    for (i, e) in edges.iter().enumerate() {
        for end in 0..2 {
            incident.entry(e.classes[end]).or_default().push((i, end));
        }
        parts.union(e.classes[0], e.classes[1]);
    }

    for (&class, list) in &incident {
        if let [(i, end)] = list[..] {
            out.free_ends.push(FreeEnd {
                class,
                point: edges[i].points[end],
                edge: edges[i].id,
            });
        }
    }

    // Prune at degree-1 classes until none is left; the pruned set does not
    // depend on the order.
    let mut degree: BTreeMap<EntityId, usize> =
        incident.iter().map(|(&c, v)| (c, v.len())).collect();
    let mut alive = vec![true; edges.len()];
    let mut leaves: Vec<EntityId> = degree
        .iter()
        .filter(|&(_, &d)| d == 1)
        .map(|(&c, _)| c)
        .collect();
    while let Some(c) = leaves.pop() {
        if degree.get(&c) != Some(&1) {
            continue;
        }
        let Some(&(i, end)) = incident
            .get(&c)
            .and_then(|v| v.iter().find(|&&(i, _)| alive[i]))
        else {
            continue;
        };
        alive[i] = false;
        let other = edges[i].classes[1 - end];
        degree.insert(c, 0);
        if let Some(d) = degree.get_mut(&other) {
            *d -= 1;
            if *d == 1 {
                leaves.push(other);
            }
        }
    }

    // Group by component: pruned edges, and the core's edges.
    let mut pruned: BTreeMap<EntityId, Vec<usize>> = BTreeMap::new();
    let mut cores: BTreeMap<EntityId, Vec<usize>> = BTreeMap::new();
    for (i, e) in edges.iter().enumerate() {
        let root = parts.find(e.classes[0]);
        let group = if alive[i] { &mut cores } else { &mut pruned };
        group.entry(root).or_default().push(i);
    }
    let mut chains: Vec<Vec<EntityId>> = Vec::new();
    for (root, list) in &pruned {
        let ids = list.iter().map(|&i| edges[i].id);
        if cores.contains_key(root) {
            out.dangling.extend(ids);
        } else {
            chains.push(ids.collect());
        }
    }
    out.dangling.sort();
    chains.sort();
    out.open_chains = chains;

    for core in cores.values() {
        let hubs: BTreeSet<EntityId> = core
            .iter()
            .flat_map(|&i| edges[i].classes)
            .filter(|c| degree.get(c).is_some_and(|&d| d > 2))
            .collect();
        if hubs.is_empty() {
            out.loops
                .push(walk(&edges, core, &incident, &alive, sketch, &mut classes));
        } else {
            out.branched.push(Branched {
                edges: core.iter().map(|&i| edges[i].id).collect(),
                joins: hubs.into_iter().collect(),
            });
        }
    }
    out.loops.sort_by_key(|l| l.edges.first().map(|t| t.entity));
    out.branched.sort_by_key(|b| b.edges.first().copied());
    out.circles.sort();
    out.degenerate.sort();
    out
}

/// Orders one core whose every class has degree 2, from its smallest edge.
fn walk(
    edges: &[Edge],
    core: &[usize],
    incident: &BTreeMap<EntityId, Vec<(usize, usize)>>,
    alive: &[bool],
    sketch: &Sketch,
    classes: &mut Classes,
) -> EdgeLoop {
    // `core` is in edge order, so its first is the smallest id.
    let start = core.first().copied().unwrap_or(0);
    let mut order = Vec::with_capacity(core.len());
    let (mut at, mut end) = (start, 0usize);
    loop {
        order.push(Traversal {
            entity: edges[at].id,
            reversed: end == 1,
        });
        let exit = edges[at].classes[1 - end];
        let next = incident
            .get(&exit)
            .and_then(|v| v.iter().find(|&&(i, _)| i != at && alive[i]));
        match next {
            Some(&(i, e)) if i != start => (at, end) = (i, e),
            _ => break,
        }
    }

    // The coincident constraints on a path between two distinct end points
    // of this loop. A class holds the loop's end points; links to a point no
    // edge of the loop uses are pruned from the leaves in, so a constraint
    // that only ties a corner to such a point is not a join, while a chain
    // through one is.
    let mut ends: BTreeMap<EntityId, BTreeSet<EntityId>> = BTreeMap::new();
    for &i in core {
        for end in 0..2 {
            ends.entry(edges[i].classes[end])
                .or_default()
                .insert(edges[i].points[end]);
        }
    }
    let terminals: BTreeSet<EntityId> = ends.values().flatten().copied().collect();
    let mut links: Vec<(ConstraintId, EntityId, EntityId)> = Vec::new();
    for (id, c) in sketch.constraints() {
        if let Constraint::Coincident { p, q } = c
            && p.id() != q.id()
            && ends
                .get(&classes.find(p.id()))
                .is_some_and(|points| points.len() > 1)
        {
            links.push((id, p.id(), q.id()));
        }
    }
    loop {
        let mut degree: BTreeMap<EntityId, usize> = BTreeMap::new();
        for &(_, p, q) in &links {
            *degree.entry(p).or_default() += 1;
            *degree.entry(q).or_default() += 1;
        }
        let leaf = |x: EntityId| !terminals.contains(&x) && degree.get(&x) == Some(&1);
        let before = links.len();
        links.retain(|&(_, p, q)| !leaf(p) && !leaf(q));
        if links.len() == before {
            break;
        }
    }
    let joins = links.into_iter().map(|(id, _, _)| id).collect();
    EdgeLoop {
        edges: order,
        joins,
    }
}

#[cfg(test)]
mod tests;
