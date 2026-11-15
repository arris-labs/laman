//! The intersection check: [`check_intersections`] re-measures what the
//! kernel reported (ADR-0018).
//!
//! It reads the sketch, the coordinates and a reported [`Intersections`]
//! value (a plain model type) and nothing of the measuring: its own
//! distances to a line or a round, its own reading of a kind from a
//! **depth**, how far the curves pass to each other's far side near the
//! point (ADR-0018 D3), and its own parameter mapped back to a point.
//!
//! Every verdict is decided at the band of ADR-0018 D1, `L/4` to `4L`
//! ([`BAND`]), never at the kernel's `L`: a quantity the kernel holds within
//! `L` is a violation past `4L`, one it holds beyond `L` only at `L/4`, and
//! between them the verdict is undecided. Each pair is measured in the frame
//! of ADR-0018 D6: scaled by exact powers of two, so no product overflows
//! near 1e±300 and a small pair far from the origin keeps its digits.

use super::contain::to_segment;
use super::profiles::{at, value};
use super::{Missing, Xy, cross, dist, dot, sub};
use laman::model::intersection::{Intersection, Intersections, Meeting, MeetingKind};
use laman::model::tolerance::Tolerances;
use laman::model::{Coordinate, Coordinates, Entity, EntityId, Sketch};
use std::collections::BTreeSet;
use std::f64::consts::TAU;

/// The factor of ADR-0018 D1: a violation is past `BAND·L`, a quantity the
/// kernel holds beyond `L` is wrong only at `L/BAND` or below. Twice the
/// `2L` by which the kernel's move of a meeting to a curve's end shifts a
/// point or a parameter, so curvature (`L²/r`) and rounding never reach it.
/// Not a tolerance of its own: it scales with `L`.
pub(crate) const BAND: f64 = 4.0;

/// What a [`IntersectionViolation`] says is wrong. A pair is named by its
/// two ids as listed, a meeting by its index in the pair's list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntersectionViolation {
    /// A meeting's point (or an overlap's other end) lies farther than four
    /// length tolerances from one of its curves: a line between its ends, a
    /// round's circle, or an arc's nearer end where the point is past its
    /// sweep.
    OffCurve {
        /// The pair's first id.
        first: EntityId,
        /// The pair's second id.
        second: EntityId,
        /// The meeting's index in the pair.
        meeting: usize,
    },
    /// A meeting's point lies near an arc's circle but past its sweep,
    /// farther than four length tolerances from both its ends.
    OutsideSweep {
        /// The pair's first id.
        first: EntityId,
        /// The pair's second id.
        second: EntityId,
        /// The meeting's index in the pair.
        meeting: usize,
    },
    /// A parameter is outside its range (NaN is), or maps back to a point
    /// more than four length tolerances from the one it belongs to.
    Parameter {
        /// The pair's first id.
        first: EntityId,
        /// The pair's second id.
        second: EntityId,
        /// The meeting's index in the pair.
        meeting: usize,
    },
    /// The checker reads a different kind at the meeting's point, and the
    /// reading is decided: past four length tolerances of depth is a
    /// crossing, `L/4` or below a touch or an overlap.
    Kind {
        /// The pair's first id.
        first: EntityId,
        /// The pair's second id.
        second: EntityId,
        /// The meeting's index in the pair.
        meeting: usize,
    },
    /// The list breaks the order of the result: the pair's ids not
    /// ascending, pairs not ascending by `(first, second)`, meetings not
    /// ascending by their parameter on the first then the second, an
    /// overlap's `at` not the end of lower parameter on the first, or no
    /// meeting in the pair. At most once per pair.
    Order {
        /// The pair's first id.
        first: EntityId,
        /// The pair's second id.
        second: EntityId,
    },
    /// A pair names an entity that is not a non-construction line, arc or
    /// circle of the sketch, names one curve twice, or names a line no
    /// longer than a quarter of its length tolerance or a round of radius
    /// that small: a curve that meets nothing.
    NotACurve {
        /// The entity.
        entity: EntityId,
    },
    /// A pair listed more than once, in either id order.
    Repeated {
        /// The pair's first id, as listed.
        first: EntityId,
        /// The pair's second id, as listed.
        second: EntityId,
    },
}

/// The result of [`check_intersections`], in report order: pairs as listed,
/// each pair's violations in the order of its meetings.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct IntersectionReport {
    /// What disagrees with the checker's own measure.
    pub violations: Vec<IntersectionViolation>,
    /// Parameters the coordinates lacked, or held non-finite. A pair that
    /// reads one is neither confirmed nor violated.
    pub missing: Vec<Missing>,
}

impl IntersectionReport {
    /// True when nothing is violated or missing.
    ///
    /// ```
    /// use laman_check::IntersectionReport;
    /// assert!(IntersectionReport::default().is_clean());
    /// ```
    pub fn is_clean(&self) -> bool {
        self.violations.is_empty() && self.missing.is_empty()
    }
}

/// Check `found`, what intersection measuring reported for `sketch` at
/// `values`, against the checker's own measure (ADR-0018).
///
/// Per meeting: its point lies on both curves, each parameter is in range
/// and maps back to its point, and the checker's own reading of the kind
/// agrees where it is decided. Per pair: the ids name two distinct
/// non-construction curves, the list is in the order of the result, no pair
/// is listed twice. Every verdict is at the band of `L/4` to `4L` about the
/// pair's length tolerance `L` (ADR-0018 D1), so a result wrong only by
/// what rounding and the kernel's own move to an end explain passes. Any
/// `Intersections` value is an input: a stale id, a NaN or an empty pair is
/// a violation. A parameter missing from `values`, or not finite, is listed
/// as [`Missing`] and its pair is left unchecked. Never panics, never
/// errors.
///
/// ```
/// use laman::model::Sketch;
/// use laman::model::intersection::{Intersection, Intersections, Meeting, MeetingKind};
/// use laman_check::{IntersectionViolation, check_intersections};
///
/// let mut sk = Sketch::new();
/// let p = [(0.0, 0.0), (4.0, 4.0), (0.0, 4.0), (4.0, 0.0)]
///     .map(|(x, y)| sk.point(x, y))
///     .into_iter()
///     .collect::<Result<Vec<_>, _>>()?;
/// let first = sk.line(p[0], p[1])?;
/// let second = sk.line(p[2], p[3])?;
/// let mut found = Intersections {
///     pairs: vec![Intersection {
///         first: first.id(),
///         second: second.id(),
///         meetings: vec![Meeting {
///             kind: MeetingKind::Crossing,
///             at: [2.0, 2.0],
///             on_first: 0.5,
///             on_second: 0.5,
///         }],
///     }],
/// };
/// assert!(check_intersections(&sk, &sk.coordinates(), &found).is_clean());
///
/// // A meeting moved off the curves is named, with its index; its
/// // parameters no longer map back to it either.
/// found.pairs[0].meetings[0].at = [3.0, 1.0];
/// let report = check_intersections(&sk, &sk.coordinates(), &found);
/// assert_eq!(
///     report.violations[0],
///     IntersectionViolation::OffCurve { first: first.id(), second: second.id(), meeting: 0 }
/// );
/// # Ok::<(), laman::model::ModelError>(())
/// ```
pub fn check_intersections(
    sketch: &Sketch,
    values: &Coordinates,
    found: &Intersections,
) -> IntersectionReport {
    let tolerances = Tolerances::at(sketch, values);
    let mut report = IntersectionReport::default();
    let mut seen: BTreeSet<(EntityId, EntityId)> = BTreeSet::new();
    let mut previous: Option<(EntityId, EntityId)> = None;
    for pair in &found.pairs {
        let (first, second) = (pair.first, pair.second);
        let key = (first, second);
        if !seen.insert((first.min(second), first.max(second))) {
            report
                .violations
                .push(IntersectionViolation::Repeated { first, second });
            continue;
        }
        // One curve twice is NotACurve's, not an order of ids.
        let mut ordered = first <= second && !pair.meetings.is_empty();
        if previous.is_some_and(|p| key < p) {
            ordered = false;
        }
        previous = Some(previous.map_or(key, |p| p.max(key)));
        ordered &= meetings_ordered(&pair.meetings);
        if !ordered {
            report
                .violations
                .push(IntersectionViolation::Order { first, second });
        }
        check_pair(sketch, values, &tolerances, pair, &mut report);
    }
    report.missing.sort();
    report.missing.dedup();
    report
}

/// Whether the meetings ascend by parameter on the first curve, ties by the
/// second's, each overlap's `at` the end of lower parameter on the first.
fn meetings_ordered(meetings: &[Meeting]) -> bool {
    let ascending = meetings.windows(2).all(|w| {
        w[0].on_first
            .total_cmp(&w[1].on_first)
            .then(w[0].on_second.total_cmp(&w[1].on_second))
            .is_le()
    });
    let lower_end = meetings.iter().all(|m| match m.kind {
        MeetingKind::Overlap { to_first, .. } => to_first.total_cmp(&m.on_first).is_ge(),
        MeetingKind::Crossing | MeetingKind::Touch => true,
    });
    ascending && lower_end
}

/// An entity as the check reads it, in the caller's coordinates.
enum Raw {
    Line { a: Xy, b: Xy },
    Arc { c: Xy, s: Xy, e: Xy },
    Circle { c: Xy, r: f64 },
}

/// The entity's geometry, `None` when it is not a non-construction line,
/// arc or circle of the sketch, or what the values lack.
fn raw(sketch: &Sketch, values: &Coordinates, id: EntityId) -> Result<Option<Raw>, Missing> {
    if sketch.is_construction(id) {
        return Ok(None);
    }
    Ok(match sketch.entity(id) {
        Some(Entity::Line { a, b }) => Some(Raw::Line {
            a: at(values, *a)?,
            b: at(values, *b)?,
        }),
        Some(Entity::Arc { c, s, e }) => Some(Raw::Arc {
            c: at(values, *c)?,
            s: at(values, *s)?,
            e: at(values, *e)?,
        }),
        Some(Entity::Circle { c, .. }) => Some(Raw::Circle {
            c: at(values, *c)?,
            r: value(values, id, Coordinate::R)?,
        }),
        Some(Entity::Point { .. }) | None => None,
    })
}

impl Raw {
    /// The curve's points and the radius of a circle (0 otherwise).
    fn parts(&self) -> (Vec<Xy>, f64) {
        match *self {
            Raw::Line { a, b } => (vec![a, b], 0.0),
            Raw::Arc { c, s, e } => (vec![c, s, e], 0.0),
            Raw::Circle { c, r } => (vec![c], r.abs()),
        }
    }

    /// Where the frame is translated to: a line's `a`, a round's centre.
    fn origin(&self) -> Xy {
        match *self {
            Raw::Line { a, .. } => a,
            Raw::Arc { c, .. } | Raw::Circle { c, .. } => c,
        }
    }
}

/// The `k` with `|m| = f·2^k`, `f ∈ [½, 1)`; 0 for 0 or a non-finite `m`.
fn exponent(m: f64) -> i32 {
    if m.is_finite() && m != 0.0 {
        libm::frexp(m).1
    } else {
        0
    }
}

fn scaled(p: Xy, k: i32) -> Xy {
    (libm::scalbn(p.0, k), libm::scalbn(p.1, k))
}

/// The pair's frame (ADR-0018 D6): a caller's point `p` is
/// `(p·2^−coarse − origin)·2^−fine`, the coarse scale putting the largest
/// coordinate below 1, the fine one the pair's extent in `[½, 1)`.
#[derive(Debug, Clone, Copy)]
struct Frame {
    coarse: i32,
    origin: Xy,
    fine: i32,
}

impl Frame {
    fn of(first: &Raw, second: &Raw) -> Frame {
        let (mut points, mut radius) = first.parts();
        let (more, r) = second.parts();
        points.extend(more);
        radius = radius.max(r);
        let largest = points
            .iter()
            .fold(radius, |m, p| m.max(p.0.abs()).max(p.1.abs()));
        let coarse = exponent(largest);
        let origin = scaled(first.origin(), -coarse);
        let extent = points.iter().fold(libm::scalbn(radius, -coarse), |m, &p| {
            let q = sub(scaled(p, -coarse), origin);
            m.max(q.0.abs()).max(q.1.abs())
        });
        Frame {
            coarse,
            origin,
            fine: exponent(extent),
        }
    }

    fn point(&self, p: Xy) -> Xy {
        let q = sub(scaled(p, -self.coarse), self.origin);
        scaled(q, -self.fine)
    }

    fn length(&self, v: f64) -> f64 {
        libm::scalbn(libm::scalbn(v, -self.coarse), -self.fine)
    }
}

/// A round in the frame: its circle, and for an arc its counter-clockwise
/// sweep from the start's angle.
#[derive(Debug, Clone, Copy)]
struct Round {
    c: Xy,
    r: f64,
    start: f64,
    sweep: f64,
    /// An arc's start and end points; `None` for a circle.
    ends: Option<(Xy, Xy)>,
}

#[derive(Debug, Clone, Copy)]
enum Curve {
    Line { a: Xy, b: Xy },
    Round(Round),
}

impl Curve {
    fn of(raw: &Raw, f: &Frame) -> Curve {
        match *raw {
            Raw::Line { a, b } => Curve::Line {
                a: f.point(a),
                b: f.point(b),
            },
            Raw::Arc { c, s, e } => {
                let (c, s, e) = (f.point(c), f.point(s), f.point(e));
                let (u, v) = (sub(s, c), sub(e, c));
                let turn = libm::atan2(cross(u, v), dot(u, v));
                Curve::Round(Round {
                    c,
                    r: dist(c, s),
                    start: libm::atan2(u.1, u.0),
                    sweep: if turn < 0.0 { turn + TAU } else { turn },
                    ends: Some((s, e)),
                })
            }
            Raw::Circle { c, r } => Curve::Round(Round {
                c: f.point(c),
                r: f.length(r.abs()),
                start: 0.0,
                sweep: TAU,
                ends: None,
            }),
        }
    }

    /// Whether the curve is too short to meet anything: a line, or a
    /// round's radius, no longer than `l / BAND`.
    fn negligible(&self, l: f64) -> bool {
        let size = match *self {
            Curve::Line { a, b } => dist(a, b),
            Curve::Round(r) => r.r,
        };
        size <= l / BAND
    }
}

/// Whether the angle of `p` about the round's centre is in its sweep.
fn in_sweep(r: &Round, p: Xy) -> bool {
    let off = (libm::atan2(p.1 - r.c.1, p.0 - r.c.0) - r.start).rem_euclid(TAU);
    off <= r.sweep
}

/// Where a point stands against a curve, at `tol`.
enum Reach {
    On,
    Off,
    Outside,
}

fn reach(curve: &Curve, p: Xy, tol: f64) -> Reach {
    match *curve {
        Curve::Line { a, b } => {
            if to_segment(a, b, p) <= tol {
                Reach::On
            } else {
                Reach::Off
            }
        }
        Curve::Round(r) => {
            let near_circle = (dist(p, r.c) - r.r).abs() <= tol;
            if !near_circle {
                return Reach::Off;
            }
            match r.ends {
                None => Reach::On,
                Some((s, e)) => {
                    if in_sweep(&r, p) || dist(p, s) <= tol || dist(p, e) <= tol {
                        Reach::On
                    } else {
                        Reach::Outside
                    }
                }
            }
        }
    }
}

/// The point of the curve at parameter `t`: a line's `a + t(b − a)`, a
/// round's start turned by `t` times its sweep (a circle's angle `2πt`).
fn map_back(curve: &Curve, t: f64) -> Xy {
    match *curve {
        Curve::Line { a, b } => (a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1)),
        Curve::Round(r) => {
            let angle = r.start + t * r.sweep;
            (
                r.c.0 + r.r * libm::cos(angle),
                r.c.1 + r.r * libm::sin(angle),
            )
        }
    }
}

/// Whether `t` is in the curve's range: [0, 1] on a line or an arc, [0, 1)
/// on a circle, up to 2 for the far end of an overlap on a circle (a stretch
/// that wraps). NaN is outside every range.
fn in_range(curve: &Curve, t: f64, far: bool) -> bool {
    match curve {
        Curve::Round(Round { ends: None, .. }) => t >= 0.0 && t < if far { 2.0 } else { 1.0 },
        Curve::Line { .. } | Curve::Round(Round { ends: Some(_), .. }) => (0.0..=1.0).contains(&t),
    }
}

/// What the checker reads at a meeting (ADR-0018 D3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Read {
    Crossing,
    Touch,
    Overlap,
}

fn reported(kind: MeetingKind) -> Read {
    match kind {
        MeetingKind::Crossing => Read::Crossing,
        MeetingKind::Touch => Read::Touch,
        MeetingKind::Overlap { .. } => Read::Overlap,
    }
}

fn unit(v: Xy) -> Option<Xy> {
    let len = libm::hypot(v.0, v.1);
    (len > 0.0 && len.is_finite()).then(|| (v.0 / len, v.1 / len))
}

/// A round's unit tangent at `p`: perpendicular to the radius through it.
fn round_tangent(r: &Round, p: Xy) -> Option<Xy> {
    let w = sub(p, r.c);
    unit((-w.1, w.0))
}

/// A parallel pair's stretch decides its kind: an overlap past `4L`, a touch
/// at `L/4` or below, otherwise undecided.
fn by_stretch(stretch: f64, l: f64) -> Option<Read> {
    if stretch > BAND * l {
        Some(Read::Overlap)
    } else if stretch <= l / BAND {
        Some(Read::Touch)
    } else {
        None
    }
}

/// The length shared by the shorter line's projection onto the longer.
fn shared_lines((a1, b1): (Xy, Xy), (a2, b2): (Xy, Xy)) -> f64 {
    let ((a, b), (c, d)) = if dist(a1, b1) < dist(a2, b2) {
        ((a2, b2), (a1, b1))
    } else {
        ((a1, b1), (a2, b2))
    };
    let len = dist(a, b);
    let Some(w) = unit(sub(b, a)) else {
        return 0.0;
    };
    let (t, u) = (dot(sub(c, a), w), dot(sub(d, a), w));
    (t.max(u).min(len) - t.min(u).max(0.0)).max(0.0)
}

/// The longest single stretch two rounds' sweeps share, times the radius.
fn shared_sweeps(a: &Round, b: &Round) -> f64 {
    let rel = (b.start - a.start).rem_euclid(TAU);
    let over = |from: f64| (a.sweep.min(from + b.sweep) - from.max(0.0)).max(0.0);
    over(rel).max(over(rel - TAU)) * a.r
}

/// The kind the checker reads at `p`, `None` where it is undecided.
fn read(first: &Curve, second: &Curve, p: Xy, l: f64) -> Option<Read> {
    let (hi, lo) = (BAND * l, l / BAND);
    let by_depth = |depth: f64| {
        if depth > hi {
            Some(Read::Crossing)
        } else if depth <= lo {
            Some(Read::Touch)
        } else {
            None
        }
    };
    match (first, second) {
        (Curve::Line { a: a1, b: b1 }, Curve::Line { a: a2, b: b2 }) => {
            let (u, v) = (unit(sub(*b1, *a1))?, unit(sub(*b2, *a2))?);
            let (end0, end1) = if dist(*a1, *b1) < dist(*a2, *b2) {
                (*a1, *b1)
            } else {
                (*a2, *b2)
            };
            let depth = cross(u, v).abs() * dist(p, end0).max(dist(p, end1));
            if depth > hi {
                Some(Read::Crossing)
            } else if depth <= lo {
                by_stretch(shared_lines((*a1, *b1), (*a2, *b2)), l)
            } else {
                None
            }
        }
        (Curve::Line { a, b }, Curve::Round(r)) | (Curve::Round(r), Curve::Line { a, b }) => {
            let (u, v) = (unit(sub(*b, *a))?, round_tangent(r, p)?);
            let (s, c) = (cross(u, v), dot(u, v).abs());
            by_depth(r.r * s * s / (1.0 + c))
        }
        (Curve::Round(r1), Curve::Round(r2)) => {
            let (d, dr) = (dist(r1.c, r2.c), (r1.r - r2.r).abs());
            if d <= hi && dr <= hi {
                if d <= lo && dr <= lo {
                    by_stretch(shared_sweeps(r1, r2), l)
                } else {
                    None
                }
            } else {
                let (w1, w2) = (sub(p, r1.c), sub(p, r2.c));
                let phi = libm::atan2(cross(w1, w2).abs(), dot(w1, w2));
                let (s, c) = (libm::sin(phi / 2.0), libm::cos(phi / 2.0));
                let inner = 4.0 * r1.r * r2.r * s * s / (d + dr);
                let outer = 4.0 * r1.r * r2.r * c * c / (r1.r + r2.r + d);
                by_depth(inner.min(outer))
            }
        }
    }
}

/// What is wrong at one meeting.
enum Flaw {
    OffCurve,
    OutsideSweep,
    Parameter,
    Kind,
}

/// The flaws of one meeting, in the frame, at the pair's tolerance `l`.
fn flaws(first: &Curve, second: &Curve, m: &Meeting, f: &Frame, l: f64) -> Vec<Flaw> {
    let tol = BAND * l;
    let place = f.point(m.at.into());
    let far = match m.kind {
        MeetingKind::Overlap { to, .. } => Some(f.point(to.into())),
        MeetingKind::Crossing | MeetingKind::Touch => None,
    };
    let mut out = Vec::new();
    let mut geometry = None;
    for p in [Some(place), far].into_iter().flatten() {
        for curve in [first, second] {
            match reach(curve, p, tol) {
                Reach::On => {}
                Reach::Off => geometry = geometry.or(Some(Flaw::OffCurve)),
                Reach::Outside => geometry = geometry.or(Some(Flaw::OutsideSweep)),
            }
        }
    }
    let clean = geometry.is_none();
    out.extend(geometry);

    let mut params = vec![
        (first, m.on_first, place, false),
        (second, m.on_second, place, false),
    ];
    if let (
        MeetingKind::Overlap {
            to_first,
            to_second,
            ..
        },
        Some(end),
    ) = (m.kind, far)
    {
        params.push((first, to_first, end, true));
        params.push((second, to_second, end, true));
    }
    let bad = params.iter().any(|&(curve, t, p, far)| {
        !(in_range(curve, t, far) && dist(map_back(curve, t), p) <= tol)
    });
    if bad {
        out.push(Flaw::Parameter);
    }
    if clean && read(first, second, place, l).is_some_and(|r| r != reported(m.kind)) {
        out.push(Flaw::Kind);
    }
    out
}

/// The per-pair and per-meeting tests of one listed pair.
fn check_pair(
    sketch: &Sketch,
    values: &Coordinates,
    tolerances: &Tolerances,
    pair: &Intersection,
    report: &mut IntersectionReport,
) {
    let (first, second) = (pair.first, pair.second);
    if first == second {
        report
            .violations
            .push(IntersectionViolation::NotACurve { entity: first });
        return;
    }
    let (ra, rb) = (raw(sketch, values, first), raw(sketch, values, second));
    let mut skip = false;
    for (entity, r) in [(first, &ra), (second, &rb)] {
        match r {
            Ok(Some(_)) => {}
            Ok(None) => {
                report
                    .violations
                    .push(IntersectionViolation::NotACurve { entity });
                skip = true;
            }
            Err(missing) => {
                report.missing.push(*missing);
                skip = true;
            }
        }
    }
    let (Ok(Some(ra)), Ok(Some(rb)), false) = (ra, rb, skip) else {
        return;
    };
    let frame = Frame::of(&ra, &rb);
    let (ca, cb) = (Curve::of(&ra, &frame), Curve::of(&rb, &frame));
    let (la, lb) = (
        frame.length(tolerances.entity(first)),
        frame.length(tolerances.entity(second)),
    );
    for (entity, curve, l) in [(first, &ca, la), (second, &cb, lb)] {
        if curve.negligible(l) {
            report
                .violations
                .push(IntersectionViolation::NotACurve { entity });
            skip = true;
        }
    }
    if skip {
        return;
    }
    let l = la.min(lb);
    for (meeting, m) in pair.meetings.iter().enumerate() {
        for flaw in flaws(&ca, &cb, m, &frame, l) {
            report.violations.push(match flaw {
                Flaw::OffCurve => IntersectionViolation::OffCurve {
                    first,
                    second,
                    meeting,
                },
                Flaw::OutsideSweep => IntersectionViolation::OutsideSweep {
                    first,
                    second,
                    meeting,
                },
                Flaw::Parameter => IntersectionViolation::Parameter {
                    first,
                    second,
                    meeting,
                },
                Flaw::Kind => IntersectionViolation::Kind {
                    first,
                    second,
                    meeting,
                },
            });
        }
    }
}
