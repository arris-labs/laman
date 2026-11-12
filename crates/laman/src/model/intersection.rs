//! The result of intersection measuring: pairs of curves and the places
//! they meet without a shared point or a declared join (ADR-0016).
//!
//! The types live in `model` so that `laman-check` can name them under the
//! layer rule; the measuring is `laman::intersect`. Every list is ordered
//! by its smallest id.

use super::EntityId;

/// How two curves meet (ADR-0016 D3).
///
/// ```
/// use laman::model::intersection::MeetingKind;
/// assert_ne!(MeetingKind::Crossing, MeetingKind::Touch);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MeetingKind {
    /// A simple root: the curves' directions differ there.
    Crossing,
    /// Tangent within the length tolerance: one point, though the curves
    /// may cross twice within it.
    Touch,
    /// A shared stretch longer than the tolerance, from the meeting's `at`
    /// to `to`: collinear lines, or concentric rounds of one radius whose
    /// sweeps share it.
    Overlap {
        /// The stretch's other end, in the caller's coordinates.
        to: [f64; 2],
        /// Its parameter on the first curve.
        to_first: f64,
        /// Its parameter on the second curve; it runs downward when the two
        /// lines point opposite ways, and past 1 on a circle whose stretch
        /// crosses angle 0.
        to_second: f64,
    },
}

/// One place where two curves meet, with its parameter on each (ADR-0016
/// D7): a line's `t` in [0, 1] from its `a`, an arc's fraction of its sweep
/// from its start in [0, 1], a circle's angle from +x over 2π in [0, 1).
///
/// ```
/// use laman::model::intersection::{Meeting, MeetingKind};
/// let m = Meeting {
///     kind: MeetingKind::Crossing,
///     at: [1.0, 2.0],
///     on_first: 0.5,
///     on_second: 0.8,
/// };
/// assert_eq!(m.at, [1.0, 2.0]);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Meeting {
    /// What kind of meeting.
    pub kind: MeetingKind,
    /// Where, in the caller's coordinates: a curve's own end when the
    /// meeting is at one (the first curve's when both have an end there); an
    /// overlap's end of lower parameter on the first curve.
    pub at: [f64; 2],
    /// Its parameter on the first curve.
    pub on_first: f64,
    /// Its parameter on the second curve.
    pub on_second: f64,
}

/// The meetings of one pair of curves, smaller id first, in order of their
/// parameter on the first, ties by the second's.
///
/// ```
/// use laman::model::{EntityId, intersection::Intersection};
/// let i = Intersection { first: EntityId(1), second: EntityId(2), meetings: vec![] };
/// assert!(i.first < i.second);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Intersection {
    /// The smaller id: a line, an arc or a circle.
    pub first: EntityId,
    /// The larger id.
    pub second: EntityId,
    /// Where they meet; never empty in a result.
    pub meetings: Vec<Meeting>,
}

/// The meetings on one curve, seen from it: see [`Intersections::along`].
///
/// ```
/// use laman::model::{EntityId, intersection::{Along, Meeting, MeetingKind}};
/// let meeting = Meeting { kind: MeetingKind::Touch, at: [0.0; 2], on_first: 1.0, on_second: 0.0 };
/// let a = Along { other: EntityId(7), meeting };
/// assert_eq!(a.meeting.on_first, 1.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Along {
    /// The curve it meets.
    pub other: EntityId,
    /// The meeting with the asked curve as its *first*: its `on_first` is
    /// the position along the asked curve.
    pub meeting: Meeting,
}

/// What intersection measuring finds in a sketch at some coordinates.
///
/// ```
/// use laman::model::intersection::Intersections;
/// assert!(Intersections::default().pairs.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Intersections {
    /// The pairs that meet, ordered by their first id, then their second's.
    pub pairs: Vec<Intersection>,
}

impl Intersections {
    /// The meetings on the curve `id`, in order of their parameter along
    /// it (ties by the other curve's id, then its parameter): each with
    /// `id` as the first curve, so `on_first` is the position along `id`
    /// whichever of the pair it was. An overlap is turned to run from its
    /// end of lower parameter on `id`. Empty for a curve that meets none.
    ///
    /// ```
    /// use laman::model::{EntityId, intersection::{Intersection, Intersections, Meeting, MeetingKind}};
    /// let found = Intersections {
    ///     pairs: vec![Intersection {
    ///         first: EntityId(1),
    ///         second: EntityId(2),
    ///         meetings: vec![Meeting {
    ///             kind: MeetingKind::Crossing,
    ///             at: [1.0, 2.0],
    ///             on_first: 0.25,
    ///             on_second: 0.75,
    ///         }],
    ///     }],
    /// };
    /// assert_eq!(found.along(EntityId(1))[0].meeting.on_first, 0.25);
    /// let seen = found.along(EntityId(2));
    /// assert_eq!((seen[0].other, seen[0].meeting.on_first), (EntityId(1), 0.75));
    /// assert!(found.along(EntityId(3)).is_empty());
    /// ```
    pub fn along(&self, id: EntityId) -> Vec<Along> {
        let mut out = Vec::new();
        for pair in &self.pairs {
            if pair.first == id {
                out.extend(pair.meetings.iter().map(|&meeting| Along {
                    other: pair.second,
                    meeting,
                }));
            } else if pair.second == id {
                out.extend(pair.meetings.iter().map(|&meeting| Along {
                    other: pair.first,
                    meeting: turned(meeting),
                }));
            }
        }
        out.sort_by(|a, b| {
            a.meeting
                .on_first
                .total_cmp(&b.meeting.on_first)
                .then(a.other.cmp(&b.other))
                .then(a.meeting.on_second.total_cmp(&b.meeting.on_second))
        });
        out
    }
}

/// The meeting seen from its second curve: parameters swapped, an overlap
/// from its end of lower parameter on the new first curve.
fn turned(m: Meeting) -> Meeting {
    match m.kind {
        MeetingKind::Overlap {
            to,
            to_first,
            to_second,
        } if to_second.total_cmp(&m.on_second).is_lt() => Meeting {
            kind: MeetingKind::Overlap {
                to: m.at,
                to_first: m.on_second,
                to_second: m.on_first,
            },
            at: to,
            on_first: to_second,
            on_second: to_first,
        },
        MeetingKind::Overlap {
            to,
            to_first,
            to_second,
        } => Meeting {
            kind: MeetingKind::Overlap {
                to,
                to_first: to_second,
                to_second: to_first,
            },
            at: m.at,
            on_first: m.on_second,
            on_second: m.on_first,
        },
        MeetingKind::Crossing | MeetingKind::Touch => Meeting {
            kind: m.kind,
            at: m.at,
            on_first: m.on_second,
            on_second: m.on_first,
        },
    }
}
