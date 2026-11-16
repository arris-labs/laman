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
    measure(sketch, values, true)
}

/// `intersections`, pruning the pairs by their boxes or, for the test that
/// holds the two equal, trying every pair.
fn measure(
    sketch: &Sketch,
    values: &Coordinates,
    pruned: bool,
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
    let pairs = if pruned {
        candidate_pairs(&items)
    } else {
        (0..items.len())
            .flat_map(|i| (i + 1..items.len()).map(move |j| (i, j)))
            .collect()
    };
    for (i, j) in pairs {
        let (first, second) = (&items[i], &items[j]);
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
    Ok(out)
}

/// How many of its own length tolerances a curve's box is widened by before
/// two boxes are tried against each other. A meeting lies within the pair's
/// length of both curves and the kernel's frame may round that length by a
/// factor of two; 4 holds each by twice what it needs.
const SWEEP_MARGIN: f64 = 4.0;

/// An axis-aligned box: `[min x, min y, max x, max y]`.
type Box2 = [f64; 4];

/// The box of a curve, widened by `SWEEP_MARGIN` of its length tolerance. An
/// arc takes its whole circle's box, from the larger of its two end radii.
fn bounds(item: &Item) -> Box2 {
    let grow = SWEEP_MARGIN * item.length;
    let (lo, hi) = match &item.curve {
        Curve::Line { a, b } => (
            [a[0].min(b[0]), a[1].min(b[1])],
            [a[0].max(b[0]), a[1].max(b[1])],
        ),
        Curve::Circle { centre, radius } => (
            [centre[0] - radius.abs(), centre[1] - radius.abs()],
            [centre[0] + radius.abs(), centre[1] + radius.abs()],
        ),
        Curve::Arc { centre, start, end } => {
            let r = distance(*centre, *start).max(distance(*centre, *end));
            (
                [centre[0] - r, centre[1] - r],
                [centre[0] + r, centre[1] + r],
            )
        }
    };
    [lo[0] - grow, lo[1] - grow, hi[0] + grow, hi[1] + grow]
}

/// The pairs `(i, j)`, `i < j`, of items whose widened boxes overlap, in
/// the order of all pairs: a sweep over boxes sorted by their left edge
/// (ties by index), the y extents then compared. Every pair that can meet is
/// among them, so the result of `intersections` equals trying every pair,
/// bit for bit. A box with a non-finite side (a sketch at the edge of f64)
/// is tried against every other, never dropped.
fn candidate_pairs(items: &[Item]) -> Vec<(usize, usize)> {
    let boxes: Vec<Box2> = items.iter().map(bounds).collect();
    let (sound, wild): (Vec<usize>, Vec<usize>) =
        (0..items.len()).partition(|&i| boxes[i].iter().all(|v| v.is_finite()));
    let mut order = sound;
    order.sort_by(|&a, &b| boxes[a][0].total_cmp(&boxes[b][0]).then(a.cmp(&b)));
    let mut pairs = Vec::new();
    for (at, &i) in order.iter().enumerate() {
        let a = &boxes[i];
        for &j in &order[at + 1..] {
            let b = &boxes[j];
            if b[0] > a[2] {
                // Sorted by left edge: no later box reaches back.
                break;
            }
            if b[1] <= a[3] && a[1] <= b[3] {
                pairs.push((i.min(j), i.max(j)));
            }
        }
    }
    for &w in &wild {
        for other in 0..items.len() {
            if other != w {
                pairs.push((w.min(other), w.max(other)));
            }
        }
    }
    pairs.sort_unstable();
    pairs.dedup();
    pairs
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

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// One primitive of a random sketch: a kind and the integers it is drawn
    /// from, on a grid small enough that curves meet, touch and share ends.
    fn shape() -> impl Strategy<Value = (u8, [i8; 6])> {
        (0u8..3, proptest::array::uniform6(-6i8..7))
    }

    proptest! {
        /// Pruning by boxes changes no result: the same pairs, the same
        /// bits, at scales from 1e-3 to 1e6 and shifted away from the origin.
        #[test]
        fn pruned_pairs_equal_all_pairs(
            shapes in proptest::collection::vec(shape(), 1..24),
            scale in prop::sample::select(vec![1e-3, 1.0, 1e6, 1e300]),
            shift in prop::sample::select(vec![0.0, 1e4]),
        ) {
            let mut sk = Sketch::new();
            let mut points: Vec<crate::model::Point> = Vec::new();
            let mut at = |sk: &mut Sketch, x: i8, y: i8| {
                let p = sk
                    .point(f64::from(x) * scale + shift * scale, f64::from(y) * scale)
                    .unwrap();
                points.push(p);
                p
            };
            for (kind, v) in shapes {
                let a = at(&mut sk, v[0], v[1]);
                let b = at(&mut sk, v[2], v[3]);
                match kind {
                    0 => { sk.line(a, b).unwrap(); }
                    1 => { sk.circle(a, f64::from(v[4].unsigned_abs() + 1) * scale).unwrap(); }
                    _ => {
                        let c = at(&mut sk, v[4], v[5]);
                        sk.arc(a, b, c).unwrap();
                    }
                }
            }
            let values = sk.coordinates();
            prop_assert_eq!(measure(&sk, &values, true), measure(&sk, &values, false));
        }

        /// Curves a few length tolerances from meeting, either side of the
        /// sweep's margin: parallel lines, a line beside a circle and a line
        /// ending short of another, the gap from a quarter to nine
        /// tolerances. Pruning still changes no result.
        #[test]
        fn pruned_pairs_equal_all_pairs_at_the_margin(
            gap in 0.25f64..9.0,
            scale in prop::sample::select(vec![1e-3, 1.0, 1e6, 1e300]),
            lean in -1.0f64..1.0,
        ) {
            let mut sk = Sketch::new();
            let size = 10.0 * scale;
            let eps = gap * crate::model::tolerance::length(size);
            let mut pt = |x: f64, y: f64| sk.point(x, y).unwrap();
            let (a, b) = (pt(0.0, 0.0), pt(size, 0.0));
            let (c, d) = (pt(0.0, eps), pt(size, eps * (1.0 + lean)));
            let (e, f) = (pt(2.0 * size, 0.0), pt(2.0 * size, size));
            let (g, h) = (pt(2.0 * size - eps, size / 2.0), pt(3.0 * size, size / 2.0));
            let m = pt(size / 2.0, -size - eps);
            sk.line(a, b).unwrap();
            sk.line(c, d).unwrap();
            sk.line(e, f).unwrap();
            sk.line(g, h).unwrap();
            sk.circle(m, size).unwrap();
            let values = sk.coordinates();
            prop_assert_eq!(measure(&sk, &values, true), measure(&sk, &values, false));
        }

        /// Boxes that overflow to infinity (a circle at the edge of f64)
        /// take the wild path: tried against every other curve.
        #[test]
        fn pruned_pairs_equal_all_pairs_with_non_finite_boxes(
            big in prop::sample::select(vec![1e300, 1.7e308, f64::MAX]),
            radius_big in prop::bool::ANY,
        ) {
            let mut sk = Sketch::new();
            let o = sk.point(big, big).unwrap();
            let q = sk.point(-big, 0.0).unwrap();
            let r = sk.point(0.0, 1.0).unwrap();
            let s2 = sk.point(1.0, 0.0).unwrap();
            sk.circle(o, if radius_big { big } else { 1.0 }).unwrap();
            sk.line(q, r).unwrap();
            sk.line(r, s2).unwrap();
            let values = sk.coordinates();
            prop_assert_eq!(measure(&sk, &values, true), measure(&sk, &values, false));
        }
    }
}
