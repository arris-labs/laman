//! The profile check: [`check_profiles`] re-measures what
//! extraction reported (ADR-0013).
//!
//! It reads the sketch, the coordinates and the reported [`Profiles`] value
//! (a plain model type) and nothing of the extraction: its own walk over
//! `coincident` for the joins, its own measure of a loop's area
//! (`area` module) and its own coverage count.

use super::Missing;
use super::area::{Measured, Piece, Shape, measure};
use super::contain::{Seg, Standing, standing};
use laman::model::profile::{Loop, Profiles, Traversal};
use laman::model::tolerance::Tolerances;
use laman::model::{Constraint, Coordinate, Coordinates, Entity, EntityId, Point, Sketch};
use std::collections::{BTreeMap, BTreeSet};

/// What a [`ProfileViolation`] says is wrong, with the loop it is about
/// named by its first edge's id (`None` for a loop with no edge).
#[derive(Debug, Clone, PartialEq)]
pub enum ProfileViolation {
    /// The loop's area, measured by the checker, differs from the reported
    /// one by more than the allowance (ADR-0013 D2). Both in the caller's
    /// units squared, saturated at `±f64::MAX`.
    Area {
        /// The loop's first edge.
        first: Option<EntityId>,
        /// The checker's measure.
        measured: f64,
        /// The loop's `signed_area`.
        reported: f64,
    },
    /// A profile's net area, measured by the checker as its outer loop less
    /// its holes (each by its magnitude), differs from the reported one by
    /// more than the loops' allowances summed (ADR-0013 addendum). Not
    /// compared where a loop of the profile is unmeasured, has an open join
    /// or has an arc off its radius. Both in the caller's units squared,
    /// saturated at `±f64::MAX`.
    NetArea {
        /// The profile's outer loop's first edge.
        first: Option<EntityId>,
        /// The checker's measure.
        measured: f64,
        /// The profile's `net_area`.
        reported: f64,
    },
    /// The loop turns the wrong way for its role: an outer loop clockwise or
    /// a hole counter-clockwise, by the checker's measured sign.
    Orientation {
        /// The loop's first edge.
        first: Option<EntityId>,
    },
    /// The loop is empty, or an edge's exit and the next one's entry (the
    /// last one's and the first's included) are not one join, by the
    /// checker's own walk over `coincident`, or a circle is not alone.
    NotChained {
        /// The loop's first edge.
        first: Option<EntityId>,
    },
    /// Two ends of a join lie farther apart than the loop's length
    /// tolerance. The loop's area is not compared (ADR-0013 D2).
    OpenJoin {
        /// The loop's first edge.
        first: Option<EntityId>,
    },
    /// An arc of the loop has ends at radii farther apart than the loop's
    /// length tolerance. The loop's area is not compared.
    ArcOffRadius {
        /// The loop's first edge.
        first: Option<EntityId>,
    },
    /// An entity listed as an edge (in a loop, an open chain, a dangling,
    /// bridge or degenerate list) is not a non-construction line, arc or
    /// circle of the sketch.
    NotAnEdge {
        /// The entity.
        entity: EntityId,
    },
    /// An edge is listed more than once in one direction of the loops, more
    /// than once in the other lists, or in a loop and another list (a bridge
    /// in a loop included). An edge between two bounded areas is listed
    /// once in each direction, which is not a repeat.
    Repeated {
        /// The edge.
        entity: EntityId,
    },
    /// A non-construction line, arc or circle of the sketch is in no list.
    Uncovered {
        /// The edge.
        entity: EntityId,
    },
    /// A hole is not inside its profile's outer loop, or a profile's outer
    /// loop is inside another's plate.
    Containment {
        /// The loop's first edge.
        first: Option<EntityId>,
    },
    /// An edge is listed in a class other than the one the checker derives
    /// for it by its own pruning and connectivity test (ADR-0018 D7): a
    /// bridge in a loop, a dangling edge among the open chains, an open
    /// chain that joins two derived chains or splits one.
    Misclassified {
        /// The edge.
        entity: EntityId,
    },
    /// A loop is not one bounded area: another edge of its piece (the
    /// connected part of the core less its bridges) has its probe strictly
    /// inside the loop, so the loop covers more than one area (ADR-0018 D7).
    /// Not decided where the piece's curves cross.
    NotOneArea {
        /// The loop's first edge.
        first: Option<EntityId>,
    },
}

/// The result of [`check_profiles`], in report order: loops (each profile's
/// outer, then its holes), then edges by id.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ProfileReport {
    /// What disagrees with the checker's own measure.
    pub violations: Vec<ProfileViolation>,
    /// Parameters the coordinates lacked, or held non-finite. A loop that
    /// reads one is neither confirmed nor violated.
    pub missing: Vec<Missing>,
}

impl ProfileReport {
    /// True when nothing is violated or missing.
    ///
    /// ```
    /// use laman_check::ProfileReport;
    /// assert!(ProfileReport::default().is_clean());
    /// ```
    pub fn is_clean(&self) -> bool {
        self.violations.is_empty() && self.missing.is_empty()
    }
}

/// Check `found`, what extraction reported for `sketch` at
/// `values`, against the checker's own measure (ADR-0013).
///
/// Every loop is chained by a walk over the sketch's `coincident`
/// constraints, and measured by sectors about its first point; its area and
/// orientation are compared with the report's within the allowance, except
/// where a join is open or an arc is off its radius, which are named
/// instead. A profile's net area is compared likewise, against its outer
/// loop less its holes, unless one of those loops was left uncompared.
/// Every non-construction edge of the sketch must be listed at most once
/// in each direction of the loops and in the class the checker derives for
/// it by its own pruning and connectivity test (open, dangling, bridge,
/// self-joined or in a loop), each loop must be one bounded area of its
/// piece and a hole must lie in none of its siblings of another piece
/// (ADR-0018 D7). Any `Profiles` value
/// is an input: an empty loop, a stale id or a wrong kind is a violation. A
/// parameter missing from `values`, or not finite, is listed as
/// [`Missing`] and its loop is left unmeasured. Never panics, never errors.
///
/// ```
/// use laman::model::Sketch;
/// use laman_check::check_profiles;
/// use laman::model::profile::{Loop, Profile, Profiles, Traversal};
///
/// let mut sk = Sketch::new();
/// let p = [(0.0, 0.0), (4.0, 0.0), (4.0, 3.0), (0.0, 3.0)]
///     .map(|(x, y)| sk.point(x, y))
///     .into_iter()
///     .collect::<Result<Vec<_>, _>>()?;
/// let lines = (0..4)
///     .map(|i| sk.line(p[i], p[(i + 1) % 4]))
///     .collect::<Result<Vec<_>, _>>()?;
/// let outer = Loop {
///     edges: lines.iter().map(|l| Traversal { entity: l.id(), reversed: false }).collect(),
///     signed_area: 12.0,
///     joins: vec![],
/// };
/// let found = Profiles {
///     profiles: vec![Profile { outer, holes: vec![], net_area: 12.0 }],
///     ..Profiles::default()
/// };
/// assert!(check_profiles(&sk, &sk.coordinates(), &found).is_clean());
///
/// // A wrong area is named.
/// let mut wrong = found.clone();
/// wrong.profiles[0].outer.signed_area = 13.0;
/// assert_eq!(check_profiles(&sk, &sk.coordinates(), &wrong).violations.len(), 1);
/// # Ok::<(), laman::model::ModelError>(())
/// ```
pub fn check_profiles(sketch: &Sketch, values: &Coordinates, found: &Profiles) -> ProfileReport {
    let tolerances = Tolerances::at(sketch, values);
    let classes = Classes::of(sketch);
    let split = Split::of(sketch, &classes);
    let context = Context {
        sketch,
        values,
        tolerances: &tolerances,
        classes: &classes,
        split: &split,
    };
    let mut report = ProfileReport::default();
    let mut plates = Vec::with_capacity(found.profiles.len());
    for profile in &found.profiles {
        let mut check = |l: &Loop, outer: bool| check_loop(&context, l, outer, &mut report);
        let (outer, outer_term) = check(&profile.outer, true);
        let mut terms = vec![outer_term.map(|t| (t, 1.0))];
        let mut holes = Vec::with_capacity(profile.holes.len());
        for h in &profile.holes {
            let (bound, term) = check(h, false);
            holes.push(bound);
            terms.push(term.map(|t| (t, -1.0)));
        }
        net_area(
            profile.outer.edges.first().map(|t| t.entity),
            profile.net_area,
            &terms,
            &mut report,
        );
        plates.push(Plate { outer, holes });
    }
    containment(&plates, &mut report);
    one_area(sketch, values, &split, &plates, &mut report);
    coverage(sketch, found, &split, &mut report);
    report.missing.sort();
    report.missing.dedup();
    report
}

/// The join classes of the sketch's points: the components of the graph of
/// its `coincident` constraints (ADR-0012 D1), found by the checker's own
/// union-find, the smallest id the representative.
pub(crate) struct Classes {
    parent: BTreeMap<EntityId, EntityId>,
}

impl Classes {
    pub(crate) fn of(sketch: &Sketch) -> Classes {
        let mut classes = Classes {
            parent: BTreeMap::new(),
        };
        for (_, c) in sketch.constraints() {
            if let Constraint::Coincident { p, q } = c {
                let (a, b) = (classes.find(p.id()), classes.find(q.id()));
                if a != b {
                    classes.parent.insert(a.max(b), a.min(b));
                }
            }
        }
        classes
    }

    pub(crate) fn find(&self, mut id: EntityId) -> EntityId {
        while let Some(&up) = self.parent.get(&id) {
            id = up;
        }
        id
    }

    fn same(&self, a: Point, b: Point) -> bool {
        self.find(a.id()) == self.find(b.id())
    }
}

/// An edge of the sketch as the check reads it.
enum Edge {
    Line { a: Point, b: Point },
    Arc { c: Point, s: Point, e: Point },
    Circle { c: Point },
}

fn edge_of(sketch: &Sketch, id: EntityId) -> Option<Edge> {
    if sketch.is_construction(id) {
        return None;
    }
    match sketch.entity(id)? {
        Entity::Line { a, b } => Some(Edge::Line { a: *a, b: *b }),
        Entity::Arc { c, s, e } => Some(Edge::Arc {
            c: *c,
            s: *s,
            e: *e,
        }),
        Entity::Circle { c, .. } => Some(Edge::Circle { c: *c }),
        Entity::Point { .. } => None,
    }
}

/// A parameter's value, finite or missing.
pub(crate) fn value(
    values: &Coordinates,
    id: EntityId,
    coordinate: Coordinate,
) -> Result<f64, Missing> {
    match values.get(&(id, coordinate)) {
        Some(v) if v.is_finite() => Ok(*v),
        Some(_) | None => Err(Missing {
            id,
            coordinate: Some(coordinate),
        }),
    }
}

pub(crate) fn at(values: &Coordinates, p: Point) -> Result<(f64, f64), Missing> {
    Ok((
        value(values, p.id(), Coordinate::X)?,
        value(values, p.id(), Coordinate::Y)?,
    ))
}

/// One loop, its edges resolved against the sketch.
struct Resolved {
    edges: Vec<(Traversal, Edge)>,
}

/// A loop's resolved edges, or the stale ids among them.
fn resolve(sketch: &Sketch, l: &Loop) -> Result<Resolved, Vec<EntityId>> {
    let mut edges = Vec::with_capacity(l.edges.len());
    let mut stale = Vec::new();
    for t in &l.edges {
        match edge_of(sketch, t.entity) {
            Some(e) => edges.push((*t, e)),
            None => stale.push(t.entity),
        }
    }
    if stale.is_empty() {
        Ok(Resolved { edges })
    } else {
        Err(stale)
    }
}

/// Whether the edges chain end to end into a closed loop through the
/// sketch's joins.
fn chained(classes: &Classes, edges: &[(Traversal, Edge)]) -> bool {
    match edges {
        [] => return false,
        [(_, Edge::Circle { .. })] => return true,
        _ => {}
    }
    let mut ends = Vec::with_capacity(edges.len());
    for (t, e) in edges {
        match *e {
            Edge::Line { a, b } => ends.push(if t.reversed { (b, a) } else { (a, b) }),
            Edge::Arc { s, e, .. } => ends.push(if t.reversed { (e, s) } else { (s, e) }),
            // A circle has no ends to join to.
            Edge::Circle { .. } => return false,
        }
    }
    ends.iter()
        .zip(ends.iter().cycle().skip(1))
        .all(|(here, next)| classes.same(here.1, next.0))
}

/// The loop's pieces or circle at `values`, or what it lacks.
fn shape(
    values: &Coordinates,
    edges: &[(Traversal, Edge)],
) -> Result<(Measured, Vec<Seg>), Missing> {
    if let [(t, Edge::Circle { c })] = edges {
        let c = at(values, *c)?;
        let r = value(values, t.entity, Coordinate::R)?;
        let m = measure(Shape::Circle {
            r,
            reversed: t.reversed,
        });
        let seg = Seg::Circle {
            c,
            r,
            reversed: t.reversed,
        };
        return Ok((m, vec![seg]));
    }
    let mut pieces = Vec::with_capacity(edges.len());
    for (t, e) in edges {
        pieces.push(match *e {
            Edge::Line { a, b } => {
                let (a, b) = (at(values, a)?, at(values, b)?);
                if t.reversed {
                    Piece::Line { from: b, to: a }
                } else {
                    Piece::Line { from: a, to: b }
                }
            }
            Edge::Arc { c, s, e } => Piece::Arc {
                c: at(values, c)?,
                s: at(values, s)?,
                e: at(values, e)?,
                reversed: t.reversed,
            },
            // A circle among other edges: not chained, never measured.
            Edge::Circle { .. } => {
                return Err(Missing {
                    id: t.entity,
                    coordinate: None,
                });
            }
        });
    }
    let segs = pieces
        .iter()
        .map(|p| match *p {
            Piece::Line { from, to } => Seg::Line { from, to },
            Piece::Arc { c, s, e, reversed } => Seg::Arc { c, s, e, reversed },
        })
        .collect();
    Ok((measure(Shape::Pieces(&pieces)), segs))
}

/// `a` as an area in the caller's units, saturated to `±f64::MAX`.
fn saturated(m: &Measured) -> f64 {
    libm::scalbn(m.area, m.area_exponent()).clamp(-f64::MAX, f64::MAX)
}

/// Whether the reported area agrees with the measure (ADR-0013 D3), in the
/// frame, with the extremes of the caller's units handled apart.
fn agrees(m: &Measured, reported: f64, allowance: f64) -> bool {
    if reported.is_nan() {
        return false;
    }
    let caller = libm::scalbn(m.area, m.area_exponent());
    // A saturated report agrees with a measure at or past the saturation.
    if reported.abs() == f64::MAX
        && caller.abs() >= f64::MAX
        && caller.signum() == reported.signum()
    {
        return true;
    }
    // A zero or subnormal report agrees with a measure that underflows.
    if reported.abs() < f64::MIN_POSITIVE && caller.abs() < f64::MIN_POSITIVE {
        return true;
    }
    (m.area - m.area_of(reported)).abs() <= allowance
}

/// A loop compared, as the net area reads it: its measured area and
/// allowance in its frame, whose area exponent is `exponent`.
#[derive(Clone, Copy)]
struct Term {
    area: f64,
    allowance: f64,
    exponent: i32,
}

/// Compare a profile's `reported` net area with the measure
/// `Σ sign·|area|` of its loops (outer `+1`, holes `-1`), all brought to
/// the largest loop's frame, within the allowances summed. A loop not
/// compared (`None`) leaves the net undecided. Same extremes rules as
/// [`agrees`]; never a NaN verdict.
fn net_area(
    first: Option<EntityId>,
    reported: f64,
    terms: &[Option<(Term, f64)>],
    report: &mut ProfileReport,
) {
    let Some(terms) = terms.iter().copied().collect::<Option<Vec<_>>>() else {
        return;
    };
    let Some(frame) = terms.iter().map(|(t, _)| t.exponent).max() else {
        return;
    };
    let (mut net, mut allowance) = (0.0, 0.0);
    for (t, sign) in &terms {
        let down = t.exponent.saturating_sub(frame);
        net += sign * libm::scalbn(t.area.abs(), down);
        allowance += libm::scalbn(t.allowance, down);
    }
    let caller = libm::scalbn(net, frame);
    // A saturated report agrees with a measure at or past the saturation,
    // a zero or subnormal one with a measure that underflows.
    let saturated = reported.abs() == f64::MAX
        && caller.abs() >= f64::MAX
        && caller.signum() == reported.signum();
    let underflowed = reported.abs() < f64::MIN_POSITIVE && caller.abs() < f64::MIN_POSITIVE;
    let agree = !reported.is_nan()
        && (saturated
            || underflowed
            || (net - libm::scalbn(reported, frame.saturating_neg())).abs() <= allowance);
    if !agree {
        report.violations.push(ProfileViolation::NetArea {
            first,
            measured: caller.clamp(-f64::MAX, f64::MAX),
            reported,
        });
    }
}

/// What one loop is checked against.
struct Context<'a> {
    sketch: &'a Sketch,
    values: &'a Coordinates,
    tolerances: &'a Tolerances,
    classes: &'a Classes,
    split: &'a Split,
}

fn check_loop(
    context: &Context<'_>,
    l: &Loop,
    outer: bool,
    report: &mut ProfileReport,
) -> (Option<Bound>, Option<Term>) {
    let Context {
        sketch,
        values,
        tolerances,
        classes,
        split,
    } = *context;
    let first = l.edges.first().map(|t| t.entity);
    let edges = match resolve(sketch, l) {
        Ok(r) => r.edges,
        Err(stale) => {
            for entity in stale {
                report
                    .violations
                    .push(ProfileViolation::NotAnEdge { entity });
            }
            return (None, None);
        }
    };
    if !chained(classes, &edges) {
        report
            .violations
            .push(ProfileViolation::NotChained { first });
        return (None, None);
    }
    let (m, segs) = match shape(values, &edges) {
        Ok(m) => m,
        Err(gap) => {
            report.missing.push(gap);
            return (None, None);
        }
    };
    let tolerance = edges
        .iter()
        .fold(0.0_f64, |t, (e, _)| t.max(tolerances.entity(e.entity)));
    let bound = Some(Bound {
        first,
        tolerance,
        segs,
        edges: l.edges.iter().map(|t| t.entity).collect(),
        piece: split.piece_of(l),
    });
    let slack = m.length(tolerance);
    let mut skipped = false;
    if m.gaps.iter().any(|g| *g > slack) {
        report.violations.push(ProfileViolation::OpenJoin { first });
        skipped = true;
    }
    if m.off_radius > slack {
        report
            .violations
            .push(ProfileViolation::ArcOffRadius { first });
        skipped = true;
    }
    if skipped {
        return (bound, None);
    }
    let allowance = m.allowance(tolerance);
    if !agrees(&m, l.signed_area, allowance) {
        report.violations.push(ProfileViolation::Area {
            first,
            measured: saturated(&m),
            reported: l.signed_area,
        });
    }
    // The sign is undecided within the allowance of 0.
    if m.area.abs() > allowance && (m.area > 0.0) != outer {
        report
            .violations
            .push(ProfileViolation::Orientation { first });
    }
    let term = Term {
        area: m.area,
        allowance,
        exponent: m.area_exponent(),
    };
    (bound, Some(term))
}

/// A loop the check could measure, as containment needs it.
struct Bound {
    first: Option<EntityId>,
    /// The loop's largest entity tolerance, in the caller's units.
    tolerance: f64,
    segs: Vec<Seg>,
    /// The loop's edges.
    edges: BTreeSet<EntityId>,
    /// The derived piece all its edges lie in, if they lie in one.
    piece: Option<EntityId>,
}

/// A profile's loops that could be measured, `None` for one that could not.
struct Plate {
    outer: Option<Bound>,
    holes: Vec<Option<Bound>>,
}

/// A boundary in the one frame all of containment runs in.
struct Framed {
    first: Option<EntityId>,
    piece: Option<EntityId>,
    tolerance: f64,
    segs: Vec<Seg>,
    probe: Option<(f64, f64)>,
}

fn framed(b: &Bound, k: i32) -> Framed {
    let segs: Vec<Seg> = b.segs.iter().map(|s| s.scaled(k)).collect();
    Framed {
        first: b.first,
        piece: b.piece,
        tolerance: libm::scalbn(b.tolerance, k),
        probe: segs.first().map(Seg::midpoint),
        segs,
    }
}

/// Each hole inside its own profile's outer loop, and each profile's outer
/// loop outside every other profile's plate (ADR-0013 D4). A probe within a
/// boundary's tolerance of it is undecided.
fn containment(plates: &[Plate], report: &mut ProfileReport) {
    let largest = plates
        .iter()
        .flat_map(|p| p.outer.iter().chain(p.holes.iter().flatten()))
        .flat_map(|b| &b.segs)
        .fold(0.0_f64, |m, s| m.max(s.largest()));
    let k = if largest.is_finite() && largest > 0.0 {
        -libm::frexp(largest).1
    } else {
        0
    };
    let frame: Vec<(Option<Framed>, Vec<Option<Framed>>)> = plates
        .iter()
        .map(|p| {
            (
                p.outer.as_ref().map(|b| framed(b, k)),
                p.holes
                    .iter()
                    .map(|h| h.as_ref().map(|b| framed(b, k)))
                    .collect(),
            )
        })
        .collect();
    let against = |b: &Framed, probe| standing(&b.segs, b.tolerance, probe);
    for (outer, holes) in &frame {
        let Some(outer) = outer else { continue };
        for hole in holes.iter().flatten() {
            let Some(probe) = hole.probe else { continue };
            // Outside the plate, or inside a sibling hole of another piece
            // (holes of one piece share edges; ADR-0018 D7).
            let sibling = holes.iter().flatten().any(|other| {
                hole.piece.is_some()
                    && other.piece.is_some()
                    && hole.piece != other.piece
                    && against(other, probe) == Standing::Inside
            });
            if against(outer, probe) == Standing::Outside || sibling {
                report
                    .violations
                    .push(ProfileViolation::Containment { first: hole.first });
            }
        }
    }
    for (i, (outer, _)) in frame.iter().enumerate() {
        let Some((outer, Some(probe))) = outer.as_ref().map(|o| (o, o.probe)) else {
            continue;
        };
        let inside_other = frame.iter().enumerate().any(|(j, (other, holes))| {
            let Some(other) = other else { return false };
            if i == j || against(other, probe) != Standing::Inside {
                return false;
            }
            // Inside the plate unless inside a hole; a hole not measured or
            // too near leaves it undecided.
            holes.iter().all(|h| {
                h.as_ref()
                    .is_some_and(|h| against(h, probe) == Standing::Outside)
            })
        });
        if inside_other {
            report
                .violations
                .push(ProfileViolation::Containment { first: outer.first });
        }
    }
}

/// Every edge of the sketch is listed at most once in each direction, in
/// the class the checker derives for it, and only edges are (ADR-0017,
/// ADR-0018 D7).
fn coverage(sketch: &Sketch, found: &Profiles, split: &Split, report: &mut ProfileReport) {
    // Times listed in a loop, by direction; and in the other lists.
    let mut looped: BTreeMap<EntityId, [usize; 2]> = BTreeMap::new();
    let mut other: BTreeMap<EntityId, usize> = BTreeMap::new();
    // The class each list puts an edge in.
    let mut listed_as: BTreeMap<EntityId, BTreeSet<Role>> = BTreeMap::new();
    for p in &found.profiles {
        for l in std::iter::once(&p.outer).chain(&p.holes) {
            for t in &l.edges {
                looped.entry(t.entity).or_default()[usize::from(t.reversed)] += 1;
                listed_as.entry(t.entity).or_default().insert(Role::Loop);
            }
        }
    }
    let mut listed = |id: EntityId, role: Role| {
        *other.entry(id).or_insert(0) += 1;
        listed_as.entry(id).or_default().insert(role);
    };
    found
        .open_chains
        .iter()
        .flatten()
        .for_each(|id| listed(*id, Role::Open));
    found
        .dangling
        .iter()
        .for_each(|id| listed(*id, Role::Dangling));
    found
        .bridges
        .iter()
        .for_each(|id| listed(*id, Role::Bridge));
    found
        .degenerate
        .iter()
        .for_each(|id| listed(*id, Role::SelfJoined));
    let ids: BTreeSet<EntityId> = looped.keys().chain(other.keys()).copied().collect();
    let mut wrong: BTreeSet<EntityId> = BTreeSet::new();
    for id in &ids {
        let directions = looped.get(id).copied().unwrap_or([0, 0]);
        let elsewhere = other.get(id).copied().unwrap_or(0);
        let in_loops = directions.iter().any(|&n| n > 0);
        if edge_of(sketch, *id).is_none() {
            // A loop's stale edge was already named with its loop.
            let named = report
                .violations
                .contains(&ProfileViolation::NotAnEdge { entity: *id });
            if !named {
                report
                    .violations
                    .push(ProfileViolation::NotAnEdge { entity: *id });
            }
        } else if directions.iter().any(|&n| n > 1) || elsewhere > 1 || (elsewhere > 0 && in_loops)
        {
            report
                .violations
                .push(ProfileViolation::Repeated { entity: *id });
        } else if let (Some(derived), Some(role)) = (
            split.role.get(id),
            listed_as.get(id).and_then(|r| r.iter().next()),
        ) && derived != role
        {
            wrong.insert(*id);
            report
                .violations
                .push(ProfileViolation::Misclassified { entity: *id });
        }
    }
    // A chain that is not one derived chain: its edges not yet named.
    for chain in &found.open_chains {
        let mut ids = chain.clone();
        ids.sort();
        ids.dedup();
        if !split.chains.contains(&ids) {
            for id in ids {
                let one_listing = other.get(&id) == Some(&1) && !looped.contains_key(&id);
                if split.role.contains_key(&id) && one_listing && wrong.insert(id) {
                    report
                        .violations
                        .push(ProfileViolation::Misclassified { entity: id });
                }
            }
        }
    }
    for (id, _) in sketch.entities() {
        if edge_of(sketch, id).is_some() && !ids.contains(&id) {
            report
                .violations
                .push(ProfileViolation::Uncovered { entity: id });
        }
    }
}

/// The class the checker derives for an edge (ADR-0018 D7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Role {
    /// Both ends in one join: left out.
    SelfJoined,
    /// Pruned from a component with no core.
    Open,
    /// Pruned from a component that keeps a core.
    Dangling,
    /// A core edge whose removal disconnects its core.
    Bridge,
    /// Any other core edge, or a circle: in a loop.
    Loop,
}

/// The split of the sketch's edges as the checker derives it, from the
/// sketch's joins alone: its own pruning to the 2-core, and its own
/// connectivity test for bridges, a union-find over the core's other edges
/// (not the depth-first low links extraction uses).
#[derive(Default)]
struct Split {
    role: BTreeMap<EntityId, Role>,
    /// The open chains: each component pruned away whole, its edges sorted.
    chains: BTreeSet<Vec<EntityId>>,
    /// The piece of every loop edge: the connected part of the core less
    /// its bridges, named by its smallest join; a circle is its own piece.
    piece: BTreeMap<EntityId, EntityId>,
}

/// A union-find over indices.
struct Dsu(Vec<usize>);

impl Dsu {
    fn new(n: usize) -> Dsu {
        Dsu((0..n).collect())
    }

    fn find(&mut self, mut x: usize) -> usize {
        while let Some(&up) = self.0.get(x) {
            if up == x {
                break;
            }
            let grand = self.0.get(up).copied().unwrap_or(up);
            if let Some(slot) = self.0.get_mut(x) {
                *slot = grand;
            }
            x = grand;
        }
        x
    }

    fn union(&mut self, a: usize, b: usize) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra != rb
            && let Some(slot) = self.0.get_mut(ra)
        {
            *slot = rb;
        }
    }
}

impl Split {
    fn of(sketch: &Sketch, classes: &Classes) -> Split {
        let mut out = Split::default();
        // (edge, its two join classes); circles are loops of their own.
        let mut edges: Vec<(EntityId, [EntityId; 2])> = Vec::new();
        for (id, e) in sketch.entities() {
            if sketch.is_construction(id) {
                continue;
            }
            let ends = match e {
                Entity::Line { a, b } => [a.id(), b.id()],
                Entity::Arc { s, e, .. } => [s.id(), e.id()],
                Entity::Circle { .. } => {
                    out.role.insert(id, Role::Loop);
                    out.piece.insert(id, id);
                    continue;
                }
                Entity::Point { .. } => continue,
            };
            let ends = ends.map(|p| classes.find(p));
            if ends[0] == ends[1] {
                out.role.insert(id, Role::SelfJoined);
            } else {
                edges.push((id, ends));
            }
        }
        // Join classes by index, ascending by id.
        let index: BTreeMap<EntityId, usize> = edges
            .iter()
            .flat_map(|(_, c)| *c)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .enumerate()
            .map(|(i, c)| (c, i))
            .collect();
        let at = |c: EntityId| index.get(&c).copied().unwrap_or(0);
        let ends: Vec<[usize; 2]> = edges.iter().map(|(_, c)| c.map(at)).collect();
        let mut degree = vec![0usize; index.len()];
        for e in &ends {
            for c in e {
                if let Some(d) = degree.get_mut(*c) {
                    *d += 1;
                }
            }
        }
        // Prune at degree-1 classes to the 2-core: the same set in any order.
        let mut alive = vec![true; edges.len()];
        let mut again = true;
        while again {
            again = false;
            for (i, e) in ends.iter().enumerate() {
                if alive.get(i) != Some(&true) {
                    continue;
                }
                if e.iter().any(|c| degree.get(*c) == Some(&1)) {
                    if let Some(a) = alive.get_mut(i) {
                        *a = false;
                    }
                    for c in e {
                        if let Some(d) = degree.get_mut(*c) {
                            *d -= 1;
                        }
                    }
                    again = true;
                }
            }
        }
        // Components of all the edges; pruned ones are open or dangling.
        let mut parts = Dsu::new(index.len());
        for e in &ends {
            parts.union(e[0], e[1]);
        }
        let mut core_roots: BTreeSet<usize> = BTreeSet::new();
        for (i, e) in ends.iter().enumerate() {
            if alive.get(i) == Some(&true) {
                core_roots.insert(parts.find(e[0]));
            }
        }
        let mut pruned: BTreeMap<usize, Vec<EntityId>> = BTreeMap::new();
        for (i, ((id, _), e)) in edges.iter().zip(&ends).enumerate() {
            if alive.get(i) == Some(&true) {
                continue;
            }
            let root = parts.find(e[0]);
            if core_roots.contains(&root) {
                out.role.insert(*id, Role::Dangling);
            } else {
                out.role.insert(*id, Role::Open);
                pruned.entry(root).or_default().push(*id);
            }
        }
        for mut chain in pruned.into_values() {
            chain.sort();
            out.chains.insert(chain);
        }
        // Bridges: no route between the ends through the core's other edges.
        let core: Vec<usize> = (0..edges.len())
            .filter(|&i| alive.get(i) == Some(&true))
            .collect();
        let mut bridge = vec![false; edges.len()];
        for &i in &core {
            let mut rest = Dsu::new(index.len());
            for &j in core.iter().filter(|&&j| j != i) {
                if let Some(e) = ends.get(j) {
                    rest.union(e[0], e[1]);
                }
            }
            if let Some(e) = ends.get(i)
                && rest.find(e[0]) != rest.find(e[1])
                && let Some(b) = bridge.get_mut(i)
            {
                *b = true;
            }
        }
        // Pieces: the core less its bridges, named by the smallest join.
        let mut pieces = Dsu::new(index.len());
        for &i in core.iter().filter(|&&i| bridge.get(i) != Some(&true)) {
            if let Some(e) = ends.get(i) {
                pieces.union(e[0], e[1]);
            }
        }
        let mut named: BTreeMap<usize, EntityId> = BTreeMap::new();
        for (&class, &i) in &index {
            named.entry(pieces.find(i)).or_insert(class);
        }
        for &i in &core {
            let (Some((id, _)), Some(e)) = (edges.get(i), ends.get(i)) else {
                continue;
            };
            if bridge.get(i) == Some(&true) {
                out.role.insert(*id, Role::Bridge);
            } else {
                out.role.insert(*id, Role::Loop);
                if let Some(p) = named.get(&pieces.find(e[0])) {
                    out.piece.insert(*id, *p);
                }
            }
        }
        out
    }

    /// The piece all of a loop's edges lie in; `None` if they lie in two,
    /// or an edge is not a loop edge of the derived split.
    fn piece_of(&self, l: &Loop) -> Option<EntityId> {
        let mut pieces = l.edges.iter().map(|t| self.piece.get(&t.entity));
        let first = pieces.next()??;
        pieces.all(|p| p == Some(first)).then_some(*first)
    }
}

/// An edge as a boundary piece, in the direction it is drawn.
fn seg_of(sketch: &Sketch, values: &Coordinates, id: EntityId) -> Result<Option<Seg>, Missing> {
    Ok(match edge_of(sketch, id) {
        Some(Edge::Line { a, b }) => Some(Seg::Line {
            from: at(values, a)?,
            to: at(values, b)?,
        }),
        Some(Edge::Arc { c, s, e }) => Some(Seg::Arc {
            c: at(values, c)?,
            s: at(values, s)?,
            e: at(values, e)?,
            reversed: false,
        }),
        Some(Edge::Circle { .. }) | None => None,
    })
}

/// How many points along an edge are tried against a loop.
const OUTSIDE_SAMPLES: usize = 15;

/// Each loop is one bounded area (ADR-0018 D7): no other edge of its piece
/// lies strictly inside it. The edge's probe is its middle; where a sample
/// along the edge is not strictly inside too, the edge comes within the
/// loop's tolerance of it or crosses it, which is a piece whose curves
/// cross and is not decided.
fn one_area(
    sketch: &Sketch,
    values: &Coordinates,
    split: &Split,
    plates: &[Plate],
    report: &mut ProfileReport,
) {
    for plate in plates {
        for bound in plate.outer.iter().chain(plate.holes.iter().flatten()) {
            let Some(piece) = bound.piece else { continue };
            let mut others = Vec::new();
            for (&id, &p) in &split.piece {
                if p != piece || bound.edges.contains(&id) {
                    continue;
                }
                match seg_of(sketch, values, id) {
                    Ok(Some(seg)) => others.push(seg),
                    Ok(None) => {}
                    Err(gap) => report.missing.push(gap),
                }
            }
            let largest = bound
                .segs
                .iter()
                .chain(&others)
                .fold(0.0_f64, |m, s| m.max(s.largest()));
            let k = if largest.is_finite() && largest > 0.0 {
                -libm::frexp(largest).1
            } else {
                0
            };
            let loop_segs: Vec<Seg> = bound.segs.iter().map(|s| s.scaled(k)).collect();
            let tolerance = libm::scalbn(bound.tolerance, k);
            let merged = others.iter().map(|s| s.scaled(k)).any(|seg| {
                (1..=OUTSIDE_SAMPLES).all(|n| {
                    let t = n as f64 / (OUTSIDE_SAMPLES + 1) as f64;
                    standing(&loop_segs, tolerance, seg.at(t)) == Standing::Inside
                })
            });
            if merged {
                report
                    .violations
                    .push(ProfileViolation::NotOneArea { first: bound.first });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)] // tests: a failing unwrap is the test failing (clippy.toml)

    use super::*;
    use laman::model::profile::Profile;

    /// A square plate of side `4s` with a clockwise square hole of side
    /// `2s`, the profile reporting `net` and the loops' own areas.
    fn plate(s: f64, net: f64) -> (Sketch, Profiles) {
        let mut sk = Sketch::new();
        let mut square = |corners: [(f64, f64); 4]| {
            let p: Vec<_> = corners
                .iter()
                .map(|&(x, y)| sk.point(x * s, y * s).unwrap())
                .collect();
            let edges: Vec<_> = (0..4)
                .map(|i| Traversal {
                    entity: sk.line(p[i], p[(i + 1) % 4]).unwrap().id(),
                    reversed: false,
                })
                .collect();
            edges
        };
        let outer = square([(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)]);
        let hole = square([(1.0, 1.0), (1.0, 3.0), (3.0, 3.0), (3.0, 1.0)]);
        let saturate = |a: f64| a.clamp(-f64::MAX, f64::MAX);
        let found = Profiles {
            profiles: vec![Profile {
                outer: Loop {
                    edges: outer,
                    signed_area: saturate(16.0 * s * s),
                    joins: vec![],
                },
                holes: vec![Loop {
                    edges: hole,
                    signed_area: saturate(-4.0 * s * s),
                    joins: vec![],
                }],
                net_area: net,
            }],
            ..Profiles::default()
        };
        (sk, found)
    }

    fn net_violations(s: f64, net: f64) -> Vec<ProfileViolation> {
        let (sk, found) = plate(s, net);
        let report = check_profiles(&sk, &sk.coordinates(), &found);
        assert!(report.missing.is_empty(), "{report:#?}");
        report
            .violations
            .into_iter()
            .filter(|v| matches!(v, ProfileViolation::NetArea { .. }))
            .collect()
    }

    /// The net area at the extremes of the caller's units (ADR-0013 D3): a
    /// saturated or underflowed report agrees with a measure at or past it,
    /// a wrong one is named, and no verdict is NaN.
    #[test]
    fn net_area_is_compared_in_the_frame_at_1e300() {
        // 12 s² overflows at 1e300 and underflows at 1e-300.
        assert!(net_violations(1.0, 12.0).is_empty());
        assert!(net_violations(1e300, f64::MAX).is_empty());
        assert!(net_violations(1e-300, 0.0).is_empty());
        assert!(net_violations(1e-300, 5e-324).is_empty());
        for (s, wrong) in [
            (1e300, -f64::MAX),
            (1e300, 1e300),
            (1e300, 0.0),
            (1e300, f64::NAN),
            (1e-300, 1e-300),
            (1e-300, f64::MAX),
            (1e-300, f64::NAN),
        ] {
            let found = net_violations(s, wrong);
            assert_eq!(found.len(), 1, "scale {s:e}, reported {wrong:e}");
            if let ProfileViolation::NetArea { measured, .. } = &found[0] {
                assert!(!measured.is_nan() && measured.abs() <= f64::MAX);
            }
        }
    }

    /// Where the net is a few ulps off, the allowance forgives it; a hole
    /// added instead of subtracted, or a net 0.1% off, is past it. Scales
    /// stop short of the tolerances' absolute floor, which at tiny scales
    /// swallows any area (ADR-0004).
    #[test]
    fn net_area_allowance_and_sign_hold_across_scales() {
        for s in [1e-3, 1.0, 1e5, 1e100] {
            let net = 12.0 * s * s;
            assert!(net_violations(s, net * (1.0 + 1e-14)).is_empty(), "{s:e}");
            assert_eq!(net_violations(s, 20.0 * s * s).len(), 1, "{s:e}");
            assert_eq!(net_violations(s, net * 1.001).len(), 1, "{s:e}");
        }
    }

    /// A square with a wall across it, a tail hanging off one corner, a
    /// separate open chain, and a circle: the checker's own derivation
    /// names each class (ADR-0018 D7).
    #[test]
    fn the_split_is_derived_from_the_joins() {
        let mut sk = Sketch::new();
        let p: Vec<_> = [
            (0.0, 0.0),
            (4.0, 0.0),
            (4.0, 4.0),
            (0.0, 4.0),
            (2.0, 0.0),
            (2.0, 4.0),
        ]
        .iter()
        .map(|&(x, y)| sk.point(x, y).unwrap())
        .collect();
        let tail_end = sk.point(-3.0, -3.0).unwrap();
        let far: Vec<_> = [(10.0, 10.0), (11.0, 10.0), (12.0, 10.0)]
            .iter()
            .map(|&(x, y)| sk.point(x, y).unwrap())
            .collect();
        let line = |sk: &mut Sketch, a, b| sk.line(a, b).unwrap().id();
        // Two cells, a wall between them: no bridge.
        let bottom_l = line(&mut sk, p[0], p[4]);
        let bottom_r = line(&mut sk, p[4], p[1]);
        let right = line(&mut sk, p[1], p[2]);
        let top_r = line(&mut sk, p[2], p[5]);
        let top_l = line(&mut sk, p[5], p[3]);
        let left = line(&mut sk, p[3], p[0]);
        let wall = line(&mut sk, p[4], p[5]);
        let tail = line(&mut sk, p[0], tail_end);
        let chain = [line(&mut sk, far[0], far[1]), line(&mut sk, far[1], far[2])];
        let circle = sk.circle(p[2], 1.0).unwrap().id();
        let split = Split::of(&sk, &Classes::of(&sk));
        let role = |id| split.role[&id];
        for id in [bottom_l, bottom_r, right, top_r, top_l, left, wall] {
            assert_eq!(role(id), Role::Loop);
        }
        assert_eq!(role(tail), Role::Dangling);
        assert_eq!(role(chain[0]), Role::Open);
        assert_eq!(role(circle), Role::Loop);
        assert_eq!(split.chains.len(), 1);
        assert!(split.chains.contains(chain.as_slice()));
        // One piece for the two cells.
        assert_eq!(split.piece[&wall], split.piece[&left]);
        assert_ne!(split.piece[&wall], split.piece[&circle]);

        // A second square tied to the first by one line: a bridge, two pieces.
        let q: Vec<_> = [(20.0, 0.0), (24.0, 0.0), (24.0, 4.0), (20.0, 4.0)]
            .iter()
            .map(|&(x, y)| sk.point(x, y).unwrap())
            .collect();
        let sides: Vec<_> = (0..4)
            .map(|i| line(&mut sk, q[i], q[(i + 1) % 4]))
            .collect();
        let tie = line(&mut sk, p[1], q[0]);
        let split = Split::of(&sk, &Classes::of(&sk));
        assert_eq!(split.role[&tie], Role::Bridge);
        assert_eq!(split.role[&sides[0]], Role::Loop);
        assert_ne!(split.piece[&sides[0]], split.piece[&wall]);
    }
}
