//! Profiles: closed loops and their holes, read from a sketch's edges and
//! any coordinates (ADR-0012).
//!
//! Joins are declared, never measured; coordinates give only area,
//! orientation and nesting, through the geometry kernel in `geometry`.

mod geometry;
mod topology;

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

impl Candidate {
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
/// non-construction lines, arcs and circles, nested by containment (ADR-0012).
///
/// Joins are declared, never measured: edges meet at a shared point id or
/// through a chain of `coincident` constraints. The coordinates give only
/// area, orientation and nesting, so a solution's `coordinates` are as good
/// an input as any others. Every outer loop is counter-clockwise and every
/// hole clockwise, whatever order the edges were drawn in; a loop inside
/// a hole is a profile of its own. Open chains, dangling edges, branched
/// cores and self-joined edges are reported by id in the result, never as
/// errors; so are free ends that lie within tolerance of one another
/// without a join, which are never joined for it.
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
/// // A point the values lack is named.
/// let mut values = sk.coordinates();
/// values.remove(&(p[2].id(), Coordinate::X));
/// assert!(profiles(&sk, &values).is_err());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn profiles(sketch: &Sketch, values: &Coordinates) -> Result<Profiles, ProfileError> {
    let topo = topology::topology(sketch);

    let mut loops: Vec<Candidate> = Vec::new();
    for l in topo.loops {
        loops.push(measure(sketch, values, l.edges, l.joins)?);
    }
    for id in topo.circles {
        let edge = Traversal {
            entity: id,
            reversed: false,
        };
        loops.push(measure(sketch, values, vec![edge], Vec::new())?);
    }
    loops.sort_by_key(|l| l.edges.first().map(|t| t.entity));

    // Containment: loop `j` lies in loop `i` when `i` winds about the
    // midpoint of `j`'s first edge.
    let n = loops.len();
    let mut shapes: Vec<Vec<Edge>> = Vec::with_capacity(n);
    for l in &loops {
        shapes.push(directed(sketch, values, &l.edges)?);
    }
    let mut containers: Vec<Vec<usize>> = Vec::with_capacity(n);
    for (j, inner) in loops.iter().enumerate() {
        let mut inside = Vec::new();
        if let Some(q) = geometry::probe(&inner.boundary(&shapes[j])) {
            for (i, outer) in loops.iter().enumerate() {
                if i != j && geometry::winding(&outer.boundary(&shapes[i]), q) != 0 {
                    inside.push(i);
                }
            }
        }
        containers.push(inside);
    }

    // A loop is a hole when its innermost container is an outer boundary,
    // and an outer boundary otherwise: for nested loops that is the parity of
    // the depth. Innermost is the container with the most containers itself
    // (the later of equals), and only loops of a lower rank (containers,
    // then index) count, so curves that cross without a shared point still
    // give every loop a role and none is dropped. Flip a loop that turns the
    // other way.
    let rank = |i: usize| (containers[i].len(), i);
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&i| rank(i));
    let mut parent: Vec<Option<usize>> = vec![None; n];
    let mut is_hole = vec![false; n];
    for &j in &order {
        parent[j] = containers[j]
            .iter()
            .copied()
            .filter(|&i| rank(i) < rank(j))
            .max_by_key(|&i| rank(i));
        is_hole[j] = parent[j].is_some_and(|p| !is_hole[p]);
    }
    let mut finished: Vec<Option<(Loop, (f64, i32))>> = Vec::with_capacity(n);
    for (j, l) in loops.iter().enumerate() {
        let hole = is_hole[j];
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
            let a = geometry::area(&flip.boundary(&shape));
            (flip.edges, a.signed, (a.twice, a.exponent))
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

    let mut out = Profiles::default();
    let mut holes: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (j, p) in parent.iter().enumerate() {
        if let (true, Some(p)) = (is_hole[j], p) {
            holes[*p].push(j);
        }
    }
    for (j, own) in holes.iter().enumerate() {
        if is_hole[j] {
            continue;
        }
        let Some((outer, exact)) = finished[j].take() else {
            continue;
        };
        let mut hs = Vec::new();
        let mut terms = vec![exact];
        for &h in own {
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
    out.branched = topo.branched;
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
