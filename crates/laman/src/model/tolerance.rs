//! The named tolerances the model and the checker share.
//!
//! Length acceptance is relative to each structural component's scale
//! (ADR-0003). The solver freezes it from its input; the checker measures
//! it at the coordinates it checks, so guesses never loosen a verdict
//! (ADR-0004). Geometry measurements remain the responsibility of each caller.

/// Legacy absolute length constant, retained for source compatibility.
/// The checker uses [`Tolerances`] instead.
///
/// `1e-9` is close to the solver's target of a billionth of the sketch's
/// size for the C1 fixtures, which are about 100 units across. It is far
/// above the rounding of `f64` at that scale (about `1e-14`) and far below
/// any dimension a person types.
///
/// ```
/// use laman::model::tolerance;
/// assert!(tolerance::LENGTH > 0.0);
/// ```
pub const LENGTH: f64 = 1e-9;

/// How close two angles, or a dimensionless ratio and its target, must be
/// to count as equal, in radians.
///
/// `1e-12` is a few thousand times the rounding of an `atan2` result near
/// `π`, and tight enough that a length of 100 units is off by under
/// `1e-10` at its far end.
///
/// ```
/// use laman::model::tolerance;
/// assert!(tolerance::ANGLE < tolerance::LENGTH);
/// ```
pub const ANGLE: f64 = 1e-12;

use super::{Constraint, ConstraintId, Coordinate, Coordinates, Entity, EntityId, Sketch};
use crate::values::Values;
use std::collections::{BTreeMap, BTreeSet};

/// Absolute floor in caller units; features below this cannot be resolved.
pub const LENGTH_FLOOR: f64 = 1e-12;
/// Relative length acceptance, measured across scale in ADR-0003.
pub const LENGTH_RELATIVE: f64 = 1e-11;
/// The most an arc's tracked sweep may move in one drag frame, in radians
/// (an eighth of a turn), before the frame is bisected as a flip is
/// (ADR-0006).
///
/// A sweep is tracked as the nearest representative of its reading, which
/// tells moves apart only up to half a turn: a true move of `f` turns
/// reads as `f` for `f < 1/2` and as `f - 1` beyond. A bound of `m` turns
/// catches every true move in `(m, 1 - m)` turns and misses the rest, so
/// the smaller the better; a frame that really moves a sweep by an eighth
/// of a turn or more, in the rare drag fast enough, costs a halving or two.
///
/// ```
/// use laman::model::tolerance;
/// assert!(tolerance::MAX_SWEEP_STEP < std::f64::consts::PI);
/// ```
pub const MAX_SWEEP_STEP: f64 = std::f64::consts::FRAC_PI_4;

/// Dimensionless independence threshold for unit-normalized Jacobians.
pub const RANK: f64 = 1e-10;

/// Dimensionless stationarity that certifies a least-squares residual as a
/// conflict (ADR-0005): `|Jᵀf|` at most this times `|(f_i |J_i|)|`, rows and
/// residuals scaled as the solver scales them. Not [`RANK`]: an iteration
/// that compares costs locates a stationary point only to a few `√ε`
/// (1.5e-8; the C1 conflicting twins stop at 1.6e-10 to 3.2e-8). Not larger
/// either: a zero-residual solution approached across a quadratic
/// singularity keeps a ratio of about `√(2 LENGTH_RELATIVE)` (4.5e-6) while
/// its residual is still above tolerance, so a slow but satisfiable solve
/// is never certified.
pub(crate) const STATIONARY: f64 = 1e-6;

/// Length acceptance at a nonnegative finite component scale (ADR-0003).
///
/// ```
/// use laman::model::tolerance::{length, LENGTH_FLOOR};
/// assert_eq!(length(0.0), LENGTH_FLOOR);
/// ```
pub fn length(scale: f64) -> f64 {
    LENGTH_FLOOR.max(LENGTH_RELATIVE * scale)
}

pub(crate) type Param = (EntityId, Coordinate);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Source {
    Arc(EntityId),
    Constraint(ConstraintId),
}

#[derive(Clone, Debug)]
pub(crate) struct Component {
    pub params: Vec<Param>,
    pub sources: Vec<Source>,
    pub scale: f64,
    pub origin: [f64; 2],
    /// What the component's scale is measured over, found once so that
    /// measuring it again at other coordinates reads the component alone.
    pub frame: std::sync::Arc<Frame>,
    /// Every coordinate the component's equations and its frame can read
    /// ([`component_reads`]), made where the component is.
    pub reads: std::sync::Arc<[Param]>,
}

/// The structure [`extent`] measures a component over: its points, circle
/// radii, the constants its constraints add and its arcs. The measure is a
/// maximum over them and an extent of the points, so their order is free; it
/// is the same number [`extent`] gives at the same coordinates
/// (`frame_measures_what_extent_does`).
#[derive(Clone, Debug, Default)]
pub(crate) struct Frame {
    /// The component's points by id, with the sketch's own coordinates.
    points: Vec<(EntityId, [f64; 2])>,
    /// Its circles' radii by id, with the sketch's own.
    radii: Vec<(EntityId, f64)>,
    /// The largest dimension its constraints state.
    stated: f64,
    /// Where its `Fix` constraints put points.
    pinned: Vec<[f64; 2]>,
    /// Its arcs' centre and start points, with the sketch's own coordinates.
    arcs: Vec<([EntityId; 2], [[f64; 2]; 2])>,
    /// What a part solve measured once outside the part ([`Frame::split`]).
    outside: Option<Terms>,
}

impl Frame {
    fn of(sk: &Sketch, base: &Coordinates, ids: &BTreeSet<EntityId>, sources: &[Source]) -> Self {
        let get = |id, c| base.get(&(id, c)).copied().unwrap_or(f64::NAN);
        let xy = |id| [get(id, Coordinate::X), get(id, Coordinate::Y)];
        let mut points = BTreeSet::new();
        let mut radii = Vec::new();
        for id in ids {
            for (id, c) in entity_params(sk, *id) {
                match c {
                    Coordinate::X | Coordinate::Y => {
                        points.insert(id);
                    }
                    Coordinate::R => radii.push((id, get(id, c))),
                }
            }
        }
        let mut stated: f64 = 0.0;
        let mut pinned = Vec::new();
        for s in sources {
            if let Source::Constraint(id) = s {
                match sk.constraint(*id) {
                    Some(
                        Constraint::Distance { value, .. }
                        | Constraint::PointLineDistance { value, .. }
                        | Constraint::Radius { value, .. },
                    ) => stated = stated.max(*value),
                    Some(Constraint::Fix { x, y, .. }) => pinned.push([*x, *y]),
                    Some(
                        Constraint::Coincident { .. }
                        | Constraint::PointOnLine { .. }
                        | Constraint::PointOnCircle { .. }
                        | Constraint::Horizontal(_)
                        | Constraint::Vertical(_)
                        | Constraint::Parallel { .. }
                        | Constraint::Perpendicular { .. }
                        | Constraint::EqualLength { .. }
                        | Constraint::EqualRadius { .. }
                        | Constraint::TangentLineCircle { .. }
                        | Constraint::TangentCircles { .. }
                        | Constraint::Midpoint { .. }
                        | Constraint::Symmetric { .. }
                        | Constraint::Angle { .. },
                    )
                    | None => {}
                }
            }
        }
        let arcs =
            ids.iter()
                .filter_map(|id| match sk.entity(*id) {
                    Some(Entity::Arc { c, s, .. }) => {
                        Some(([c.id(), s.id()], [xy(c.id()), xy(s.id())]))
                    }
                    Some(Entity::Point { .. } | Entity::Line { .. } | Entity::Circle { .. })
                    | None => None,
                })
                .collect();
        Self {
            points: points.into_iter().map(|id| (id, xy(id))).collect(),
            radii,
            stated,
            pinned,
            arcs,
            outside: None,
        }
    }

    /// The scale and origin at `values`, a coordinate it lacks read from the
    /// sketch the frame was made from.
    pub(crate) fn measure(&self, values: &(impl Values + ?Sized)) -> (f64, [f64; 2]) {
        let mut terms = self.terms(values);
        if let Some(outside) = &self.outside {
            terms.join(outside);
        }
        terms.finish()
    }

    /// The frame of a part solve: what `self` measures, with everything that
    /// is not the part's own measured once at `values` and kept. See
    /// [`Splitter`], of which this is the case of one part.
    #[cfg(test)]
    pub(crate) fn split(&self, own: &[Param], values: &(impl Values + ?Sized)) -> Frame {
        Splitter::new(self, &[own]).split(0, values)
    }

    /// Everything a measure takes a maximum or an extent of, before the
    /// extent is turned into a length.
    fn terms(&self, values: &(impl Values + ?Sized)) -> Terms {
        let xy = |id: EntityId, own: [f64; 2]| {
            let at = |c, own| values.get((id, c)).unwrap_or(own);
            [at(Coordinate::X, own[0]), at(Coordinate::Y, own[1])]
        };
        let mut terms = Terms::default();
        for (id, own) in &self.radii {
            let r = values.get((*id, Coordinate::R)).unwrap_or(*own).abs();
            if r.is_finite() {
                terms.scale = terms.scale.max(r);
            }
        }
        // One pass: each point is read once, and the first of the least ids
        // is the origin, as `min_by_key` would pick it.
        for (id, own) in &self.points {
            let p = xy(*id, *own);
            if terms.origin.is_none_or(|(first, _)| *id < first) {
                terms.origin = Some((*id, p));
            }
            if p.iter().all(|v| v.is_finite()) {
                terms.include(p);
            }
        }
        terms.scale = terms.scale.max(self.stated);
        for p in &self.pinned {
            terms.include(*p);
        }
        for ([c, s], [c_own, s_own]) in &self.arcs {
            let a = xy(*c, *c_own);
            let b = xy(*s, *s_own);
            if a.iter().chain(&b).all(|v| v.is_finite()) {
                terms.scale = terms
                    .scale
                    .max(libm::hypot(b[0] - a[0], b[1] - a[1]).min(f64::MAX));
            }
        }
        terms
    }
}

/// The frames of a component's part solves, made together. A part moves only
/// its own parameters, so what is outside it is measured once ([`Terms`])
/// and the part's own entities at each evaluation. The entities are sorted
/// once by the parts that move them: none (constants), one (that part's
/// group) or several. The terms of a group are kept until its part has been
/// solved ([`Self::moved`]), so the part solves of one component measure
/// each entity about once and not once per part. Every term is a maximum, a
/// minimum or a box, so what is joined is what the whole frame measures at
/// the same coordinates, bit for bit (`split_frame_measures_what_the_whole_does`).
/// What a split joins is the whole's only at the coordinates it was made at:
/// a measure at other coordinates (a part's start, after earlier parts
/// moved) is the whole frame's, not a split's.
pub(crate) struct Splitter {
    /// Constants: no part moves them. Measured at the first split.
    fixed: Frame,
    /// What the whole frame was itself split from.
    before: Option<Terms>,
    /// The constants' terms joined with `before`, once measured.
    fixed_terms: Option<Terms>,
    /// The entities exactly one part moves, by part.
    groups: Vec<Frame>,
    /// The groups' terms as the leaves of a binary tree whose nodes are the
    /// joins of their children, so the join of all groups but one is read in
    /// O(log parts) and a group is measured again in O(log parts).
    tree: Vec<Terms>,
    /// The groups whose leaf is out of date: not yet measured, or moved
    /// since. Each is measured at the next split of another part.
    stale: Vec<usize>,
    /// The entities several parts move, with those parts.
    /// Each with its terms, kept until one of its parts has been solved.
    shared: Vec<(Vec<usize>, Frame, Option<Terms>)>,
}

impl Splitter {
    /// The split of `whole` by `parts`' parameters. A point is a part's if
    /// the part holds a coordinate of it, a circle if it holds its radius, an
    /// arc if it holds a coordinate of its centre or start.
    pub(crate) fn new(whole: &Frame, parts: &[&[Param]]) -> Self {
        let mut holders: BTreeMap<Param, Vec<usize>> = BTreeMap::new();
        for (j, part) in parts.iter().enumerate() {
            for p in *part {
                holders.entry(*p).or_default().push(j);
            }
        }
        let holding = |ps: &[Param]| -> Vec<usize> {
            let mut held: Vec<usize> = ps
                .iter()
                .filter_map(|p| holders.get(p))
                .flatten()
                .copied()
                .collect();
            held.sort_unstable();
            held.dedup();
            held
        };
        let xy = |id| [(id, Coordinate::X), (id, Coordinate::Y)];
        let mut fixed = Frame {
            stated: whole.stated,
            pinned: whole.pinned.clone(),
            ..Frame::default()
        };
        let mut groups = vec![Frame::default(); parts.len()];
        let mut shared: Vec<(Vec<usize>, Frame, Option<Terms>)> = Vec::new();
        // Puts one item where its holders say; `add` appends it to a frame.
        let mut place = |held: Vec<usize>, add: &dyn Fn(&mut Frame)| match held[..] {
            [] => add(&mut fixed),
            [j] => add(&mut groups[j]),
            _ => {
                let mut frame = Frame::default();
                add(&mut frame);
                shared.push((held, frame, None));
            }
        };
        for point in &whole.points {
            place(holding(&xy(point.0)), &|f| f.points.push(*point));
        }
        for radius in &whole.radii {
            place(holding(&[(radius.0, Coordinate::R)]), &|f| {
                f.radii.push(*radius);
            });
        }
        for arc in &whole.arcs {
            let [c, s] = arc.0;
            let ps = [
                (c, Coordinate::X),
                (c, Coordinate::Y),
                (s, Coordinate::X),
                (s, Coordinate::Y),
            ];
            place(holding(&ps), &|f| f.arcs.push(*arc));
        }
        Self {
            fixed,
            before: whole.outside.clone(),
            fixed_terms: None,
            tree: vec![Terms::default(); 2 * groups.len().next_power_of_two().max(1)],
            stale: (0..groups.len()).collect(),
            groups,
            shared,
        }
    }

    /// The frame of part `k` solved from `values`: its own entities, and
    /// everything else measured at `values` (cached for the parts not moved
    /// since).
    pub(crate) fn split(&mut self, k: usize, values: &(impl Values + ?Sized)) -> Frame {
        let (fixed, before) = (&self.fixed, &self.before);
        let mut kept = self
            .fixed_terms
            .get_or_insert_with(|| {
                let mut terms = fixed.terms(values);
                if let Some(before) = before {
                    terms.join(before);
                }
                terms
            })
            .clone();
        // Every group but `k` is measured now if it has not been since it
        // moved; the join of the others is the tree read around leaf `k`.
        let leaves = self.tree.len() / 2;
        let pending = std::mem::take(&mut self.stale);
        for j in pending {
            if j == k {
                self.stale.push(j);
            } else if let Some(group) = self.groups.get(j) {
                let mut i = leaves + j;
                self.tree[i] = group.terms(values);
                while i > 1 {
                    i >>= 1;
                    let mut node = self.tree[2 * i].clone();
                    node.join(&self.tree[2 * i + 1]);
                    self.tree[i] = node;
                }
            }
        }
        if k < self.groups.len() {
            let mut i = leaves + k;
            while i > 1 {
                kept.join(&self.tree[i ^ 1]);
                i >>= 1;
            }
        } else {
            kept.join(&self.tree[1]);
        }
        let mut inside = self.groups.get(k).cloned().unwrap_or_default();
        for (held, frame, cached) in &mut self.shared {
            if held.contains(&k) {
                inside.points.extend_from_slice(&frame.points);
                inside.radii.extend_from_slice(&frame.radii);
                inside.arcs.extend_from_slice(&frame.arcs);
            } else {
                kept.join(cached.get_or_insert_with(|| frame.terms(values)));
            }
        }
        inside.outside = Some(kept);
        inside
    }

    /// Part `k` has been solved: what it moved is measured again when next
    /// needed.
    pub(crate) fn moved(&mut self, k: usize) {
        if k < self.groups.len() && !self.stale.contains(&k) {
            self.stale.push(k);
        }
        for (held, _, cached) in &mut self.shared {
            if held.contains(&k) {
                *cached = None;
            }
        }
    }
}

/// A frame's measure before its extent is read: the largest radius,
/// dimension and arc length, the bounding box of its positions and its
/// first point by id. Joining two is joining their maxima, minima and
/// first points, so the order of the parts does not reach the result.
#[derive(Clone, Debug)]
pub(crate) struct Terms {
    scale: f64,
    /// The first point by id and where it is, finite or not.
    origin: Option<(EntityId, [f64; 2])>,
    min: [f64; 2],
    max: [f64; 2],
    /// Whether any position entered the box.
    any: bool,
}

impl Default for Terms {
    fn default() -> Self {
        Self {
            scale: 0.0,
            origin: None,
            min: [f64::INFINITY; 2],
            max: [f64::NEG_INFINITY; 2],
            any: false,
        }
    }
}

impl Terms {
    fn include(&mut self, p: [f64; 2]) {
        self.any = true;
        self.min = [self.min[0].min(p[0]), self.min[1].min(p[1])];
        self.max = [self.max[0].max(p[0]), self.max[1].max(p[1])];
    }

    fn join(&mut self, other: &Terms) {
        self.scale = self.scale.max(other.scale);
        self.any |= other.any;
        self.min = [self.min[0].min(other.min[0]), self.min[1].min(other.min[1])];
        self.max = [self.max[0].max(other.max[0]), self.max[1].max(other.max[1])];
        self.origin = match (self.origin, other.origin) {
            (Some(a), Some(b)) => Some(if b.0 < a.0 { b } else { a }),
            (a, b) => a.or(b),
        };
    }

    fn finish(self) -> (f64, [f64; 2]) {
        let mut scale = self.scale;
        if self.any {
            for axis in 0..2 {
                scale = scale.max((self.max[axis] - self.min[axis]).min(f64::MAX));
            }
        }
        (scale, self.origin.map_or([0.0; 2], |(_, p)| p))
    }
}

/// Component-relative length tolerances: [`Tolerances::new`] at the sketch's
/// stored coordinates (the solver's frozen input), [`Tolerances::at`] at given
/// ones (the checker's). Ground references do not connect otherwise
/// independent components. Unknown ids return the absolute floor. No residual
/// or solver code is used.
///
/// ```
/// use laman::model::{Sketch, tolerance::Tolerances};
/// let mut sk = Sketch::new();
/// let a = sk.point(0.0, 0.0)?;
/// let b = sk.point(3.0, 4.0)?;
/// let id = sk.distance(a, b, 5.0)?;
/// assert_eq!(Tolerances::new(&sk).constraint(id), 5.0 * laman::model::tolerance::LENGTH_RELATIVE);
/// # Ok::<(), laman::model::ModelError>(())
/// ```
#[derive(Clone, Debug)]
pub struct Tolerances {
    pub(crate) components: Vec<Component>,
    constraints: BTreeMap<ConstraintId, f64>,
    entities: BTreeMap<EntityId, f64>,
    /// Each component's entities, in `components`' order.
    members: Vec<std::sync::Arc<[EntityId]>>,
    /// The entities in no component: measured alone, and again at other
    /// coordinates.
    lone: Vec<EntityId>,
}

// Structural reads, not nonzero derivatives. Expand references explicitly;
// zero derivatives and exact degeneracy must never split a component.
pub(crate) fn entity_params(sk: &Sketch, id: EntityId) -> Vec<Param> {
    match sk.entity(id) {
        Some(Entity::Point { .. }) => vec![(id, Coordinate::X), (id, Coordinate::Y)],
        Some(Entity::Circle { c, .. }) => {
            let mut p = entity_params(sk, c.id());
            p.push((id, Coordinate::R));
            p
        }
        Some(Entity::Line { .. } | Entity::Arc { .. }) => sk
            .entity(id)
            .into_iter()
            .flat_map(Entity::refs)
            .flat_map(|r| entity_params(sk, r))
            .collect(),
        None => vec![],
    }
}

pub(crate) fn source_entities(sk: &Sketch, source: Source) -> Vec<EntityId> {
    match source {
        Source::Arc(id) => vec![id],
        Source::Constraint(id) => sk
            .constraint(id)
            .map(Constraint::entities)
            .unwrap_or_default(),
    }
}

pub(crate) fn dependencies(sk: &Sketch, source: Source) -> Vec<Param> {
    let mut p: Vec<_> = source_entities(sk, source)
        .into_iter()
        .flat_map(|id| entity_params(sk, id))
        .collect();
    if let Source::Constraint(id) = source {
        match sk.constraint(id) {
            Some(Constraint::Horizontal(_)) => p.retain(|(_, c)| *c == Coordinate::Y),
            Some(Constraint::Vertical(_)) => p.retain(|(_, c)| *c == Coordinate::X),
            Some(Constraint::Radius { round, .. }) => {
                if matches!(sk.entity(round.id()), Some(Entity::Circle { .. })) {
                    p = vec![(round.id(), Coordinate::R)];
                }
            }
            Some(Constraint::EqualRadius { a, b }) => {
                p = [a, b]
                    .into_iter()
                    .flat_map(|r| {
                        if matches!(sk.entity(r.id()), Some(Entity::Circle { .. })) {
                            vec![(r.id(), Coordinate::R)]
                        } else {
                            entity_params(sk, r.id())
                        }
                    })
                    .collect();
            }
            Some(
                Constraint::Coincident { .. }
                | Constraint::PointOnLine { .. }
                | Constraint::PointOnCircle { .. }
                | Constraint::Parallel { .. }
                | Constraint::Perpendicular { .. }
                | Constraint::EqualLength { .. }
                | Constraint::TangentLineCircle { .. }
                | Constraint::TangentCircles { .. }
                | Constraint::Midpoint { .. }
                | Constraint::Symmetric { .. }
                | Constraint::Fix { .. }
                | Constraint::Distance { .. }
                | Constraint::PointLineDistance { .. }
                | Constraint::Angle { .. },
            )
            | None => {}
        }
    }
    p.sort();
    p.dedup();
    p
}

fn root(parent: &[usize], mut i: usize) -> usize {
    while parent[i] != i {
        i = parent[i];
    }
    i
}

impl Tolerances {
    /// Freeze scales from the sketch's stored coordinates, its dimensions and
    /// structural dependencies. See the type example.
    pub fn new(sk: &Sketch) -> Self {
        let own = sk.coordinates();
        Self::build(sk, &own, &own)
    }

    /// Scales measured at `values`, a coordinate missing from it read from
    /// the sketch. A non-finite value enters no extent, so it cannot loosen
    /// the rest of its component; the sketch's stored coordinates (guesses)
    /// never enter at all (ADR-0004).
    ///
    /// ```
    /// use laman::model::{Coordinate, Sketch, tolerance::{length, Tolerances}};
    /// let mut sk = Sketch::new();
    /// let o = sk.point_fixed(0.0, 0.0)?;
    /// let far = sk.point(1e6, 0.0)?; // a guess far from the solution
    /// let id = sk.distance(o, far, 1.0)?;
    /// let mut solution = sk.coordinates();
    /// solution.insert((far.id(), Coordinate::X), 1.0);
    /// assert_eq!(Tolerances::new(&sk).constraint(id), length(1e6));
    /// assert_eq!(Tolerances::at(&sk, &solution).constraint(id), length(1.0));
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn at(sk: &Sketch, values: &Coordinates) -> Self {
        let own = sk.coordinates();
        let coords = merged(&own, values);
        Self::build(sk, &own, &coords)
    }

    /// The components and scales of `sk`, measured at `coords` (the sketch's
    /// own coordinates overlaid with the values asked for) and framed at
    /// `own`.
    fn build(sk: &Sketch, own: &Coordinates, coords: &Coordinates) -> Self {
        let params: Vec<_> = own
            .keys()
            .copied()
            .filter(|(id, c)| !sk.is_fixed(*id, *c))
            .collect();
        let indices: BTreeMap<_, _> = params.iter().enumerate().map(|(i, p)| (*p, i)).collect();
        let sources: Vec<_> = sk
            .entities()
            .filter_map(|(id, e)| matches!(e, Entity::Arc { .. }).then_some(Source::Arc(id)))
            .chain(sk.constraints().map(|(id, _)| Source::Constraint(id)))
            .collect();
        let reads: Vec<Vec<usize>> = sources
            .iter()
            .map(|s| {
                dependencies(sk, *s)
                    .iter()
                    .filter_map(|p| indices.get(p).copied())
                    .collect()
            })
            .collect();
        let mut parent: Vec<_> = (0..params.len()).collect();
        for read in &reads {
            if let Some(&first) = read.first() {
                for &i in read {
                    let a = root(&parent, first);
                    let b = root(&parent, i);
                    parent[a.max(b)] = a.min(b);
                }
            }
        }
        let mut grouped: BTreeMap<usize, Component> = BTreeMap::new();
        for (i, p) in params.iter().enumerate() {
            grouped
                .entry(root(&parent, i))
                .or_insert_with(|| Component {
                    params: vec![],
                    sources: vec![],
                    scale: 0.0,
                    origin: [0.0; 2],
                    frame: Default::default(),
                    reads: Default::default(),
                })
                .params
                .push(*p);
        }
        for (i, (s, read)) in sources.iter().zip(&reads).enumerate() {
            let key = read
                .first()
                .map(|j| root(&parent, *j))
                .unwrap_or(params.len() + i);
            grouped
                .entry(key)
                .or_insert_with(|| Component {
                    params: vec![],
                    sources: vec![],
                    scale: 0.0,
                    origin: [0.0; 2],
                    frame: Default::default(),
                    reads: Default::default(),
                })
                .sources
                .push(*s);
        }
        let mut components: Vec<_> = grouped.into_values().collect();
        let mut members = Vec::with_capacity(components.len());
        for component in &mut components {
            let ids = component_entities(sk, component);
            component.frame = std::sync::Arc::new(Frame::of(sk, own, &ids, &component.sources));
            component.reads = reads_of(sk, &ids);
            members.push(ids.into_iter().collect());
        }
        let mut tolerances = Self {
            components,
            constraints: BTreeMap::new(),
            entities: BTreeMap::new(),
            members,
            lone: Vec::new(),
        };
        tolerances.measure(coords);
        let in_a_component: BTreeSet<EntityId> = tolerances
            .members
            .iter()
            .flat_map(|m| m.iter().copied())
            .collect();
        tolerances.lone = sk
            .entities()
            .map(|(id, _)| id)
            .filter(|id| !in_a_component.contains(id))
            .collect();
        tolerances.measure_lone(sk, coords);
        tolerances
    }

    /// The same components measured at `values` (the sketch's own
    /// coordinates where it lacks one): what [`Tolerances::at`] returns for
    /// the same sketch and values, without finding the components again.
    pub(crate) fn remeasured(&self, sk: &Sketch, values: &Coordinates) -> Self {
        let mut again = self.clone();
        again.measure(values);
        if !again.lone.is_empty() {
            let coords = merged(&sk.coordinates(), values);
            again.measure_lone(sk, &coords);
        }
        again
    }

    /// Each component's scale and origin at `values`, and the thresholds of
    /// its constraints and entities.
    fn measure(&mut self, values: &Coordinates) {
        self.constraints.clear();
        self.entities.clear();
        for (component, ids) in self.components.iter_mut().zip(&self.members) {
            let (scale, origin) = component.frame.measure(values);
            component.scale = scale;
            component.origin = origin;
            let tol = length(scale);
            for id in ids.iter() {
                self.entities
                    .entry(*id)
                    .and_modify(|v| *v = v.max(tol))
                    .or_insert(tol);
            }
            for s in &component.sources {
                if let Source::Constraint(id) = s {
                    self.constraints.insert(*id, tol);
                }
            }
        }
    }

    /// Entities in no component still have a meaningful local size. One in a
    /// component never needs it: a component's scale covers every position,
    /// radius and arc of its entities, so it is at least the entity's own.
    fn measure_lone(&mut self, sk: &Sketch, coords: &Coordinates) {
        for id in &self.lone {
            let own = length(extent(sk, coords, &BTreeSet::from([*id]), &[]).0);
            self.entities
                .entry(*id)
                .and_modify(|v| *v = v.max(own))
                .or_insert(own);
        }
    }
    /// Frozen length threshold for a constraint, or the floor for an unknown id.
    /// See [`Tolerances`] for an example.
    pub fn constraint(&self, id: ConstraintId) -> f64 {
        self.constraints.get(&id).copied().unwrap_or(LENGTH_FLOOR)
    }
    /// Frozen length threshold for an entity, or the floor for an unknown id.
    ///
    /// ```
    /// use laman::model::{Sketch, tolerance::Tolerances};
    /// let mut sk = Sketch::new();
    /// let p = sk.point(0.0, 0.0)?;
    /// assert!(Tolerances::new(&sk).entity(p.id()) > 0.0);
    /// # Ok::<(), laman::model::ModelError>(())
    /// ```
    pub fn entity(&self, id: EntityId) -> f64 {
        self.entities.get(&id).copied().unwrap_or(LENGTH_FLOOR)
    }
}

/// The two measures of a sketch one frame needs: [`Tolerances::new`] (the
/// solve's and the analysis's systems are compiled under it) and
/// [`Tolerances::at`] the frame's final coordinates (the branch observation
/// and the degeneracy test). Made once and shared, never remade; each is
/// what a fresh call at the same sketch and coordinates returns.
#[derive(Clone, Debug)]
pub(crate) struct FrameTolerances {
    /// At the sketch's stored coordinates.
    pub frozen: Tolerances,
    /// At the coordinates the frame reached.
    pub reached: Tolerances,
}

impl FrameTolerances {
    #[cfg(any(test, feature = "reference"))]
    pub(crate) fn of(sk: &Sketch, reached: &Coordinates) -> Self {
        Self {
            frozen: Tolerances::new(sk),
            reached: Tolerances::at(sk, reached),
        }
    }
}

/// The sketch's coordinates overlaid with `values` for the keys it has.
fn merged(own: &Coordinates, values: &Coordinates) -> Coordinates {
    let mut coords = own.clone();
    for (key, value) in coords.iter_mut() {
        if let Some(v) = values.get(key) {
            *value = *v;
        }
    }
    coords
}

/// The scale of `component` at `values`, measured as [`Tolerances::at`] does.
pub(crate) fn scale_at(component: &Component, values: &(impl Values + ?Sized)) -> f64 {
    component.frame.measure(values).0
}

/// Every coordinate `component`'s equations and its frame can read: the
/// parameters of its entities, fixed ones included, in order.
pub(crate) fn component_reads(sk: &Sketch, component: &Component) -> std::sync::Arc<[Param]> {
    reads_of(sk, &component_entities(sk, component))
}

fn reads_of(sk: &Sketch, ids: &BTreeSet<EntityId>) -> std::sync::Arc<[Param]> {
    let mut reads: Vec<Param> = ids.iter().flat_map(|id| entity_params(sk, *id)).collect();
    reads.sort();
    reads.dedup();
    reads.into()
}

fn component_entities(sk: &Sketch, component: &Component) -> BTreeSet<EntityId> {
    let mut ids: BTreeSet<_> = component.params.iter().map(|p| p.0).collect();
    for s in &component.sources {
        ids.extend(source_entities(sk, *s));
    }
    ids
}

// Every key `entity_params` yields is in `coords` (`merged` starts from the
// sketch's complete coordinates). Non-finite values are skipped, never saturated.
fn extent(
    sk: &Sketch,
    coords: &Coordinates,
    ids: &BTreeSet<EntityId>,
    sources: &[Source],
) -> (f64, [f64; 2]) {
    let mut points = BTreeSet::new();
    let mut scale: f64 = 0.0;
    for id in ids {
        for (id, c) in entity_params(sk, *id) {
            match c {
                Coordinate::X | Coordinate::Y => {
                    points.insert(id);
                }
                Coordinate::R => {
                    let r = coords[&(id, c)].abs();
                    if r.is_finite() {
                        scale = scale.max(r);
                    }
                }
            }
        }
    }
    let xy = |id| [coords[&(id, Coordinate::X)], coords[&(id, Coordinate::Y)]];
    let origin = points.first().map(|id| xy(*id)).unwrap_or([0.0; 2]);
    let mut positions: Vec<_> = points
        .into_iter()
        .map(xy)
        .filter(|p| p.iter().all(|v| v.is_finite()))
        .collect();
    for s in sources {
        if let Source::Constraint(id) = s {
            match sk.constraint(*id) {
                Some(
                    Constraint::Distance { value, .. }
                    | Constraint::PointLineDistance { value, .. }
                    | Constraint::Radius { value, .. },
                ) => scale = scale.max(*value),
                Some(Constraint::Fix { x, y, .. }) => positions.push([*x, *y]),
                Some(
                    Constraint::Coincident { .. }
                    | Constraint::PointOnLine { .. }
                    | Constraint::PointOnCircle { .. }
                    | Constraint::Horizontal(_)
                    | Constraint::Vertical(_)
                    | Constraint::Parallel { .. }
                    | Constraint::Perpendicular { .. }
                    | Constraint::EqualLength { .. }
                    | Constraint::EqualRadius { .. }
                    | Constraint::TangentLineCircle { .. }
                    | Constraint::TangentCircles { .. }
                    | Constraint::Midpoint { .. }
                    | Constraint::Symmetric { .. }
                    | Constraint::Angle { .. },
                )
                | None => {}
            }
        }
    }
    for id in ids {
        if let Some(Entity::Arc { c, s, .. }) = sk.entity(*id) {
            let a = xy(c.id());
            let b = xy(s.id());
            if a.iter().chain(&b).all(|v| v.is_finite()) {
                scale = scale.max(libm::hypot(b[0] - a[0], b[1] - a[1]).min(f64::MAX));
            }
        }
    }
    for axis in 0..2 {
        let min = positions
            .iter()
            .map(|p| p[axis])
            .fold(f64::INFINITY, f64::min);
        let max = positions
            .iter()
            .map(|p| p[axis])
            .fold(f64::NEG_INFINITY, f64::max);
        if !positions.is_empty() {
            scale = scale.max((max - min).min(f64::MAX));
        }
    }
    (scale, origin)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// Many independent components: the entities in none are the same as a
    /// scan of every component's members finds, and every entity has a
    /// threshold.
    #[test]
    fn many_components_find_the_same_lone_entities() {
        let mut sk = Sketch::new();
        let mut loose = vec![];
        for i in 0..500 {
            let x = i as f64 * 10.0;
            let a = sk.point(x, 0.0).unwrap();
            let b = sk.point(x + 3.0, 1.0).unwrap();
            sk.distance(a, b, 3.0 + (i % 5) as f64).unwrap();
            if i % 7 == 0 {
                loose.push(sk.point_fixed(x, 50.0).unwrap().id());
            }
        }
        let t = Tolerances::new(&sk);
        assert!(t.components.len() >= 500);
        let want: Vec<EntityId> = sk
            .entities()
            .map(|(id, _)| id)
            .filter(|id| !t.members.iter().any(|m| m.binary_search(id).is_ok()))
            .collect();
        assert_eq!(t.lone, want);
        for id in &loose {
            assert!(t.lone.contains(id));
        }
        for (id, _) in sk.entities() {
            assert!(t.entity(id) > 0.0);
        }
    }

    /// The frame measures a component as [`extent`] does, bit for bit, at the
    /// sketch's coordinates, at moved ones, at a partial map and with a
    /// non-finite value.
    #[test]
    fn frame_measures_what_extent_does() {
        let mut sk = Sketch::new();
        let o = sk.point_fixed(1.0, -2.0).unwrap();
        let c = sk.point(3.0, 4.0).unwrap();
        let s = sk.point(8.0, 4.5).unwrap();
        let e = sk.point(3.0, 9.0).unwrap();
        let arc = sk.arc(c, s, e).unwrap();
        let q = sk.point(-6.0, 7.0).unwrap();
        let circle = sk.circle(q, 2.5).unwrap();
        sk.distance(o, c, 12.0).unwrap();
        sk.radius(circle, 3.5).unwrap();
        sk.fix(e, 20.0, 1.0).unwrap();
        sk.distance(q, s, 4.0).unwrap();
        sk.point(100.0, 100.0).unwrap();
        let _ = arc;
        let base = sk.coordinates();
        let mut moved = base.clone();
        for (i, v) in moved.values_mut().enumerate() {
            *v += i as f64 * 0.37 - 1.0;
        }
        let mut partial = Coordinates::new();
        partial.insert((s.id(), Coordinate::X), -40.0);
        let mut broken = base.clone();
        broken.insert((c.id(), Coordinate::Y), f64::NAN);
        for component in Tolerances::new(&sk).components {
            for values in [&base, &moved, &partial, &broken] {
                let ids = component_entities(&sk, &component);
                let want = extent(
                    &sk,
                    &merged(&sk.coordinates(), values),
                    &ids,
                    &component.sources,
                );
                let got = component.frame.measure(values);
                assert_eq!(got.0.to_bits(), want.0.to_bits());
                assert_eq!(got.1.map(f64::to_bits), want.1.map(f64::to_bits));
                let dense = crate::values::Dense::of(values);
                let again = component.frame.measure(&dense);
                assert_eq!(again.0.to_bits(), want.0.to_bits());
                assert_eq!(again.1.map(f64::to_bits), want.1.map(f64::to_bits));
                // Frame::terms, before its maximum and extent are taken.
                assert_eq!(
                    format!("{:?}", component.frame.terms(&dense)),
                    format!("{:?}", component.frame.terms(values)),
                );
            }
        }
    }

    /// A split frame measures what the whole frame does at any coordinates
    /// that differ from the split's only in the part's own parameters: bit
    /// for bit, for one sketch built to have every kind of term.
    fn assert_split_measures_whole(sk: &Sketch, own_every: usize, shift: f64, big: f64) {
        let base = sk.coordinates();
        for component in Tolerances::new(sk).components {
            let own: Vec<Param> = component
                .params
                .iter()
                .copied()
                .enumerate()
                .filter(|(i, _)| i % own_every == 0)
                .map(|(_, p)| p)
                .collect();
            let split = component.frame.split(&own, &base);
            let mut moved = base.clone();
            for (k, p) in own.iter().enumerate() {
                let v = match k % 6 {
                    0 => shift * (k as f64 + 1.0),
                    1 => big,
                    2 => -big,
                    3 => 0.0,
                    4 => -0.0,
                    _ => base[p] + shift,
                };
                moved.insert(*p, v);
            }
            let mut broken = moved.clone();
            if let Some(p) = own.first() {
                broken.insert(*p, f64::NAN);
            }
            let mut partial = Coordinates::new();
            for p in &own {
                partial.insert(*p, shift);
            }
            for values in [&base, &moved, &broken, &partial] {
                let want = component.frame.measure(values);
                let got = split.measure(values);
                assert_eq!(got.0.to_bits(), want.0.to_bits(), "scale");
                assert_eq!(got.1.map(f64::to_bits), want.1.map(f64::to_bits), "origin");
            }
        }
    }

    /// Parts solved in turn, each moving only its parameters, each split
    /// from the coordinates the earlier ones left: the joined measure is the
    /// whole's at the part's moved coordinates, parts sharing entities (an
    /// overlapping `held`, as a point whose X and Y fall to different parts)
    /// included.
    fn assert_splitter_measures_whole(sk: &Sketch, parts: usize, shift: f64, big: f64) {
        let base = sk.coordinates();
        for component in Tolerances::new(sk).components {
            let held: Vec<Vec<Param>> = (0..parts)
                .map(|j| {
                    component
                        .params
                        .iter()
                        .copied()
                        .enumerate()
                        .filter(|(i, _)| i % parts == j)
                        .map(|(_, p)| p)
                        .collect()
                })
                .collect();
            let slices: Vec<&[Param]> = held.iter().map(Vec::as_slice).collect();
            let mut splitter = Splitter::new(&component.frame, &slices);
            let mut values = base.clone();
            for (j, own) in held.iter().enumerate() {
                let split = splitter.split(j, &values);
                for (k, p) in own.iter().enumerate() {
                    let v = match (j + k) % 6 {
                        0 => shift * (k as f64 + 1.0),
                        1 => big,
                        2 => -big,
                        3 => 0.0,
                        4 => -0.0,
                        _ => base[p] + shift,
                    };
                    values.insert(*p, v);
                }
                let want = component.frame.measure(&values);
                let got = split.measure(&values);
                assert_eq!(got.0.to_bits(), want.0.to_bits(), "scale");
                assert_eq!(got.1.map(f64::to_bits), want.1.map(f64::to_bits), "origin");
                splitter.moved(j);
            }
        }
    }

    /// The thresholds as the first implementation measured them: `extent`
    /// over every component's entities and over every entity alone, at the
    /// sketch's coordinates overlaid with `values`.
    fn assert_remeasured_is_at(sk: &Sketch, values: &Coordinates) {
        let own = sk.coordinates();
        let coords = merged(&own, values);
        let frozen = Tolerances::new(sk);
        let again = frozen.remeasured(sk, values);
        let fresh = Tolerances::at(sk, values);
        let mut constraints = BTreeMap::new();
        let mut entities: BTreeMap<EntityId, f64> = BTreeMap::new();
        for (a, b) in again.components.iter().zip(&fresh.components) {
            assert_eq!(a.params, b.params);
            assert_eq!(a.sources, b.sources);
            assert_eq!(a.scale.to_bits(), b.scale.to_bits());
            assert_eq!(a.origin.map(f64::to_bits), b.origin.map(f64::to_bits));
        }
        assert_eq!(again.components.len(), fresh.components.len());
        for component in &fresh.components {
            let ids = component_entities(sk, component);
            let (scale, origin) = extent(sk, &coords, &ids, &component.sources);
            assert_eq!(component.scale.to_bits(), scale.to_bits());
            assert_eq!(component.origin.map(f64::to_bits), origin.map(f64::to_bits));
            let tol = length(scale);
            for id in ids {
                entities
                    .entry(id)
                    .and_modify(|v| *v = v.max(tol))
                    .or_insert(tol);
            }
            for s in &component.sources {
                if let Source::Constraint(id) = s {
                    constraints.insert(*id, tol);
                }
            }
        }
        for (id, _) in sk.entities() {
            let own = length(extent(sk, &coords, &BTreeSet::from([id]), &[]).0);
            entities
                .entry(id)
                .and_modify(|v| *v = v.max(own))
                .or_insert(own);
        }
        for t in [&again, &fresh] {
            assert_eq!(t.constraints.len(), constraints.len());
            for (id, v) in &constraints {
                assert_eq!(t.constraints[id].to_bits(), v.to_bits());
            }
            assert_eq!(t.entities.len(), entities.len());
            for (id, v) in &entities {
                assert_eq!(t.entities[id].to_bits(), v.to_bits());
            }
        }
    }

    fn assert_remeasured_at_extremes(sk: &Sketch, shift: f64, big: f64) {
        let base = sk.coordinates();
        let mut moved = base.clone();
        let mut broken = base.clone();
        let mut partial = Coordinates::new();
        for (k, (p, v)) in base.iter().enumerate() {
            let w = match k % 6 {
                0 => *v + shift,
                1 => big,
                2 => -big,
                3 => 0.0,
                4 => -0.0,
                _ => shift * (k as f64 + 1.0),
            };
            moved.insert(*p, w);
            if k % 3 == 0 {
                partial.insert(*p, w);
            }
        }
        if let Some(p) = base.keys().next() {
            broken.insert(*p, f64::NAN);
        }
        for values in [&base, &moved, &broken, &partial, &Coordinates::new()] {
            assert_remeasured_is_at(sk, values);
        }
    }

    proptest::proptest! {
        #[test]
        fn remeasured_tolerances_are_those_of_at(
            steps in proptest::collection::vec(
                (-1e3..1e3_f64, -1e3..1e3_f64, 0..7_u8, 0..64_usize),
                1..24,
            ),
            shift in -1e3..1e3_f64,
            extreme in proptest::sample::select(vec![1e-300, 1.0, 1e150, 1e300, 1.7e308]),
        ) {
            let Some(sk) = constructed(&steps) else { return Ok(()); };
            assert_remeasured_at_extremes(&sk, shift, extreme);
        }
    }

    #[test]
    fn remeasured_tolerances_are_those_of_at_on_the_corpus() {
        use laman_debug::corpus::{rect_chain, slot_grid, triangle_strip};
        for case in [rect_chain(8), slot_grid(2, 3), triangle_strip(9)] {
            let case = case.unwrap();
            let sk: Sketch =
                serde_json::from_str(&serde_json::to_string(&case.sketch).unwrap()).unwrap();
            assert_remeasured_at_extremes(&sk, 0.75, 1e300);
        }
    }

    fn constructed(steps: &[(f64, f64, u8, usize)]) -> Option<Sketch> {
        let mut sk = Sketch::new();
        let mut points = vec![sk.point_fixed(1.0, -2.0).ok()?];
        for (x, y, kind, link) in steps {
            let q = sk.point(*x, *y).ok()?;
            let other = points[link % points.len()];
            match kind % 7 {
                0 => {
                    sk.distance(other, q, x.abs() + 0.5).ok()?;
                }
                1 => {
                    let c = sk.circle(q, y.abs() + 0.5).ok()?;
                    sk.radius(c, x.abs() + 1.0).ok()?;
                }
                2 => {
                    sk.fix(q, *y, *x).ok()?;
                }
                3 if points.len() >= 2 => {
                    let e = points[(link + 1) % points.len()];
                    if other.id() != e.id() {
                        sk.arc(q, other, e).ok()?;
                    }
                }
                4 => {
                    sk.coincident(other, q).ok()?;
                }
                _ => {}
            }
            points.push(q);
        }
        Some(sk)
    }

    proptest::proptest! {
        #[test]
        fn split_frame_measures_what_the_whole_does(
            steps in proptest::collection::vec(
                (-1e3..1e3_f64, -1e3..1e3_f64, 0..7_u8, 0..64_usize),
                1..24,
            ),
            own_every in 1..6_usize,
            shift in -1e3..1e3_f64,
            extreme in proptest::sample::select(vec![1e-300, 1e-150, 1.0, 1e150, 1e300, 1.7e308]),
        ) {
            let Some(sk) = constructed(&steps) else { return Ok(()); };
            assert_split_measures_whole(&sk, own_every, shift, extreme);
            assert_splitter_measures_whole(&sk, own_every + 1, shift, extreme);
        }
    }

    /// The same on the C2 corpus, whose components the staged solve splits.
    #[test]
    fn split_frame_measures_what_the_whole_does_on_the_corpus() {
        use laman_debug::corpus::{rect_chain, slot_grid, triangle_strip};
        for case in [rect_chain(8), slot_grid(2, 3), triangle_strip(9)] {
            let case = case.unwrap();
            let sk: Sketch =
                serde_json::from_str(&serde_json::to_string(&case.sketch).unwrap()).unwrap();
            for every in [1, 2, 5] {
                assert_split_measures_whole(&sk, every, 0.75, 1e300);
                assert_splitter_measures_whole(&sk, every + 1, 0.75, 1e300);
            }
        }
    }

    /// Hundreds of parts moving in turn, the tree's depth and padding
    /// included: the joined measure stays the whole's.
    #[test]
    fn splitter_measures_the_whole_over_500_parts() {
        use laman_debug::corpus::rect_chain;
        let case = rect_chain(125).unwrap();
        let sk: Sketch =
            serde_json::from_str(&serde_json::to_string(&case.sketch).unwrap()).unwrap();
        for parts in [499, 500] {
            assert_splitter_measures_whole(&sk, parts, 0.75, 1e300);
        }
    }
}
