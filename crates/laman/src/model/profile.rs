//! The result of profile extraction: loops, profiles, and what could not
//! close (ADR-0012).
//!
//! The types live in `model` so that `laman-check` can name them under the
//! layer rule; the extraction is `laman::profile`. Every list is ordered by
//! its smallest id.

use super::{ConstraintId, EntityId};

/// An edge as a loop traverses it.
///
/// ```
/// use laman::model::{profile::Traversal, EntityId};
/// let t = Traversal { entity: EntityId(4), reversed: true };
/// assert!(t.reversed);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Traversal {
    /// The line or arc.
    pub entity: EntityId,
    /// Traversed from its end to its start (a line from `b` to `a`, an arc
    /// clockwise) rather than as drawn.
    pub reversed: bool,
}

/// A closed boundary: edges end to end, or one circle. A line or arc shared
/// by two bounded areas is in both loops, once in each direction.
///
/// ```
/// use laman::model::profile::Loop;
/// let l = Loop { edges: vec![], signed_area: 0.0, joins: vec![] };
/// assert!(l.edges.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Loop {
    /// The directed edges in loop order, from the smallest edge id; a
    /// circle is its single edge, counter-clockwise unless reversed.
    pub edges: Vec<Traversal>,
    /// The area the loop encloses, positive counter-clockwise, in the
    /// caller's units squared; saturates at `±f64::MAX`.
    pub signed_area: f64,
    /// The `coincident` constraints that join this loop's ends, by id: those
    /// on a chain between two distinct end points of its edges, not one that
    /// only ties an end to a point no edge of the loop uses. A violated one
    /// is named too: the solve's status reports it.
    pub joins: Vec<ConstraintId>,
}

/// An outer loop, counter-clockwise, with the loops it holds as holes,
/// clockwise.
///
/// ```
/// use laman::model::profile::{Loop, Profile};
/// let outer = Loop { edges: vec![], signed_area: 4.0, joins: vec![] };
/// let p = Profile { outer, holes: vec![], net_area: 4.0 };
/// assert_eq!(p.net_area, p.outer.signed_area);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Profile {
    /// The outer boundary.
    pub outer: Loop,
    /// The holes directly inside it, ordered by smallest edge id.
    pub holes: Vec<Loop>,
    /// The outer area less the holes' areas, summed before it saturates at
    /// `±f64::MAX`: where the plate's area overflows, so does the net, with
    /// the sign of the true sum (not the 0 of two saturated terms).
    pub net_area: f64,
}

/// Two free ends that coincide in the coordinates but are not joined: the
/// join the caller probably meant is missing.
///
/// ```
/// use laman::model::{profile::Unjoined, EntityId};
/// let u = Unjoined { first: EntityId(2), second: EntityId(9) };
/// assert!(u.first < u.second);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unjoined {
    /// The smaller id of the two end points, as the edges draw them (never
    /// another point of the join).
    pub first: EntityId,
    /// The larger id of the two end points.
    pub second: EntityId,
}

/// What profile extraction finds in a sketch at some coordinates.
///
/// ```
/// use laman::model::profile::Profiles;
/// assert!(Profiles::default().profiles.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Profiles {
    /// The profiles, ordered by their outer loop's smallest edge id.
    pub profiles: Vec<Profile>,
    /// Edges that join end to end without closing, one list per chain, each
    /// in id order.
    pub open_chains: Vec<Vec<EntityId>>,
    /// Edges hanging from a component that still holds a loop.
    pub dangling: Vec<EntityId>,
    /// Bridges: edges of a branched core whose removal disconnects it, so
    /// the same area lies on both their sides. In no loop, ascending.
    pub bridges: Vec<EntityId>,
    /// Self-joined edges: those whose two ends are one join, left out. (Not
    /// the geometry that collapsed at a solution, which the diagnostics call
    /// degenerate.)
    pub degenerate: Vec<EntityId>,
    /// Free ends that coincide without a join.
    pub unjoined: Vec<Unjoined>,
}
