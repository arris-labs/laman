//! Profiles: closed loops and their holes, read from a sketch's edges and
//! any coordinates (ADR-0012).
//!
//! Joins are declared, never measured; coordinates give only area,
//! orientation and nesting, through the geometry kernel in `geometry`.

mod geometry;
mod join;
mod split;
pub(crate) mod topology;

use crate::model::profile::{Loop, Profile, Profiles, Traversal, Unjoined};
use crate::model::tolerance::Tolerances;
use crate::model::{Coordinate, Coordinates, Entity, EntityId, Sketch};
use geometry::{Boundary, Edge, Orientation};

/// Why profiles could not be extracted. Names the caller's id.
///
/// ```
/// use laman::model::EntityId;
/// use laman::profile::ProfileError;
/// let e = ProfileError::Missing { entity: EntityId(3) };
/// assert_eq!(e.to_string(), "entity 3 has no coordinates in the values");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ProfileError {
    /// The values lack a coordinate of a point or circle an edge uses.
    #[error("{entity} has no coordinates in the values")]
    Missing {
        /// The point or circle.
        entity: EntityId,
    },
    /// A coordinate of a point or circle an edge uses is not finite.
    #[error("{entity} has a non-finite coordinate")]
    NonFinite {
        /// The point or circle.
        entity: EntityId,
    },
}

fn value(values: &Coordinates, entity: EntityId, c: Coordinate) -> Result<f64, ProfileError> {
    match values.get(&(entity, c)) {
        None => Err(ProfileError::Missing { entity }),
        Some(v) if !v.is_finite() => Err(ProfileError::NonFinite { entity }),
        Some(&v) => Ok(v),
    }
}

fn point(values: &Coordinates, id: EntityId) -> Result<[f64; 2], ProfileError> {
    Ok([
        value(values, id, Coordinate::X)?,
        value(values, id, Coordinate::Y)?,
    ])
}

/// A loop being assembled: its traversal and its measured boundary.
#[derive(Debug)]
struct Candidate {
    edges: Vec<Traversal>,
    joins: Vec<crate::model::ConstraintId>,
    /// The circle's centre and radius, for a loop that is one circle.
    circle: Option<([f64; 2], f64)>,
    area: f64,
    orientation: Orientation,
    /// The area unsaturated, as `(twice, exponent)` for [`geometry::net`].
    exact: (f64, i32),
}

/// A bounded area: a loop with the piece it belongs to and its edges'
/// coordinates.
struct Area {
    piece: usize,
    cand: Candidate,
    shape: Vec<Edge>,
}

/// A connected part of the core with its bounded areas.
struct Piece {
    /// The probe of its outline, for containment.
    probe: Option<[f64; 2]>,
    areas: Vec<usize>,
}

impl Candidate {
    /// The same loop without its joins, to probe.
    fn clone_edges(&self) -> Candidate {
        Candidate {
            edges: self.edges.clone(),
            joins: Vec::new(),
            circle: self.circle,
            area: self.area,
            orientation: self.orientation,
            exact: self.exact,
        }
    }

    fn boundary<'a>(&self, directed: &'a [Edge]) -> Boundary<'a> {
        match self.circle {
            Some((centre, radius)) => Boundary::Circle {
                centre,
                radius,
                reversed: self.edges.first().is_some_and(|t| t.reversed),
            },
            None => Boundary::Edges(directed),
        }
    }
}

/// The edges of `traversals` with their coordinates, in loop direction.
fn directed(
    sketch: &Sketch,
    values: &Coordinates,
    traversals: &[Traversal],
) -> Result<Vec<Edge>, ProfileError> {
    let mut out = Vec::with_capacity(traversals.len());
    for t in traversals {
        match sketch.entity(t.entity) {
            Some(Entity::Line { a, b }) => {
                let (a, b) = (point(values, a.id())?, point(values, b.id())?);
                let (from, to) = if t.reversed { (b, a) } else { (a, b) };
                out.push(Edge::Line { from, to });
            }
            Some(Entity::Arc { c, s, e }) => out.push(Edge::Arc {
                centre: point(values, c.id())?,
                start: point(values, s.id())?,
                end: point(values, e.id())?,
                reversed: t.reversed,
            }),
            Some(Entity::Point { .. } | Entity::Circle { .. }) | None => {}
        }
    }
    Ok(out)
}

/// Measures one loop in the direction `traversals` give.
fn measure(
    sketch: &Sketch,
    values: &Coordinates,
    traversals: Vec<Traversal>,
    joins: Vec<crate::model::ConstraintId>,
) -> Result<Candidate, ProfileError> {
    let circle = match traversals.first().and_then(|t| sketch.entity(t.entity)) {
        Some(Entity::Circle { c, .. }) => {
            let id = traversals.first().map_or(c.id(), |t| t.entity);
            Some((point(values, c.id())?, value(values, id, Coordinate::R)?))
        }
        _ => None,
    };
    let edges = directed(sketch, values, &traversals)?;
    let mut cand = Candidate {
        edges: traversals,
        joins,
        circle,
        area: 0.0,
        orientation: Orientation::Flat,
        exact: (0.0, 0),
    };
    let a = geometry::area(&cand.boundary(&edges));
    cand.area = a.signed;
    cand.orientation = a.orientation;
    cand.exact = (a.twice, a.exponent);
    Ok(cand)
}

/// The same loop traversed the other way, from the same first edge.
fn flipped(edges: &[Traversal]) -> Vec<Traversal> {
    let flip = |t: &Traversal| Traversal {
        entity: t.entity,
        reversed: !t.reversed,
    };
    let mut out = Vec::with_capacity(edges.len());
    if let Some((first, rest)) = edges.split_first() {
        out.push(flip(first));
        out.extend(rest.iter().rev().map(flip));
    }
    out
}

/// Extracts the profiles of `sketch` at `values`: loops from its
/// non-construction lines, arcs and circles, nested by containment
/// (ADR-0012, ADR-0017).
///
/// Joins are declared, never measured: edges meet at a shared point id or
/// through a chain of `coincident` constraints. The coordinates give only
/// area, orientation, nesting and, at a join of three or more edges, the
/// order of the edges around it. Every outer loop is counter-clockwise and
/// every hole clockwise, whatever order the edges were drawn in; a loop
/// inside a hole is a profile of its own. A branched core splits at its
/// joins: every bounded area of it is a loop, an edge between two of them
/// is in both, once in each direction, and an edge with the same area on
/// both sides (a bridge) is listed in `bridges` and in no loop. Open
/// chains, dangling edges, bridges and self-joined edges are reported by
/// id in the result, never as errors; so are free ends that lie within
/// tolerance of one another without a join, which are never joined for it.
///
/// Refuses only values that lack, or hold a non-finite, coordinate of a
/// point or circle an edge uses ([`ProfileError`]). Curves that cross or
/// touch without a shared point give a deterministic result whose nesting is
/// not specified, but no loop is dropped: each is an outer boundary or a hole
/// of one.
///
/// ```
/// use laman::model::{Coordinate, Sketch};
/// use laman::profile::profiles;
///
/// let mut sk = Sketch::new();
/// let p = [(0.0, 0.0), (4.0, 0.0), (4.0, 3.0), (0.0, 3.0)]
///     .map(|(x, y)| sk.point(x, y))
///     .into_iter()
///     .collect::<Result<Vec<_>, _>>()?;
/// for i in 0..4 {
///     sk.line(p[i], p[(i + 1) % 4])?;
/// }
/// let found = profiles(&sk, &sk.coordinates())?;
/// assert_eq!(found.profiles.len(), 1);
/// assert_eq!(found.profiles[0].net_area, 12.0);
///
/// // A wall across a rectangle splits it into two profiles that share it.
/// let mut sk = Sketch::new();
/// let p = [(0.0, 0.0), (2.0, 0.0), (4.0, 0.0), (4.0, 3.0), (2.0, 3.0), (0.0, 3.0)]
///     .map(|(x, y)| sk.point(x, y))
///     .into_iter()
///     .collect::<Result<Vec<_>, _>>()?;
/// for i in 0..6 {
///     sk.line(p[i], p[(i + 1) % 6])?;
/// }
/// sk.line(p[1], p[4])?;
/// let found = profiles(&sk, &sk.coordinates())?;
/// assert_eq!(found.profiles.len(), 2);
/// assert!(found.profiles.iter().all(|p| p.net_area == 6.0));
/// assert!(found.bridges.is_empty());
///
/// // A point the values lack is named.
/// let mut values = sk.coordinates();
/// values.remove(&(p[2].id(), Coordinate::X));
/// assert!(profiles(&sk, &values).is_err());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn profiles(sketch: &Sketch, values: &Coordinates) -> Result<Profiles, ProfileError> {
    let topo = topology::topology(sketch);

    // A piece is a connected part of the core: its bounded areas, and the
    // probe of its outline for containment (ADR-0017 D5, D6). A plain loop
    // or circle is a piece whose one bounded area is itself.
    let mut areas: Vec<Area> = Vec::new();
    let mut pieces: Vec<Piece> = Vec::new();
    let mut add = |sketch: &Sketch,
                   probe_of: Option<Candidate>,
                   bounded: Vec<Candidate>|
     -> Result<(), ProfileError> {
        let piece = pieces.len();
        let probe = match &probe_of {
            Some(c) => {
                let shape = directed(sketch, values, &c.edges)?;
                geometry::probe(&c.boundary(&shape))
            }
            None => None,
        };
        let mut own = Vec::with_capacity(bounded.len());
        for cand in bounded {
            let shape = directed(sketch, values, &cand.edges)?;
            own.push(areas.len());
            areas.push(Area { piece, cand, shape });
        }
        pieces.push(Piece { probe, areas: own });
        Ok(())
    };
    for l in topo.loops {
        let cand = measure(sketch, values, l.edges, l.joins)?;
        let probe = cand.clone_edges();
        add(sketch, Some(probe), vec![cand])?;
    }
    for id in topo.circles {
        let edge = Traversal {
            entity: id,
            reversed: false,
        };
        let cand = measure(sketch, values, vec![edge], Vec::new())?;
        let probe = cand.clone_edges();
        add(sketch, Some(probe), vec![cand])?;
    }
    // Only a branched core needs the split, and its tolerances and classes
    // cost a pass over the whole sketch: not built for plain loops.
    let cut = if topo.cores.is_empty() {
        split::Split::default()
    } else {
        let tolerances = Tolerances::at(sketch, values);
        split::split(sketch, values, &tolerances, &topo.cores)?
    };
    for piece in cut.pieces {
        let mut bounded = Vec::with_capacity(piece.loops.len());
        let mut probe = None;
        for (k, l) in piece.loops.into_iter().enumerate() {
            if k == piece.outline {
                probe = Some(l.clone_edges());
            } else {
                bounded.push(l);
            }
        }
        add(sketch, probe, bounded)?;
    }
    // Pieces by their first bounded area's smallest edge id.
    let key = |p: &Piece| p.areas.first().map(|&a| split::first_key(&areas[a].cand));
    let mut order: Vec<usize> = (0..pieces.len()).collect();
    order.sort_by_key(|&p| (key(&pieces[p]), p));
    let rename: Vec<usize> = {
        let mut r = vec![0; pieces.len()];
        for (new, &old) in order.iter().enumerate() {
            r[old] = new;
        }
        r
    };
    let mut ordered: Vec<Option<Piece>> = pieces.into_iter().map(Some).collect();
    let pieces: Vec<Piece> = order
        .iter()
        .filter_map(|&old| ordered[old].take())
        .collect();
    for a in &mut areas {
        a.piece = rename[a.piece];
    }

    // Containment: a piece lies in a bounded area of another piece when
    // that area winds about its outline's probe.
    let n = pieces.len();
    let mut containers: Vec<Vec<usize>> = Vec::with_capacity(n);
    for (j, piece) in pieces.iter().enumerate() {
        let mut inside = Vec::new();
        if let Some(q) = piece.probe {
            for (i, a) in areas.iter().enumerate() {
                if a.piece != j && geometry::winding(&a.cand.boundary(&a.shape), q) != 0 {
                    inside.push(i);
                }
            }
        }
        containers.push(inside);
    }

    // A piece takes one role for all its areas: hole when the innermost area
    // it lies in (least absolute area, ties by smallest edge id) belongs to
    // an outer piece, outer boundary otherwise. Only pieces of a lower rank
    // (areas containing them, then index) count, so curves that cross
    // without a shared point still give every piece a role and none is
    // dropped.
    let rank = |p: usize| (containers[p].len(), p);
    let mut by_rank: Vec<usize> = (0..n).collect();
    by_rank.sort_by_key(|&p| rank(p));
    let mut parent: Vec<Option<usize>> = vec![None; n];
    let mut is_hole = vec![false; n];
    for &j in &by_rank {
        parent[j] = containers[j]
            .iter()
            .copied()
            .filter(|&a| rank(areas[a].piece) < rank(j))
            .min_by(|&a, &b| {
                split::compare_magnitude(&areas[a].cand, &areas[b].cand).then_with(|| {
                    split::first_key(&areas[a].cand).cmp(&split::first_key(&areas[b].cand))
                })
            });
        is_hole[j] = parent[j].is_some_and(|a| !is_hole[areas[a].piece]);
    }

    // Each area as its role wants it: counter-clockwise as an outer
    // boundary, clockwise as a hole.
    let mut finished: Vec<Option<(Loop, (f64, i32))>> = Vec::with_capacity(areas.len());
    for a in &areas {
        let l = &a.cand;
        let hole = is_hole[a.piece];
        let wrong = match l.orientation {
            Orientation::CounterClockwise => hole,
            Orientation::Clockwise => !hole,
            Orientation::Flat => false,
        };
        let (edges, area, exact) = if wrong {
            let edges = flipped(&l.edges);
            let shape = directed(sketch, values, &edges)?;
            let flip = Candidate {
                edges,
                joins: Vec::new(),
                circle: l.circle,
                area: 0.0,
                orientation: Orientation::Flat,
                exact: (0.0, 0),
            };
            let m = geometry::area(&flip.boundary(&shape));
            (flip.edges, m.signed, (m.twice, m.exponent))
        } else {
            (l.edges.clone(), l.area, l.exact)
        };
        finished.push(Some((
            Loop {
                edges,
                signed_area: area,
                joins: l.joins.clone(),
            },
            exact,
        )));
    }

    // The holes of each outer area: every bounded area of a hole piece whose
    // innermost container it is.
    let mut by_key: Vec<usize> = (0..areas.len()).collect();
    by_key.sort_by_key(|&a| split::first_key(&areas[a].cand));
    let mut holes: Vec<Vec<usize>> = vec![Vec::new(); areas.len()];
    for &a in &by_key {
        let piece = areas[a].piece;
        if let (true, Some(p)) = (is_hole[piece], parent[piece]) {
            holes[p].push(a);
        }
    }
    let mut out = Profiles::default();
    for &j in &by_key {
        if is_hole[areas[j].piece] {
            continue;
        }
        let Some((outer, exact)) = finished[j].take() else {
            continue;
        };
        let mut hs = Vec::new();
        let mut terms = vec![exact];
        for &h in &holes[j] {
            if let Some((hole, exact)) = finished[h].take() {
                terms.push(exact);
                hs.push(hole);
            }
        }
        out.profiles.push(Profile {
            outer,
            holes: hs,
            net_area: geometry::net(&terms),
        });
    }
    out.open_chains = topo.open_chains;
    out.dangling = topo.dangling;
    out.bridges = cut.bridges;
    out.degenerate = topo.degenerate;
    out.unjoined = unjoined(sketch, values, &topo.free_ends)?;
    Ok(out)
}

/// Pairs of free ends whose points lie within the smaller of their edges'
/// length tolerances (ADR-0012 D4), found by a sweep along x. Reports the
/// edges' end points, smaller id first, in id order.
fn unjoined(
    sketch: &Sketch,
    values: &Coordinates,
    ends: &[topology::FreeEnd],
) -> Result<Vec<Unjoined>, ProfileError> {
    if ends.len() < 2 {
        return Ok(Vec::new());
    }
    let tolerances = Tolerances::at(sketch, values);
    let mut at: Vec<([f64; 2], f64, EntityId)> = Vec::with_capacity(ends.len());
    for end in ends {
        at.push((
            point(values, end.point)?,
            tolerances.entity(end.edge),
            end.point,
        ));
    }
    at.sort_by(|a, b| a.0[0].total_cmp(&b.0[0]).then(a.2.cmp(&b.2)));
    let mut out = Vec::new();
    for (i, a) in at.iter().enumerate() {
        for b in at.iter().skip(i + 1) {
            // `a` is the left end, so a pair within the smaller tolerance is
            // within `a`'s: one large edge widens only its own scan.
            if b.0[0] - a.0[0] > a.1 {
                break;
            }
            let (dx, dy) = (b.0[0] - a.0[0], b.0[1] - a.0[1]);
            if libm::hypot(dx, dy) <= a.1.min(b.1) {
                out.push(Unjoined {
                    first: a.2.min(b.2),
                    second: a.2.max(b.2),
                });
            }
        }
    }
    out.sort_by_key(|u| (u.first, u.second));
    Ok(out)
}
