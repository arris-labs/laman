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
        }
    }

    /// The scale and origin at `values`, a coordinate it lacks read from the
    /// sketch the frame was made from.
    pub(crate) fn measure(&self, values: &Coordinates) -> (f64, [f64; 2]) {
        let xy = |id: EntityId, own: [f64; 2]| {
            let at = |c, own| values.get(&(id, c)).copied().unwrap_or(own);
            [at(Coordinate::X, own[0]), at(Coordinate::Y, own[1])]
        };
        let mut scale: f64 = 0.0;
        for (id, own) in &self.radii {
            let r = values
                .get(&(*id, Coordinate::R))
                .copied()
                .unwrap_or(*own)
                .abs();
            if r.is_finite() {
                scale = scale.max(r);
            }
        }
        let origin = self
            .points
            .first()
            .map(|(id, own)| xy(*id, *own))
            .unwrap_or([0.0; 2]);
        let mut positions: Vec<[f64; 2]> = self
            .points
            .iter()
            .map(|(id, own)| xy(*id, *own))
            .filter(|p| p.iter().all(|v| v.is_finite()))
            .collect();
        scale = scale.max(self.stated);
        positions.extend(&self.pinned);
        for ([c, s], [c_own, s_own]) in &self.arcs {
            let a = xy(*c, *c_own);
            let b = xy(*s, *s_own);
            if a.iter().chain(&b).all(|v| v.is_finite()) {
                scale = scale.max(libm::hypot(b[0] - a[0], b[1] - a[1]).min(f64::MAX));
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
        Self::at(sk, &sk.coordinates())
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
        let coords = merged(sk, values);
        let params: Vec<_> = sk
            .coordinates()
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
                })
                .sources
                .push(*s);
        }
        let mut components: Vec<_> = grouped.into_values().collect();
        let own = sk.coordinates();
        let mut constraints = BTreeMap::new();
        let mut entities: BTreeMap<EntityId, f64> = BTreeMap::new();
        for component in &mut components {
            let ids = component_entities(sk, component);
            let (scale, origin) = extent(sk, &coords, &ids, &component.sources);
            component.scale = scale;
            component.origin = origin;
            component.frame = std::sync::Arc::new(Frame::of(sk, &own, &ids, &component.sources));
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
        // Unconstrained geometric entities still have a meaningful local size.
        for (id, _) in sk.entities() {
            let own = length(extent(sk, &coords, &BTreeSet::from([id]), &[]).0);
            entities
                .entry(id)
                .and_modify(|v| *v = v.max(own))
                .or_insert(own);
        }
        Self {
            components,
            constraints,
            entities,
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
    pub(crate) fn of(sk: &Sketch, reached: &Coordinates) -> Self {
        Self {
            frozen: Tolerances::new(sk),
            reached: Tolerances::at(sk, reached),
        }
    }
}

/// The sketch's coordinates overlaid with `values` for the keys it has.
fn merged(sk: &Sketch, values: &Coordinates) -> Coordinates {
    let mut coords = sk.coordinates();
    for (key, value) in coords.iter_mut() {
        if let Some(v) = values.get(key) {
            *value = *v;
        }
    }
    coords
}

/// The scale of `component` at `values`, measured as [`Tolerances::at`] does.
pub(crate) fn scale_at(component: &Component, values: &Coordinates) -> f64 {
    component.frame.measure(values).0
}

/// Every coordinate `component`'s equations and its frame can read: the
/// parameters of its entities, fixed ones included, in order.
pub(crate) fn component_reads(sk: &Sketch, component: &Component) -> Vec<Param> {
    let mut reads: Vec<Param> = component_entities(sk, component)
        .into_iter()
        .flat_map(|id| entity_params(sk, id))
        .collect();
    reads.sort();
    reads.dedup();
    reads
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
                let want = extent(&sk, &merged(&sk, values), &ids, &component.sources);
                let got = component.frame.measure(values);
                assert_eq!(got.0.to_bits(), want.0.to_bits());
                assert_eq!(got.1.map(f64::to_bits), want.1.map(f64::to_bits));
            }
        }
    }
}
