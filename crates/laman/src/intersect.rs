//! Intersections: where two curves of a sketch meet without a shared point
//! or a declared join, measured and classified, never joined (ADR-0016).
//!
//! The pair-by-pair geometry is the kernel in `kernel`; the join classes
//! are those of the profiles (ADR-0012).

mod kernel;

use crate::model::intersection::{Intersection, Intersections, Meeting, MeetingKind};
use crate::model::tolerance::Tolerances;
use crate::model::{Coordinate, Coordinates, Entity, EntityId, Sketch};
use crate::profile::topology::Classes;
use kernel::{Curve, Kind};

/// Why intersections could not be measured. Names the caller's id.
///
/// ```
/// use laman::intersect::IntersectError;
/// use laman::model::EntityId;
/// let e = IntersectError::NonFinite { entity: EntityId(3) };
/// assert_eq!(e.to_string(), "entity 3 has a non-finite coordinate");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum IntersectError {
    /// The values lack a coordinate of a point or circle a curve uses.
    #[error("{entity} has no coordinates in the values")]
    Missing {
        /// The point or circle.
        entity: EntityId,
    },
    /// A coordinate of a point or circle a curve uses is not finite.
    #[error("{entity} has a non-finite coordinate")]
    NonFinite {
        /// The point or circle.
        entity: EntityId,
    },
}

fn value(values: &Coordinates, entity: EntityId, c: Coordinate) -> Result<f64, IntersectError> {
    match values.get(&(entity, c)) {
        None => Err(IntersectError::Missing { entity }),
        Some(v) if !v.is_finite() => Err(IntersectError::NonFinite { entity }),
        Some(&v) => Ok(v),
    }
}

fn point(values: &Coordinates, id: EntityId) -> Result<[f64; 2], IntersectError> {
    Ok([
        value(values, id, Coordinate::X)?,
        value(values, id, Coordinate::Y)?,
    ])
}

/// A curve to measure: its geometry, its tolerance and the join classes of
/// its ends with the points it draws them as.
struct Item {
    id: EntityId,
    curve: Curve,
    /// The curve's length tolerance (ADR-0004).
    length: f64,
    /// Each end's join class and its point, as the curve draws it.
    ends: Vec<(EntityId, [f64; 2])>,
}

/// Measures where the non-construction lines, arcs and circles of `sketch`
/// meet at `values`, without sharing a point or a join (ADR-0016).
///
/// Each unordered pair is measured once, smaller id first, at the smaller
/// of the two curves' length tolerances. A meeting is a crossing, a touch
/// (tangent within the tolerance, one point) or an overlap (a shared stretch,
/// by its two ends), placed by its parameter on both curves; an arc's
/// meetings are bounded by its sweep. An end lying on another curve is one of
/// these, its parameter 0 or 1. A meeting at an end the two curves share, by
/// point id or through `coincident`, is that join and is not reported.
/// Nothing is changed or joined: only the caller turns an intersection into a
/// join. Pairs with no meeting are left out.
///
/// Refuses only values that lack, or hold a non-finite, coordinate of a
/// point or circle a curve uses ([`IntersectError`]). A line no longer than
/// its tolerance, or a round whose radius is within it, meets nothing. In
/// geometry smaller than the length floor everything is a touch or an
/// overlap, as the solver reads it: finite and typed, not the closed form.
///
/// ```
/// use laman::intersect::intersections;
/// use laman::model::{Sketch, intersection::MeetingKind};
///
/// let mut sk = Sketch::new();
/// let p = [(0.0, 0.0), (4.0, 4.0), (0.0, 4.0), (4.0, 0.0)]
///     .map(|(x, y)| sk.point(x, y))
///     .into_iter()
///     .collect::<Result<Vec<_>, _>>()?;
/// let first = sk.line(p[0], p[1])?;
/// let second = sk.line(p[2], p[3])?;
/// let found = intersections(&sk, &sk.coordinates())?;
/// assert_eq!(found.pairs.len(), 1);
/// let m = found.pairs[0].meetings[0];
/// assert_eq!(m.kind, MeetingKind::Crossing);
/// assert!((m.at[0] - 2.0).abs() < 1e-12 && (m.at[1] - 2.0).abs() < 1e-12);
/// assert!((m.on_first - 0.5).abs() < 1e-12 && (m.on_second - 0.5).abs() < 1e-12);
/// assert_eq!(found.along(second.id())[0].other, first.id());
///
/// // Values missing a point of a curve are refused, naming it.
/// let mut values = sk.coordinates();
/// values.remove(&(p[1].id(), laman::model::Coordinate::X));
/// assert!(intersections(&sk, &values).is_err());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn intersections(
    sketch: &Sketch,
    values: &Coordinates,
) -> Result<Intersections, IntersectError> {
    let mut classes = Classes::of(sketch);
    let mut items: Vec<Item> = Vec::new();
    for (id, e) in sketch.entities() {
        if sketch.is_construction(id) {
            continue;
        }
        let (curve, ends) = match e {
            Entity::Line { a, b } => {
                let (pa, pb) = (point(values, a.id())?, point(values, b.id())?);
                (
                    Curve::Line { a: pa, b: pb },
                    vec![(classes.find(a.id()), pa), (classes.find(b.id()), pb)],
                )
            }
            Entity::Arc { c, s, e } => {
                let (centre, start, end) = (
                    point(values, c.id())?,
                    point(values, s.id())?,
                    point(values, e.id())?,
                );
                (
                    Curve::Arc { centre, start, end },
                    vec![(classes.find(s.id()), start), (classes.find(e.id()), end)],
                )
            }
            Entity::Circle { c, .. } => (
                Curve::Circle {
                    centre: point(values, c.id())?,
                    radius: value(values, id, Coordinate::R)?,
                },
                Vec::new(),
            ),
            Entity::Point { .. } => continue,
        };
        items.push(Item {
            id,
            curve,
            length: 0.0,
            ends,
        });
    }
    let tolerances = Tolerances::at(sketch, values);
    for item in &mut items {
        item.length = tolerances.entity(item.id);
    }

    let mut out = Intersections::default();
    for (i, first) in items.iter().enumerate() {
        for second in items.iter().skip(i + 1) {
            let length = first.length.min(second.length);
            let meetings: Vec<Meeting> = kernel::meet(&first.curve, &second.curve, length)
                .into_iter()
                .filter(|m| !is_join(m, first, second, length))
                .map(|m| Meeting {
                    kind: match m.kind {
                        Kind::Crossing => MeetingKind::Crossing,
                        Kind::Touch => MeetingKind::Touch,
                        Kind::Overlap {
                            to,
                            to_first,
                            to_second,
                        } => MeetingKind::Overlap {
                            to,
                            to_first,
                            to_second,
                        },
                    },
                    at: m.at,
                    on_first: m.on_first,
                    on_second: m.on_second,
                })
                .collect();
            if !meetings.is_empty() {
                out.pairs.push(Intersection {
                    first: first.id,
                    second: second.id,
                    meetings,
                });
            }
        }
    }
    Ok(out)
}

/// Whether a crossing or a touch lies within `length` of an end the two
/// curves share by join class (ADR-0016 D5). An overlap is never a join: it
/// runs along both.
fn is_join(m: &kernel::Meeting, first: &Item, second: &Item, length: f64) -> bool {
    if !matches!(m.kind, Kind::Crossing | Kind::Touch) {
        return false;
    }
    first.ends.iter().any(|(class, p)| {
        second.ends.iter().any(|(other, q)| {
            class == other && [p, q].iter().any(|end| distance(m.at, **end) <= length)
        })
    })
}

fn distance(a: [f64; 2], b: [f64; 2]) -> f64 {
    libm::hypot(a[0] - b[0], a[1] - b[1])
}
