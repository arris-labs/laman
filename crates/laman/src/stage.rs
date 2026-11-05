//! The decomposition of a component into parts: an over-determined head,
//! square stages in dependency order, an under-determined remainder
//! (plans/m6-stages).
//!
//! The structure is the bipartite graph of a component's required
//! equations and free parameters, an edge where an equation's gradient has
//! a term. It is read from the term keys, never their values, so it does not
//! change as the coordinates move; a contact's regularized terms are its four
//! points (ADR-0002). A maximum matching of equations to parameters
//! (augmenting paths, in compiled order, the smallest parameter first) gives
//! the coarse Dulmage–Mendelsohn split of Dulmage and Mendelsohn, "Coverings
//! of bipartite graphs", Canadian Journal of Mathematics 10 (1958): the rows
//! an unmatched row can reach by alternating paths are the head, the columns
//! an unmatched column can reach are the remainder, and the rest is square
//! and perfectly matched. Tarjan's strongly connected components of that
//! middle ("Depth-first search and linear graph algorithms", SIAM Journal on
//! Computing 1 (1972)) are the stages, emitted in dependency order.
//!
//! A constraint's equations never split across parts, so equations of one
//! source that land in different parts merge them, and the merged graph is
//! condensed again. A part reads only its own parameters and those of earlier
//! parts.

use crate::model::tolerance::{self, Component, Source};
use crate::model::{Coordinate, Coordinates};
use crate::residual::{self, Param};
use crate::system::{Equation, System};
use std::collections::{BTreeMap, BTreeSet};

/// Stride, period and spacing of the generic position (`support_at`): they
/// place every parameter at a fixed, deterministic, non-symmetric position
/// so that a degenerate equation (coincident ends) gets a nonzero gradient
/// whose term keys give its structure. They are not tolerances: nothing is
/// compared against them. The pattern repeats every `GENERIC_PERIOD`
/// parameters, so two points can coincide there on a large sketch; then
/// `keys_at` still finds the equation degenerate and the code falls back to
/// `tolerance::dependencies`, a superset of what it reads: the cut is
/// coarser, never wrong.
const GENERIC_STRIDE: usize = 37;
const GENERIC_PERIOD: usize = 101;
const GENERIC_SPACING: f64 = 0.37;

/// What a part is, by its place in the decomposition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Kind {
    /// More equations than parameters it fixes: every redundancy and
    /// conflict lives here. Reads nothing outside itself.
    Head,
    /// Square and structurally determined.
    Stage,
    /// Fewer equations than parameters: every freedom lives here.
    Remainder,
}

/// One part of a component.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Part {
    pub kind: Kind,
    /// Indices into [`System::equations`], ascending.
    pub rows: Vec<usize>,
    /// The parameters the part owns, in the component's order.
    pub params: Vec<Param>,
    /// How many of its rows the matching covers.
    pub matched: usize,
}

/// A component's parts in dependency order: head, stages, remainder
/// (a group merged across kinds is a head before a remainder before a stage).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Decomposition {
    pub parts: Vec<Part>,
    /// The structural rank: the size of the maximum matching.
    pub rank: usize,
}

/// The parameters of `component` that equation `eq` reads, in order.
#[cfg(test)]
pub(crate) fn support(system: &System<'_>, eq: &Equation) -> Vec<Param> {
    support_at(system, eq, &system.sketch.coordinates())
}

/// [`support`] with the sketch's coordinates read once by the caller.
pub(crate) fn support_at(system: &System<'_>, eq: &Equation, own: &Coordinates) -> Vec<Param> {
    let sk = system.sketch;
    if let Some(contact) = &eq.contact {
        let mut out = Vec::new();
        for point in [contact.a, contact.b, contact.point, contact.centre] {
            out.push((point.id(), Coordinate::X));
            out.push((point.id(), Coordinate::Y));
        }
        out.sort();
        out.dedup();
        return out;
    }
    let keys_at = |values: &Coordinates| -> Option<Vec<Param>> {
        let equation = match eq.source {
            Source::Arc(id) => residual::intrinsic(sk, values, id),
            Source::Constraint(id) => residual::equations(sk, values, id)
                .and_then(|e| e.as_slice().get(eq.index).copied()),
        }?;
        (!equation.is_degenerate()).then(|| equation.terms().iter().map(|t| t.0).collect())
    };
    // A degenerate equation (coincident ends) has no gradient where it
    // stands: read its structure at a fixed generic position instead, and
    // failing that, all it could read.
    let mut keys = keys_at(own)
        .or_else(|| {
            let mut generic = sk.coordinates();
            for (i, value) in generic.values_mut().enumerate() {
                *value = 1.0 + (i * GENERIC_STRIDE % GENERIC_PERIOD) as f64 * GENERIC_SPACING;
            }
            keys_at(&generic)
        })
        .unwrap_or_else(|| tolerance::dependencies(sk, eq.source));
    keys.sort();
    keys.dedup();
    keys
}

/// The decompositions of a system's components, kept for a system compiled
/// again from the same equations (a drag session's frames): the structure
/// does not depend on coordinates, so it is cut once. An entry is used only
/// for the component whose parameters and sources it was cut from.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Decompositions {
    by_first: BTreeMap<Param, usize>,
    entries: Vec<(Vec<Param>, Vec<Source>, Decomposition)>,
}

impl Decompositions {
    /// Every component of `system` cut into parts.
    pub(crate) fn of(system: &System<'_>) -> Self {
        let mut out = Self::default();
        for component in &system.components {
            if let Some(first) = component.params.first() {
                out.by_first.insert(*first, out.entries.len());
                out.entries.push((
                    component.params.clone(),
                    component.sources.clone(),
                    cut(system, component),
                ));
            }
        }
        out
    }

    fn get(&self, component: &Component) -> Option<&Decomposition> {
        let first = component.params.first()?;
        let (params, sources, decomposition) = self.entries.get(*self.by_first.get(first)?)?;
        (*params == component.params && *sources == component.sources).then_some(decomposition)
    }
}

#[cfg(test)]
thread_local! {
    /// How many components this thread has cut (`cut` calls): a test's count
    /// of the work a frame does.
    pub(crate) static CUTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Cut `component` into parts, from the system's kept decompositions when it
/// has them for this component.
pub(crate) fn decompose(system: &System<'_>, component: &Component) -> Decomposition {
    match system.decompositions.and_then(|d| d.get(component)) {
        Some(kept) => kept.clone(),
        None => system
            .cut
            .get_or_init(|| Decompositions::of(system))
            .get(component)
            .cloned()
            .unwrap_or_else(|| cut(system, component)),
    }
}

fn cut(system: &System<'_>, component: &Component) -> Decomposition {
    #[cfg(test)]
    CUTS.with(|n| n.set(n.get() + 1));
    let rows = system.rows_of(component);
    let own = system.sketch.coordinates();
    let mut cols: Vec<Param> = component.params.clone();
    cols.sort();
    cols.dedup();
    let col_of: BTreeMap<Param, usize> = cols.iter().enumerate().map(|(i, p)| (*p, i)).collect();
    let (nr, nc) = (rows.len(), cols.len());
    let adj: Vec<Vec<usize>> = rows
        .iter()
        .map(|&r| {
            let mut a: Vec<usize> = system
                .equations
                .get(r)
                .map(|e| support_at(system, e, &own))
                .unwrap_or_default()
                .iter()
                .filter_map(|p| col_of.get(p).copied())
                .collect();
            a.sort_unstable();
            a.dedup();
            a
        })
        .collect();
    let mut radj: Vec<Vec<usize>> = vec![vec![]; nc];
    for (r, a) in adj.iter().enumerate() {
        for &c in a {
            radj[c].push(r);
        }
    }

    // Maximum matching.
    let mut match_row: Vec<Option<usize>> = vec![None; nr];
    let mut match_col: Vec<Option<usize>> = vec![None; nc];
    let mut seen = vec![0_usize; nc];
    let mut path = Path::default();
    for r in 0..nr {
        augment(
            r,
            &adj,
            &mut match_row,
            &mut match_col,
            &mut seen,
            r + 1,
            &mut path,
        );
    }
    let rank = match_row.iter().flatten().count();

    // Head: rows reachable from an unmatched row by alternating paths.
    let mut head_row: Vec<bool> = match_row.iter().map(Option::is_none).collect();
    let mut head_col = vec![false; nc];
    let mut queue: Vec<usize> = (0..nr).filter(|&r| head_row[r]).collect();
    while let Some(r) = queue.pop() {
        for &c in &adj[r] {
            if !head_col[c] {
                head_col[c] = true;
                if let Some(r2) = match_col[c] {
                    if !head_row[r2] {
                        head_row[r2] = true;
                        queue.push(r2);
                    }
                }
            }
        }
    }
    // Remainder: columns reachable from an unmatched column.
    let mut rem_col: Vec<bool> = match_col.iter().map(Option::is_none).collect();
    let mut rem_row = vec![false; nr];
    let mut queue: Vec<usize> = (0..nc).filter(|&c| rem_col[c]).collect();
    while let Some(c) = queue.pop() {
        for &r in &radj[c] {
            if !rem_row[r] {
                rem_row[r] = true;
                if let Some(c2) = match_row[r] {
                    if !rem_col[c2] {
                        rem_col[c2] = true;
                        queue.push(c2);
                    }
                }
            }
        }
    }

    // Groups: elements 0..nr are rows, nr the head, nr + 1 the remainder.
    let (head, rem) = (nr, nr + 1);
    let mut uf = UnionFind::new(nr + 2);
    let mut first_of: BTreeMap<Source, usize> = BTreeMap::new();
    for (r, &i) in rows.iter().enumerate() {
        if let Some(e) = system.equations.get(i) {
            let first = *first_of.entry(e.source).or_insert(r);
            uf.union(first, r);
        }
        if head_row[r] {
            uf.union(head, r);
        } else if rem_row[r] {
            uf.union(rem, r);
        }
    }
    let has_head = head_row.iter().any(|&h| h);
    let has_rem = rem_col.iter().any(|&h| h);
    let owner = |c: usize| match_col[c].unwrap_or(rem);
    let mut node_of_root: BTreeMap<usize, usize> = BTreeMap::new();
    fn node_of(uf: &mut UnionFind, map: &mut BTreeMap<usize, usize>, e: usize) -> usize {
        let root = uf.find(e);
        let next = map.len();
        *map.entry(root).or_insert(next)
    }
    let mut elems: Vec<usize> = (0..nr).collect();
    if has_head {
        elems.push(head);
    }
    if has_rem {
        elems.push(rem);
    }
    let mut nodes: Vec<usize> = Vec::new(); // element -> node, by position in `elems`
    for &e in &elems {
        nodes.push(node_of(&mut uf, &mut node_of_root, e));
    }
    let n = node_of_root.len();
    let mut holds_head = vec![false; n];
    let mut holds_rem = vec![false; n];
    let mut first_row = vec![usize::MAX; n];
    for (&e, &node) in elems.iter().zip(&nodes) {
        if e == head {
            holds_head[node] = true;
        } else if e == rem {
            holds_rem[node] = true;
        } else {
            first_row[node] = first_row[node].min(e);
        }
    }
    // u depends on v when a row of u reads a column v owns.
    let mut deps: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); n];
    for r in 0..nr {
        let u = nodes[r];
        for &c in &adj[r] {
            let v = node_of(&mut uf, &mut node_of_root, owner(c));
            if u != v {
                deps[u].insert(v);
            }
        }
    }
    let deps: Vec<Vec<usize>> = deps.into_iter().map(|d| d.into_iter().collect()).collect();
    let comp = tarjan(&deps);
    let k = comp.iter().copied().max().map_or(0, |m| m + 1);
    let mut cfirst = vec![usize::MAX; k];
    let mut chead = vec![false; k];
    let mut crem = vec![false; k];
    for (node, &c) in comp.iter().enumerate() {
        cfirst[c] = cfirst[c].min(first_row[node]);
        chead[c] |= holds_head[node];
        crem[c] |= holds_rem[node];
    }
    // A merged group takes the earliest kind: head, else remainder, else stage.
    let ckind: Vec<Kind> = (0..k)
        .map(|c| match (chead[c], crem[c]) {
            (true, _) => Kind::Head,
            (false, true) => Kind::Remainder,
            (false, false) => Kind::Stage,
        })
        .collect();
    let mut cdeps: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); k];
    for (u, ds) in deps.iter().enumerate() {
        for &v in ds {
            if comp[u] != comp[v] {
                cdeps[comp[u]].insert(comp[v]);
            }
        }
    }
    let mut waiting: Vec<usize> = cdeps.iter().map(BTreeSet::len).collect();
    let mut users: Vec<Vec<usize>> = vec![vec![]; k];
    for (u, ds) in cdeps.iter().enumerate() {
        for &v in ds {
            users[v].push(u);
        }
    }
    let mut ready: BTreeSet<(Kind, usize, usize)> = (0..k)
        .filter(|&c| waiting[c] == 0)
        .map(|c| (ckind[c], cfirst[c], c))
        .collect();
    let mut order = Vec::with_capacity(k);
    while let Some(next) = ready.pop_first() {
        let c = next.2;
        order.push(c);
        for &u in &users[c] {
            waiting[u] -= 1;
            if waiting[u] == 0 {
                ready.insert((ckind[u], cfirst[u], u));
            }
        }
    }
    let position: BTreeMap<usize, usize> = order.iter().enumerate().map(|(p, c)| (*c, p)).collect();
    let mut parts: Vec<Part> = order
        .iter()
        .map(|&c| Part {
            kind: ckind[c],
            rows: vec![],
            params: vec![],
            matched: 0,
        })
        .collect();
    for r in 0..nr {
        if let Some(p) = position
            .get(&comp[nodes[r]])
            .and_then(|&p| parts.get_mut(p))
        {
            p.rows.push(rows[r]);
            p.matched += usize::from(match_row[r].is_some());
        }
    }
    for (c, param) in cols.iter().enumerate() {
        let node = node_of(&mut uf, &mut node_of_root, owner(c));
        if let Some(p) = position.get(&comp[node]).and_then(|&p| parts.get_mut(p)) {
            p.params.push(*param);
        }
    }
    parts.retain(|p| !p.rows.is_empty() || !p.params.is_empty());
    Decomposition { parts, rank }
}

/// The search stack of [`augment`], kept between calls so that a search
/// allocates nothing once the buffers have grown.
#[derive(Default)]
struct Path {
    stack: Vec<(usize, usize)>,
    via: Vec<usize>,
}

/// Extend the matching by an augmenting path from `root`, iteratively (a
/// long chain of equations must not overflow the stack). `stamp` marks the
/// columns visited in this attempt; `path` is scratch, left empty.
fn augment(
    root: usize,
    adj: &[Vec<usize>],
    match_row: &mut [Option<usize>],
    match_col: &mut [Option<usize>],
    seen: &mut [usize],
    stamp: usize,
    path: &mut Path,
) -> bool {
    let Path { stack, via } = path;
    stack.clear();
    via.clear();
    stack.push((root, 0));
    while let Some(&(r, i)) = stack.last() {
        let Some(&c) = adj[r].get(i) else {
            stack.pop();
            via.pop();
            continue;
        };
        if let Some(top) = stack.last_mut() {
            top.1 += 1;
        }
        if seen[c] == stamp {
            continue;
        }
        seen[c] = stamp;
        via.push(c);
        match match_col[c] {
            None => {
                for (&(row, _), &col) in stack.iter().zip(via.iter()) {
                    match_row[row] = Some(col);
                    match_col[col] = Some(row);
                }
                return true;
            }
            Some(next) => stack.push((next, 0)),
        }
    }
    false
}

/// Strongly connected components, numbered so that every node's
/// dependencies (its out-neighbours) get a smaller or equal number.
fn tarjan(adj: &[Vec<usize>]) -> Vec<usize> {
    let n = adj.len();
    let mut index = vec![usize::MAX; n];
    let mut low = vec![0_usize; n];
    let mut on = vec![false; n];
    let mut comp = vec![usize::MAX; n];
    let (mut counter, mut count) = (0, 0);
    let mut path: Vec<usize> = Vec::new();
    for start in 0..n {
        if index[start] != usize::MAX {
            continue;
        }
        let mut call: Vec<(usize, usize)> = vec![(start, 0)];
        index[start] = counter;
        low[start] = counter;
        counter += 1;
        path.push(start);
        on[start] = true;
        while let Some(&(v, i)) = call.last() {
            if let Some(&w) = adj[v].get(i) {
                if let Some(top) = call.last_mut() {
                    top.1 += 1;
                }
                if index[w] == usize::MAX {
                    index[w] = counter;
                    low[w] = counter;
                    counter += 1;
                    path.push(w);
                    on[w] = true;
                    call.push((w, 0));
                } else if on[w] {
                    low[v] = low[v].min(index[w]);
                }
            } else {
                call.pop();
                if low[v] == index[v] {
                    while let Some(w) = path.pop() {
                        on[w] = false;
                        comp[w] = count;
                        if w == v {
                            break;
                        }
                    }
                    count += 1;
                }
                if let Some(&(parent, _)) = call.last() {
                    low[parent] = low[parent].min(low[v]);
                }
            }
        }
    }
    comp
}

struct UnionFind {
    parent: Vec<usize>,
}

impl UnionFind {
    fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
        }
    }

    fn find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    /// Joins by the smaller root, so a group's root is its smallest element.
    fn union(&mut self, a: usize, b: usize) {
        let (a, b) = (self.find(a), self.find(b));
        self.parent[a.max(b)] = a.min(b);
    }
}

#[cfg(test)]
mod tests;
