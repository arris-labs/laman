//! Rank, geometric DoF, per-entity DoF and free directions at given coordinates.
//!
//! The Jacobian is the compiled [`System`]'s: required equations after
//! structural contact regularization (ADR-0002), intrinsic equations included,
//! scaled per component and row-normalized as ADR-0003 fixes. Rank is a
//! column-pivoted Householder QR (Businger & Golub, *Linear least squares
//! solutions by Householder transformations*, Numer. Math. 7, 1965) whose
//! column norms are recomputed at every step and whose ties go to the lowest
//! original parameter, so no library tie rule reaches a result. Every sum runs
//! in index order. Status, redundancy, conflicts and degeneracy build on it
//! in [`analysis`] (ADR-0005).
//!
//! **Staged**, the default [`Path`]: a component is cut into parts
//! ([`crate::stage`]) and each part factorized alone, on its own columns,
//! its rows unit-normalized over the whole row as the whole path's are, then
//! cut to those columns. The parts are in dependency
//! order, so the Jacobian is block lower triangular; when every part before
//! the last has full column rank, a null vector is zero on their columns,
//! the rank is the sum of the part ranks and the null space is the last
//! part's. A component that is not satisfied (failed or degenerate), a part
//! before the last that is column-deficient, or a Jacobian entry that
//! breaks the block structure at these coordinates, sends the component to
//! the whole factorization instead (plans/m6-stages). The rows are read
//! sparsely, a row's terms in the component's columns, and each part's
//! block is made from them without the component's dense Jacobian; the
//! blocks are the dense ones bit for bit (plans/m8-frame-floor).
//!
//! A frame's solve and analysis read one component's staged factorization
//! up to three times at the same coordinates: the solve's acceptance of a
//! staged result, the freedom, the redundancy candidates. [`StagedRanks`]
//! makes it once for all of them (plans/m8-frame-budget).
use crate::model::tolerance::{self, Component, RANK, Source};
use crate::model::{Coordinate, Coordinates, EntityId, Sketch};
use crate::solve::Path;
use crate::stage::Decomposition;
use crate::system::System;
use crate::values::Values;
use std::cell::RefCell;
use std::collections::BTreeMap;

type Param = (EntityId, Coordinate);

mod analysis;
pub use analysis::*;

/// The freedom left at some coordinates.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Freedom {
    /// Free parameters minus the rank of the regularized Jacobian.
    pub dof: usize,
    /// Every entity's DoF: the rank of the null basis restricted to the
    /// free parameters it reads (a line's are its two points').
    pub entities: BTreeMap<EntityId, usize>,
    /// An orthonormal null basis, component by component, each vector
    /// listed by the parameters it moves in compiled order.
    pub directions: Vec<Vec<(EntityId, Coordinate, f64)>>,
}

/// [`Freedom`] of `sketch` at `values` (missing keys read from the sketch),
/// staged.
#[cfg(test)]
pub(crate) fn freedom(sketch: &Sketch, values: &Coordinates) -> Freedom {
    freedom_of(&System::new(sketch), values)
}

/// The `dof` of [`freedom_on`] alone: the same ranks from the same
/// factorizations, without the null basis or the per-entity counts, which
/// is all redundancy needs and most of what a freedom costs.
pub(crate) fn dof_only(sketch: &Sketch, values: &Coordinates, path: Path) -> usize {
    let system = System::new(sketch);
    let coordinates = merged(sketch, values);
    system
        .components
        .iter()
        .map(|component| {
            let rank = match path {
                Path::Staged => staged(&system, component, &coordinates, false),
                Path::Whole => None,
            }
            .map_or_else(
                || rank_of(&system, component, &coordinates),
                |(rank, _)| rank,
            );
            component.params.len() - rank
        })
        .sum()
}

/// The constraints whose removal can lower the DoF at `values`, by `path`:
/// every constraint on the whole path, and on the staged one those of a
/// component that is not staged (failed, degenerate, one part, or
/// column-deficient before its last part, or with a dependent row outside the
/// head) together with those holding a head
/// equation of a staged one. The head is where every redundancy lives and a
/// square stage or the remainder never has one (ADR-0009), so the other
/// constraints are not tested.
pub(crate) fn head_constraints(
    system: &System<'_>,
    values: &Coordinates,
    path: Path,
    ranks: &StagedRanks,
) -> std::collections::BTreeSet<crate::model::ConstraintId> {
    let coordinates = merged(system.sketch, values);
    let mut out = std::collections::BTreeSet::new();
    for component in &system.components {
        let heads: Option<std::collections::BTreeSet<usize>> = match path {
            Path::Staged => ranks
                .read(system, component, &coordinates, false, true)
                .map(|_| {
                    crate::stage::decompose(system, component)
                        .parts
                        .into_iter()
                        .filter(|part| part.kind == crate::stage::Kind::Head)
                        .flat_map(|part| part.rows)
                        .collect()
                }),
            Path::Whole => None,
        };
        for (row, eq) in system.equations.iter().enumerate() {
            let Source::Constraint(id) = eq.source else {
                continue;
            };
            if !component.sources.contains(&eq.source) {
                continue;
            }
            if heads.as_ref().is_none_or(|h| h.contains(&row)) {
                out.insert(id);
            }
        }
    }
    out
}

/// The sketch's coordinates with `values` laid over them.
fn merged(sketch: &Sketch, values: &Coordinates) -> Coordinates {
    let mut coordinates = sketch.coordinates();
    for (key, value) in coordinates.iter_mut() {
        if let Some(v) = values.get(key) {
            *value = *v;
        }
    }
    coordinates
}

/// [`Freedom`] of an already compiled system, staged.
#[cfg(test)]
pub(crate) fn freedom_of(system: &System<'_>, values: &Coordinates) -> Freedom {
    freedom_on(system, values, Path::Staged, &StagedRanks::default())
}

/// [`Freedom`] of an already compiled system, by `path`, its staged
/// factorizations read from `ranks`.
pub(crate) fn freedom_on(
    system: &System<'_>,
    values: &Coordinates,
    path: Path,
    ranks: &StagedRanks,
) -> Freedom {
    let coordinates = merged(system.sketch, values);
    let mut dof = 0;
    let mut basis: Vec<Vec<(Param, f64)>> = Vec::new();
    for component in &system.components {
        let (rank, vectors) = match path {
            Path::Staged => ranks.read(system, component, &coordinates, true, false),
            Path::Whole => None,
        }
        .unwrap_or_else(|| null_space(system, component, &coordinates));
        dof += component.params.len() - rank;
        basis.extend(vectors.into_iter().map(|v| {
            component
                .params
                .iter()
                .copied()
                .zip(v)
                .filter(|(_, x)| *x != 0.0)
                .collect::<Vec<_>>()
        }));
    }
    let entities = system
        .sketch
        .entities()
        .map(|(id, _)| {
            let mut params = tolerance::entity_params(system.sketch, id);
            params.sort();
            params.dedup();
            let rows: Vec<Vec<f64>> = params
                .iter()
                .map(|p| {
                    basis
                        .iter()
                        .map(|v| v.iter().find(|(q, _)| q == p).map_or(0.0, |(_, x)| *x))
                        .collect()
                })
                .collect();
            (id, Qr::new(rows, basis.len()).rank)
        })
        .collect();
    let directions = basis
        .into_iter()
        .map(|v| v.into_iter().map(|((id, c), x)| (id, c, x)).collect())
        .collect();
    Freedom {
        dof,
        entities,
        directions,
    }
}

/// A component's rank and its null basis over `component.params`: the
/// required equations' only, which is also the space a goal moves in.
pub(crate) fn null_space(
    system: &System<'_>,
    component: &Component,
    values: &(impl Values + ?Sized),
) -> (usize, Vec<Vec<f64>>) {
    if component.params.is_empty() {
        return (0, vec![]);
    }
    let qr = component_qr(system, component, values);
    (qr.rank, qr.null_basis())
}

/// A component's rank alone: [`null_space`]'s, without the basis.
fn rank_of(system: &System<'_>, component: &Component, values: &(impl Values + ?Sized)) -> usize {
    if component.params.is_empty() {
        return 0;
    }
    component_qr(system, component, values).rank
}

/// A component's rank and, when `basis` is asked for, its null basis over
/// `component.params`, part by part (the module's *Staged*). `None`, for
/// the whole path, when the component is not satisfied at `values` (failed
/// or degenerate) or the part ranks do not add up to the component's: a
/// part before the last is column-deficient, or a row reads a later part's
/// column at `values`.
/// A component of one part is factorized whole, the same matrix in the same
/// order, so it keeps the whole path's bits.
fn staged(
    system: &System<'_>,
    component: &Component,
    values: &(impl Values + ?Sized),
    basis: bool,
) -> Option<(usize, Vec<Vec<f64>>)> {
    #[cfg(test)]
    if three_pass::active() {
        return staged_checked(system, component, values, basis, false);
    }
    let parts = crate::stage::decompose(system, component);
    factorize(system, component, values, &parts)
        .1?
        .read(basis, false)
}

/// Whether `component` is analysed by parts at `values`: satisfied there,
/// of more than one part, every part before the last of full column rank and
/// no row reading a later part's column. The staged solve keeps its result
/// only then, so a solve and its diagnosis take the same path. The
/// factorization is kept in `ranks` for the diagnosis to read.
pub(crate) fn stageable(
    system: &System<'_>,
    component: &Component,
    values: &(impl Values + ?Sized),
    ranks: &StagedRanks,
) -> bool {
    // `values` holds every coordinate of the sketch (a solve's), so no
    // overlay on the sketch's own is needed (plans/m8-dense-rank step 7).
    ranks
        .read(system, component, values, false, false)
        .is_some()
}

/// The staged factorizations of one frame (plans/m8-frame-budget): the
/// solve's acceptance of a staged result ([`stageable`]), the freedom
/// ([`freedom_on`]) and the redundancy candidates ([`head_constraints`])
/// each read a component's, and where they read it at the same coordinates
/// it is made once. An entry is read again only for the same component
/// (parameters, sources, scale and parts) at the same bits of every
/// coordinate it can read, so a read returns what a factorization made
/// there returns. The systems that share one compile the same sketch with
/// the same constraint values (what an equation's rows depend on beyond the
/// coordinates): one frame's solve and analysis. That is a precondition of
/// the cache and of [`analyse_under`], not checked by them.
#[derive(Debug, Default)]
pub(crate) struct StagedRanks {
    entries: RefCell<Vec<Entry>>,
}

/// One factorization and everything it was made from.
#[derive(Debug)]
struct Entry {
    params: Vec<Param>,
    sources: Vec<Source>,
    scale: u64,
    parts: Decomposition,
    /// Every coordinate the component can read ([`tolerance::component_reads`])
    /// and its bits.
    reads: Vec<(Param, Option<u64>)>,
    /// Whether the component is satisfied there ([`crate::solve::satisfied`]),
    /// which [`factorize`] tests first (plans/m8-dense-rank step 3).
    satisfied: bool,
    staged: Option<Staged>,
}

impl StagedRanks {
    /// [`staged`] with `independent` as [`staged_checked`] reads it, from the
    /// entry made at these coordinates or a new one. `values` holds every
    /// coordinate of the sketch ([`merged`]).
    fn read(
        &self,
        system: &System<'_>,
        component: &Component,
        values: &(impl Values + ?Sized),
        basis: bool,
        independent: bool,
    ) -> Option<(usize, Vec<Vec<f64>>)> {
        #[cfg(test)]
        if three_pass::active() {
            return staged_checked(system, component, values, basis, independent);
        }
        if component.params.is_empty() || component.sources.is_empty() {
            return None;
        }
        let parts = crate::stage::decompose(system, component);
        let (reads, scale) = key(component, values);
        let mut entries = self.entries.borrow_mut();
        let found = entries
            .iter()
            .position(|e| e.is_for(component, scale, &reads, &parts));
        #[cfg(test)]
        three_pass::count(found.is_some());
        let index = match found {
            Some(index) => index,
            None => {
                let (satisfied, staged) = factorize(system, component, values, &parts);
                #[cfg(test)]
                three_pass::count_staged(staged.is_some());
                entries.push(Entry {
                    params: component.params.clone(),
                    sources: component.sources.clone(),
                    scale,
                    parts,
                    reads,
                    satisfied,
                    staged,
                });
                entries.len() - 1
            }
        };
        entries
            .get(index)?
            .staged
            .as_ref()?
            .read(basis, independent)
    }
}

impl StagedRanks {
    /// [`crate::solve::satisfied`] for `component` at `values`, from the
    /// entry a [`stageable`] test made at these coordinates, which tested it
    /// already, or computed afresh (and not kept) when there is none: an
    /// empty component, or coordinates the factorizations were not made at
    /// (plans/m8-dense-rank step 3).
    pub(crate) fn satisfied(
        &self,
        system: &System<'_>,
        component: &Component,
        values: &(impl Values + ?Sized),
    ) -> bool {
        // The reference tests afresh, as it factorizes.
        #[cfg(test)]
        let kept = !three_pass::active();
        #[cfg(not(test))]
        let kept = true;
        if kept {
            if let Some(known) = self.known(system, component, values) {
                return known;
            }
        }
        #[cfg(test)]
        three_pass::count_tested();
        crate::solve::satisfied(system, component, values)
    }

    /// The kept answer at exactly these coordinates, if any.
    fn known(
        &self,
        system: &System<'_>,
        component: &Component,
        values: &(impl Values + ?Sized),
    ) -> Option<bool> {
        if component.params.is_empty() || component.sources.is_empty() {
            return None;
        }
        let parts = crate::stage::decompose(system, component);
        let (reads, scale) = key(component, values);
        self.entries
            .borrow()
            .iter()
            .find(|e| e.is_for(component, scale, &reads, &parts))
            .map(|e| e.satisfied)
    }
}

/// What an [`Entry`] is keyed by besides the component's own fields: the
/// bits of every coordinate it can read in `values`, and its scale's bits.
fn key(component: &Component, values: &(impl Values + ?Sized)) -> (Vec<(Param, Option<u64>)>, u64) {
    let reads = component
        .reads
        .iter()
        .map(|p| (*p, values.get(*p).map(f64::to_bits)))
        .collect();
    (reads, component.scale.to_bits())
}

impl Entry {
    /// Whether this entry was made for `component` with this key.
    fn is_for(
        &self,
        component: &Component,
        scale: u64,
        reads: &[(Param, Option<u64>)],
        parts: &Decomposition,
    ) -> bool {
        self.params == component.params
            && self.sources == component.sources
            && self.scale == scale
            && self.reads == reads
            && self.parts == *parts
    }
}

/// A component's staged factorization at some coordinates, whoever reads it.
#[derive(Debug)]
struct Staged {
    /// The sum of the part ranks.
    rank: usize,
    /// Every part outside the head has a rank equal to its row count.
    independent: bool,
    /// The last part's factorization, whose null space is the component's.
    last: Qr,
    /// The component columns the last part owns, ascending.
    columns: Vec<usize>,
    /// The component's column count.
    n: usize,
}

impl Staged {
    /// The rank and, when `basis` is asked for, the null basis over the
    /// component's columns; `None` when `independent` is asked for and a
    /// part outside the head has a dependent row.
    fn read(&self, basis: bool, independent: bool) -> Option<(usize, Vec<Vec<f64>>)> {
        if independent && !self.independent {
            return None;
        }
        let vectors = if basis {
            self.last
                .null_basis()
                .into_iter()
                .map(|v| {
                    let mut full = vec![0.0; self.n];
                    for (j, x) in self.columns.iter().zip(v) {
                        if let Some(slot) = full.get_mut(*j) {
                            *slot = x;
                        }
                    }
                    full
                })
                .collect()
        } else {
            vec![]
        };
        Some((self.rank, vectors))
    }
}

/// The factorization [`staged`] reads, part by part over `parts`; `None`
/// where it falls back to the whole path. Whether a part outside the head
/// has a row dependent on the others at `values` (a rank below its row
/// count: a redundancy the structure does not show, as a point on a line
/// through a point it coincides with) is kept for [`head_constraints`].
fn factorize(
    system: &System<'_>,
    component: &Component,
    values: &(impl Values + ?Sized),
    parts: &Decomposition,
) -> (bool, Option<Staged>) {
    if component.params.is_empty() || component.sources.is_empty() {
        return (false, None);
    }
    // A failed or degenerate component is the whole path's, as its solve
    // and its conflict search are: only a satisfied one is staged. The rows
    // are read once, for the test and for the blocks (plans/m8-dense-rank
    // step 2), and the answer is kept for the diagnosis (step 3).
    let (rows, flagged, valid) = system.rows(component, component, values, 1.0);
    #[cfg(test)]
    three_pass::count_tested();
    if !crate::solve::satisfied_over(system, component, values, &rows, flagged && valid) {
        return (false, None);
    }
    (true, staged_parts(component, parts, rows))
}

/// [`factorize`] of a satisfied component, over its `rows`.
fn staged_parts(
    component: &Component,
    parts: &Decomposition,
    rows: Vec<crate::system::Row>,
) -> Option<Staged> {
    if parts.parts.len() < 2 {
        return None;
    }
    // Each component column's part, and each row's.
    let n = component.params.len();
    let mut part_of: BTreeMap<Param, usize> = BTreeMap::new();
    for (p, part) in parts.parts.iter().enumerate() {
        for param in &part.params {
            part_of.insert(*param, p);
        }
    }
    let column_part: Vec<usize> = component
        .params
        .iter()
        .map(|param| part_of.get(param).copied())
        .collect::<Option<_>>()?;
    let column_of: BTreeMap<Param, usize> = component
        .params
        .iter()
        .enumerate()
        .map(|(j, param)| (*param, j))
        .collect();
    // Each part's columns, ascending, and each column's place among them.
    let mut part_columns: Vec<Vec<usize>> = vec![vec![]; parts.parts.len()];
    let mut place = vec![0; n];
    for (j, &q) in column_part.iter().enumerate() {
        if let (Some(columns), Some(at)) = (part_columns.get_mut(q), place.get_mut(j)) {
            *at = columns.len();
            columns.push(j);
        }
    }
    let mut row_part: BTreeMap<usize, usize> = BTreeMap::new();
    for (p, part) in parts.parts.iter().enumerate() {
        for &row in &part.rows {
            row_part.insert(row, p);
        }
    }
    // The rows read sparsely: a row's nonzero entries are its terms in the
    // component's columns times its factor, the dense row's (plans/m8-frame-floor
    // step 2). A zero entry adds +0 to the running maximum and to the sum of
    // squares and is visited last in neither, so the unit row and the
    // triangular test over the nonzero entries in column order equal the
    // dense ones bit for bit.
    let mut blocks: Vec<Vec<Vec<f64>>> = vec![vec![]; parts.parts.len()];
    for row in rows {
        let p = *row_part.get(&row.index)?;
        let mut entries: Vec<(usize, f64)> = row
            .terms
            .iter()
            .filter_map(|(param, term)| Some((*column_of.get(param)?, term * row.factor)))
            .collect();
        entries.sort_unstable_by_key(|(j, _)| *j);
        // Block lower triangular: a row reads nothing of a later part.
        if entries
            .iter()
            .any(|(j, x)| column_part.get(*j).is_some_and(|q| *q > p) && *x != 0.0)
        {
            return None;
        }
        // Unit over the whole row, then cut to the part's columns: a row
        // whose own entries are rounding of a dependency on earlier rows
        // stays below RANK there, instead of being blown up to a unit row
        // that the whole factorization would never count.
        let big = entries.iter().fold(0.0, |m: f64, (_, x)| m.max(x.abs()));
        if big == 0.0 || !big.is_finite() {
            continue;
        }
        let norm = entries
            .iter()
            .fold(0.0, |s, (_, x)| {
                let scaled = x / big;
                s + scaled * scaled
            })
            .sqrt();
        let width = part_columns.get(p).map_or(0, Vec::len);
        let mut block_row = vec![0.0; width];
        for (j, x) in &entries {
            if column_part.get(*j) == Some(&p) {
                if let Some(slot) = place.get(*j).and_then(|at| block_row.get_mut(*at)) {
                    *slot = (x / big) / norm;
                }
            }
        }
        if let Some(block) = blocks.get_mut(p) {
            block.push(block_row);
        }
    }
    let last = parts.parts.len() - 1;
    let mut rank = 0;
    let mut independent = true;
    let mut tail = None;
    for (p, ((rows, part), columns)) in blocks
        .into_iter()
        .zip(&parts.parts)
        .zip(part_columns)
        .enumerate()
    {
        let qr = Qr::new(rows, columns.len());
        if p < last && qr.rank < columns.len() {
            return None;
        }
        independent &= !(part.kind != crate::stage::Kind::Head && qr.rank < part.rows.len());
        rank += qr.rank;
        if p == last {
            tail = Some((qr, columns));
        }
    }
    let (last, columns) = tail?;
    Some(Staged {
        rank,
        independent,
        last,
        columns,
        n,
    })
}

/// The pre-M8 reads, each factorizing afresh by [`staged_checked`]: the
/// reference [`StagedRanks`] is held bit-equal to (plans/m8-frame-budget).
#[cfg(test)]
pub(crate) mod three_pass {
    use std::cell::Cell;

    thread_local! {
        static ACTIVE: Cell<bool> = const { Cell::new(false) };
        static READS: Cell<[usize; 2]> = const { Cell::new([0; 2]) };
        static STAGED: Cell<usize> = const { Cell::new(0) };
    }

    thread_local! {
        static TESTED: Cell<usize> = const { Cell::new(0) };
    }

    /// Counts one test of a component's satisfaction on the diagnosis's path.
    pub(crate) fn count_tested() {
        TESTED.with(|t| t.set(t.get() + 1));
    }

    /// The satisfaction tests on this thread since the last call, which
    /// resets the count.
    pub(crate) fn tested() -> usize {
        TESTED.with(|t| t.replace(0))
    }

    /// Counts a new factorization that came out staged (`made`).
    pub(crate) fn count_staged(made: bool) {
        STAGED.with(|s| s.set(s.get() + usize::from(made)));
    }

    /// The staged factorizations made on this thread since the last call,
    /// which resets the count: a test's proof that a comparison was not
    /// between two whole-component paths.
    pub(crate) fn staged_made() -> usize {
        STAGED.with(|s| s.replace(0))
    }

    /// Counts one read of a kept factorization (`hit`) or a new one.
    pub(crate) fn count(hit: bool) {
        READS.with(|r| {
            let mut n = r.get();
            n[usize::from(hit)] += 1;
            r.set(n);
        });
    }

    /// The factorizations made and the reads of a kept one on this thread
    /// since the last call, which resets them.
    pub(crate) fn reads() -> [usize; 2] {
        READS.with(|r| r.replace([0; 2]))
    }

    /// Whether this thread runs the reference.
    pub(crate) fn active() -> bool {
        ACTIVE.with(Cell::get)
    }

    /// `f` run on the reference.
    pub(crate) fn run<T>(f: impl FnOnce() -> T) -> T {
        ACTIVE.with(|a| a.set(true));
        let out = f();
        ACTIVE.with(|a| a.set(false));
        out
    }
}

/// [`staged`], and with `independent` also `None` when a part outside the
/// head has a row dependent on the others at `values` (a rank below its
/// row count): a redundancy the structure does not show, as a point on a
/// line through a point it coincides with. The pre-M8 code, kept as the
/// reference ([`three_pass`]).
#[cfg(test)]
fn staged_checked(
    system: &System<'_>,
    component: &Component,
    values: &(impl Values + ?Sized),
    basis: bool,
    independent: bool,
) -> Option<(usize, Vec<Vec<f64>>)> {
    if component.params.is_empty() || component.sources.is_empty() {
        return None;
    }
    // A failed or degenerate component is the whole path's, as its solve
    // and its conflict search are: only a satisfied one is staged.
    if !crate::solve::satisfied(system, component, values) {
        return None;
    }
    let decomposition = crate::stage::decompose(system, component);
    if decomposition.parts.len() < 2 {
        return None;
    }
    // Each component column's part, and each row's.
    let n = component.params.len();
    let mut part_of: BTreeMap<Param, usize> = BTreeMap::new();
    for (p, part) in decomposition.parts.iter().enumerate() {
        for param in &part.params {
            part_of.insert(*param, p);
        }
    }
    let column_part: Vec<usize> = component
        .params
        .iter()
        .map(|param| part_of.get(param).copied())
        .collect::<Option<_>>()?;
    let mut row_part: BTreeMap<usize, usize> = BTreeMap::new();
    for (p, part) in decomposition.parts.iter().enumerate() {
        for &row in &part.rows {
            row_part.insert(row, p);
        }
    }
    let (evaluation, indices) = system.evaluate_indexed(component, values, 1.0);
    let mut blocks: Vec<Vec<Vec<f64>>> = vec![vec![]; decomposition.parts.len()];
    for (row, index) in evaluation.jacobian.into_iter().zip(indices) {
        let p = *row_part.get(&index)?;
        // Block lower triangular: a row reads nothing of a later part.
        if row
            .iter()
            .zip(&column_part)
            .any(|(x, q)| *q > p && *x != 0.0)
        {
            return None;
        }
        // Unit over the whole row, then cut to the part's columns: a row
        // whose own entries are rounding of a dependency on earlier rows
        // stays below RANK there, instead of being blown up to a unit row
        // that the whole factorization would never count.
        if let Some(unit) = unit_row(row) {
            blocks[p].push(
                unit.into_iter()
                    .zip(&column_part)
                    .filter(|(_, q)| **q == p)
                    .map(|(x, _)| x)
                    .collect(),
            );
        }
    }
    let last = decomposition.parts.len() - 1;
    let mut rank = 0;
    let mut vectors = Vec::new();
    for (p, rows) in blocks.into_iter().enumerate() {
        let columns: Vec<usize> = (0..n).filter(|&j| column_part[j] == p).collect();
        let qr = Qr::new(rows, columns.len());
        if p < last && qr.rank < columns.len() {
            return None;
        }
        if independent
            && decomposition.parts[p].kind != crate::stage::Kind::Head
            && qr.rank < decomposition.parts[p].rows.len()
        {
            return None;
        }
        rank += qr.rank;
        if p == last && basis {
            vectors = qr
                .null_basis()
                .into_iter()
                .map(|v| {
                    let mut full = vec![0.0; n];
                    for (j, x) in columns.iter().zip(v) {
                        full[*j] = x;
                    }
                    full
                })
                .collect();
        }
    }
    Some((rank, vectors))
}

/// The factorization of a component's unit-row Jacobian at `values`.
fn component_qr(system: &System<'_>, component: &Component, values: &(impl Values + ?Sized)) -> Qr {
    // A non-finite row (an unreadable configuration) contributes no rank:
    // the DoF is then an upper estimate, and degeneracy is reported separately.
    let rows = if component.sources.is_empty() {
        vec![]
    } else {
        system
            .evaluate(component, values)
            .jacobian
            .into_iter()
            .filter_map(unit_row)
            .collect()
    };
    Qr::new(rows, component.params.len())
}

/// The row scaled to unit Euclidean norm; `None` for a zero or non-finite row.
/// Dividing by the largest magnitude first keeps the squares representable.
fn unit_row(row: Vec<f64>) -> Option<Vec<f64>> {
    let big = row.iter().fold(0.0, |m: f64, x| m.max(x.abs()));
    if big == 0.0 || !big.is_finite() {
        return None;
    }
    let scaled: Vec<f64> = row.iter().map(|x| x / big).collect();
    let norm = scaled.iter().fold(0.0, |s, x| s + x * x).sqrt();
    Some(scaled.into_iter().map(|x| x / norm).collect())
}

/// Column-pivoted Householder QR of an `m × n` matrix, stopped at the first
/// remaining column whose part outside the chosen columns' span is at most
/// [`RANK`]. `a` holds R in its first `rank` rows, columns permuted by `perm`.
#[derive(Debug)]
struct Qr {
    a: Vec<Vec<f64>>,
    perm: Vec<usize>,
    rank: usize,
}

impl Qr {
    fn new(mut a: Vec<Vec<f64>>, n: usize) -> Self {
        let m = a.len();
        let mut perm: Vec<usize> = (0..n).collect();
        let mut rank = 0;
        for k in 0..m.min(n) {
            // Recomputed norms (no downdating): ties go to the lowest original column.
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
            // A NaN norm ends the factorization too.
            if norm.is_nan() || norm <= RANK {
                break;
            }
            perm.swap(k, j);
            for row in a.iter_mut() {
                row.swap(k, j);
            }
            // Reflect column k onto alpha e_k, alpha of the sign that avoids cancellation.
            let alpha = if a[k][k] > 0.0 { -norm } else { norm };
            let mut v: Vec<f64> = (k..m).map(|i| a[i][k]).collect();
            v[0] -= alpha;
            let vv = v.iter().fold(0.0, |s, x| s + x * x);
            // Column sums over row-major storage: the index is the column.
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
        Self { a, perm, rank }
    }

    /// ADR-0003's null basis: one vector per non-pivot column in original
    /// order (that column 1, the other non-pivots 0, the pivots solved from
    /// R), unpermuted, then orthonormalized in that order with two
    /// Gram–Schmidt passes. Each is unit; components at or below [`RANK`]
    /// are canonical +0; the first nonzero component is positive.
    fn null_basis(&self) -> Vec<Vec<f64>> {
        let n = self.perm.len();
        let r = self.rank;
        let mut free: Vec<usize> = (r..n).collect();
        free.sort_by_key(|q| self.perm[*q]);
        let mut basis: Vec<Vec<f64>> = Vec::with_capacity(free.len());
        for q in free {
            let mut z = vec![0.0; n];
            z[q] = 1.0;
            for i in (0..r).rev() {
                let s = (i + 1..r).fold(self.a[i][q], |s, j| s + self.a[i][j] * z[j]);
                z[i] = -s / self.a[i][i];
            }
            let mut x = vec![0.0; n];
            for (i, value) in z.into_iter().enumerate() {
                x[self.perm[i]] = value;
            }
            for _ in 0..2 {
                for b in &basis {
                    let d = dot(b, &x);
                    for (xi, bi) in x.iter_mut().zip(b) {
                        *xi -= d * bi;
                    }
                }
            }
            let norm = dot(&x, &x).sqrt();
            // Unreachable in exact arithmetic (each vector owns a unit
            // non-pivot coordinate); a back substitution that overflowed is
            // dropped rather than reported as a direction.
            if !(norm > 0.0 && norm.is_finite()) {
                continue;
            }
            for xi in &mut x {
                *xi /= norm;
            }
            basis.push(x);
        }
        for x in &mut basis {
            let flip = x.iter().find(|v| v.abs() > RANK).is_some_and(|v| *v < 0.0);
            for xi in x.iter_mut() {
                *xi = if xi.abs() <= RANK {
                    0.0
                } else if flip {
                    -*xi
                } else {
                    *xi
                };
            }
        }
        basis
    }
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).fold(0.0, |s, (x, y)| s + x * y)
}

#[cfg(test)]
mod tests;
