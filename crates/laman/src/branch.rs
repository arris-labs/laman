//! Branches: what no equation holds, observed at coordinates and compared
//! between solves (ADR-0006, ARCHITECTURE.md §Branches).
//!
//! An [`Observation`] is one quantity per subject whose sign (an arc's: the
//! turn of its tracked sweep) says which root of its equations the sketch
//! sits on. It is *known* only beyond a margin, the tolerance that makes its
//! entity degenerate or its lines meet the other root; inside it, the last
//! known branch is carried, so a crossing spread over two solves is still a
//! [`Flip`].

use crate::model::tolerance::{ANGLE, MAX_SWEEP_STEP, Tolerances};
use crate::model::{
    Constraint, ConstraintId, Coordinate, Coordinates, Entity, EntityId, Line, Point, Sketch,
};
use crate::residual::unit_vector;
use std::collections::BTreeMap;
use std::f64::consts::{PI, TAU};

/// What an observation is of: an arc or circle, or a `parallel`,
/// `perpendicular` or `angle` constraint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Subject {
    /// An arc (its tracked sweep) or a circle (its radius).
    Entity(EntityId),
    /// A pair of lines whose orientation the constraint does not hold.
    Constraint(ConstraintId),
}

/// A branch measured beyond its margin: which one, and the quantity it was
/// read from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Known {
    /// `±1` for a sign; for an arc, `⌊sweep / 2π⌋` (0 as drawn).
    pub branch: i64,
    /// The observed quantity when the branch was last known.
    pub value: f64,
}

/// One subject at one set of coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Observation {
    /// The quantity at these coordinates: `dot` of the unit directions for
    /// `parallel`, their `cross` for `perpendicular`, `cos(φ − θ)` for
    /// `angle`, a circle's radius, an arc's sweep tracked from the last
    /// observation (radians, not reduced). Non-finite where unreadable.
    pub value: f64,
    /// The branch here if beyond the margin, else the last one known
    /// before; `None` if never known.
    pub known: Option<Known>,
    /// An arc whose tracked sweep moved more than [`MAX_SWEEP_STEP`] from
    /// the observation before, read beyond its margin both times. Not a
    /// flip: a change of branch the tracking could still read, but a move
    /// too large for the half-turn it can tell apart (ADR-0006).
    pub jump: bool,
}

/// A change of branch between two observation sets.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Flip {
    /// What flipped.
    pub subject: Subject,
    /// The quantity where the old branch was last known.
    pub was: f64,
    /// The quantity on the new branch.
    pub now: f64,
}

/// Every observed subject of a sketch at some coordinates, in subject order.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Observations(BTreeMap<Subject, Observation>);

impl Observations {
    /// Observed with no history: guesses, or the first frame. Arcs' sweeps
    /// are read in `[0, 2π)`.
    pub(crate) fn at(sketch: &Sketch, values: &Coordinates) -> Self {
        observe(sketch, values, None)
    }

    /// Observed after `self`: arcs tracked from its sweeps, and a quantity
    /// inside its margin carrying `self`'s known branch.
    pub(crate) fn next(&self, sketch: &Sketch, values: &Coordinates) -> Self {
        observe(sketch, values, Some(self))
    }

    /// The flips from `self` (the earlier set) to `now`: every subject known
    /// in both whose branch differs, in subject order. A subject unknown on
    /// either side is not compared.
    pub(crate) fn flips(&self, now: &Observations) -> Vec<Flip> {
        now.0
            .iter()
            .filter_map(|(subject, o)| {
                let was = self.0.get(subject)?.known?;
                let is = o.known?;
                (was.branch != is.branch).then_some(Flip {
                    subject: *subject,
                    was: was.value,
                    now: is.value,
                })
            })
            .collect()
    }

    /// Whether any arc's tracked sweep moved by more than
    /// [`MAX_SWEEP_STEP`] from the earlier set to `self`: a drag frame this
    /// large is bisected as a flip is, without naming one (ADR-0006).
    pub(crate) fn jumped(&self) -> bool {
        self.0.values().any(|o| o.jump)
    }

    /// One subject's observation.
    #[cfg(test)]
    pub(crate) fn get(&self, subject: Subject) -> Option<&Observation> {
        self.0.get(&subject)
    }
}

/// A quantity read at coordinates, with its branch if beyond the margin.
struct Reading {
    value: f64,
    branch: Option<i64>,
    /// See [`Observation::jump`].
    jump: bool,
}

fn observe(sketch: &Sketch, values: &Coordinates, last: Option<&Observations>) -> Observations {
    let tolerances = Tolerances::at(sketch, values);
    let rd = Reader { sketch, values };
    let previous = |s: Subject| last.and_then(|l| l.0.get(&s));
    let mut out = BTreeMap::new();
    let mut record = |subject: Subject, reading: Reading| {
        let known = reading
            .branch
            .map(|branch| Known {
                branch,
                value: reading.value,
            })
            .or_else(|| previous(subject).and_then(|o| o.known));
        out.insert(
            subject,
            Observation {
                value: reading.value,
                known,
                jump: reading.jump,
            },
        );
    };
    for (id, entity) in sketch.entities() {
        let subject = Subject::Entity(id);
        let margin = tolerances.entity(id);
        match *entity {
            Entity::Point { .. } | Entity::Line { .. } => {}
            Entity::Circle { r, .. } => {
                let r = values.get(&(id, Coordinate::R)).copied().unwrap_or(r);
                record(subject, sign(r, margin));
            }
            Entity::Arc { c, s, e } => {
                let before = previous(subject).map_or(f64::NAN, |o| o.value);
                record(subject, arc(&rd, (c, s, e), before, margin));
            }
        }
    }
    for (id, constraint) in sketch.constraints() {
        let pair = |a: Line, b: Line, coeffs: (f64, f64)| lines(&rd, &tolerances, (a, b), coeffs);
        let reading = match *constraint {
            Constraint::Parallel { a, b } => pair(a, b, (0.0, 1.0)),
            Constraint::Perpendicular { a, b } => pair(a, b, (1.0, 0.0)),
            // cos(φ − θ) = dot cos θ + cross sin θ over unit directions.
            Constraint::Angle { a, b, value } => pair(a, b, (libm::sin(value), libm::cos(value))),
            Constraint::Coincident { .. }
            | Constraint::PointOnLine { .. }
            | Constraint::PointOnCircle { .. }
            | Constraint::Horizontal(_)
            | Constraint::Vertical(_)
            | Constraint::EqualLength { .. }
            | Constraint::EqualRadius { .. }
            | Constraint::TangentLineCircle { .. }
            | Constraint::TangentCircles { .. }
            | Constraint::Midpoint { .. }
            | Constraint::Symmetric { .. }
            | Constraint::Fix { .. }
            | Constraint::Distance { .. }
            | Constraint::PointLineDistance { .. }
            | Constraint::Radius { .. } => continue,
        };
        record(Subject::Constraint(id), reading);
    }
    Observations(out)
}

/// A sign known beyond `margin`.
fn sign(value: f64, margin: f64) -> Reading {
    let known = value.is_finite() && value.abs() > margin;
    Reading {
        value,
        branch: known.then_some(if value > 0.0 { 1 } else { -1 }),
        jump: false,
    }
}

/// An arc's sweep from `s` to `e` about `c`, tracked from `before` (a
/// non-finite `before` reads it in `[0, 2π)`). Known when neither the
/// radius nor the chord `|e − s|` is below `margin`, the lengths that make
/// the arc degenerate (DATA-MODEL.md §Diagnostics).
fn arc(rd: &Reader<'_>, (c, s, e): (Point, Point, Point), before: f64, margin: f64) -> Reading {
    let (pc, ps, pe) = (rd.xy(c), rd.xy(s), rd.xy(e));
    let (radius, us, _) = unit_vector(diff(pc, ps));
    let (_, ue, _) = unit_vector(diff(pc, pe));
    let (chord, _, _) = unit_vector(diff(ps, pe));
    // On unit directions, so no product of two lengths can overflow.
    let mut raw = libm::atan2(us[0] * ue[1] - us[1] * ue[0], us[0] * ue[0] + us[1] * ue[1]);
    if raw < 0.0 {
        raw += TAU;
    }
    let value = if before.is_finite() {
        // The representative of `raw` nearest the last sweep: a solve moves
        // a sweep by less than half a turn.
        let mut step = raw - before.rem_euclid(TAU);
        if step > PI {
            step -= TAU;
        } else if step <= -PI {
            step += TAU;
        }
        before + step
    } else {
        raw
    };
    let known = value.is_finite() && radius >= margin && chord >= margin;
    Reading {
        value,
        // Saturating, never a panic; a turn count is far inside i64.
        branch: known.then_some((value / TAU).floor() as i64),
        jump: known && before.is_finite() && (value - before).abs() > MAX_SWEEP_STEP,
    }
}

/// `(A cross + B dot)` of the two lines' unit directions, `coeffs = (A, B)`:
/// its sign is the pair's orientation. Known when both lines are at least
/// their degeneracy length and the quantity is beyond [`ANGLE`], the
/// angular distance at which the two roots of the constraint meet.
fn lines(
    rd: &Reader<'_>,
    tolerances: &Tolerances,
    (l1, l2): (Line, Line),
    (ca, cb): (f64, f64),
) -> Reading {
    let unit = |l: Line| {
        let [a, b] = rd.ends(l);
        let (n, u, _) = unit_vector(diff(a, b));
        (n >= tolerances.entity(l.id()), u)
    };
    let ((long1, e1), (long2, e2)) = (unit(l1), unit(l2));
    let cross = e1[0] * e2[1] - e1[1] * e2[0];
    let dot = e1[0] * e2[0] + e1[1] * e2[1];
    let value = ca * cross + cb * dot;
    let mut reading = sign(value, ANGLE);
    if !(long1 && long2) {
        reading.branch = None;
    }
    reading
}

/// Reads a point's coordinates from the values, then from the sketch;
/// NaN where neither has them.
struct Reader<'a> {
    sketch: &'a Sketch,
    values: &'a Coordinates,
}

impl Reader<'_> {
    fn xy(&self, p: Point) -> [f64; 2] {
        let id = p.id();
        let own = match self.sketch.entity(id) {
            Some(Entity::Point { x, y }) => [*x, *y],
            Some(Entity::Line { .. } | Entity::Circle { .. } | Entity::Arc { .. }) | None => {
                [f64::NAN; 2]
            }
        };
        let get = |c, o: f64| self.values.get(&(id, c)).copied().unwrap_or(o);
        [get(Coordinate::X, own[0]), get(Coordinate::Y, own[1])]
    }

    /// A line's end coordinates; NaN (never known) for an id that names no
    /// line, which a validated sketch does not hold.
    fn ends(&self, l: Line) -> [[f64; 2]; 2] {
        match self.sketch.entity(l.id()) {
            Some(Entity::Line { a, b }) => [self.xy(*a), self.xy(*b)],
            Some(Entity::Point { .. } | Entity::Circle { .. } | Entity::Arc { .. }) | None => {
                [[f64::NAN; 2]; 2]
            }
        }
    }
}

/// `to − from`.
fn diff(from: [f64; 2], to: [f64; 2]) -> [f64; 2] {
    [to[0] - from[0], to[1] - from[1]]
}

#[cfg(test)]
mod tests;
