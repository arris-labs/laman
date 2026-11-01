//! Configurations: one constraint, the minimal entities it reads, and
//! coordinates drawn to land where its gradient is most likely wrong.
//!
//! A [`Variant`] is one case of `docs/DATA-MODEL.md` §Constraints that has
//! its own equations: `Span` as a line and as points, `Round` as a circle
//! and as an arc on each side of a two-round constraint, tangent external
//! and internal in each sense, line-circle in each sense. A [`Mode`] is a
//! place to draw it:
//!
//! - [`Mode::Generic`]: every point in a cell of its own, so no two are
//!   closer than a tenth of the size.
//! - [`Mode::ShortLine`]: one line (or point pair) a few millionths of the
//!   size long.
//! - [`Mode::On`] and [`Mode::NearOn`]: the constraint's own equation
//!   satisfied by construction (a point on the line, tangent circles in
//!   contact), and the same off by [`NEAR`] of the size.
//! - [`Mode::Shared`]: entities share a point instead of owning one each.
//! - [`Mode::NearParallel`]: the second line turned from the first by a
//!   thousandth of a radian or less.
//! - [`Mode::NearEqualRadius`]: radii closer than a thousandth of the size.
//!
//! # The scale it keeps
//!
//! The finite-difference rule is sound while the coordinates are at most
//! `1e6` times the equation's local scale (`laman_debug::fd`). Every
//! configuration keeps its coordinates within `4 ×` [`Configuration::size`]
//! and every length a local scale counts (a line, a radius, the distance of
//! two points the constraint names or of a round's centre) at least
//! [`MIN_LENGTH`] of the size, a short line being [`SHORT`] of it. A point
//! shared between two entities is one point, never a pair, and the end of
//! an arc is read by its own equation only through the radius it shares
//! with the start, so it is not counted.
#![allow(clippy::expect_used)] // a refusal here is this strategy's bug, not an input's

use std::f64::consts::{PI, TAU};

use std::collections::BTreeSet;

use laman::model::{
    Constraint, ConstraintId, ConstraintKind, Contact, Entity, EntityId, Line, ModelError, Point,
    Round, Sense, Sketch, Span,
};
use proptest::array::{uniform8, uniform32};
use proptest::prelude::*;
use proptest::sample::select;

/// How short a [`Mode::ShortLine`] line is, as a fraction of the size, at
/// its shortest (it is drawn up to twice this). Four times [`MIN_LENGTH`]
/// because the coordinates reach `4 ×` the size.
pub const SHORT: f64 = 4.0e-6;

/// The least a length a local scale counts may be, as a fraction of
/// [`Configuration::size`]: the bound the rule of `laman_debug::fd` needs.
pub const MIN_LENGTH: f64 = 1.0e-6;

/// How far [`Mode::NearOn`] is off the satisfied position, as a fraction of
/// the size.
pub const NEAR: f64 = 1.0e-9;

/// Whether a round is a circle or an arc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Curve {
    /// A circle.
    Circle,
    /// An arc.
    Arc,
}

/// Whether a `horizontal` or `vertical` names a line or two points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Reach {
    /// A line segment.
    Line,
    /// Two points.
    Points,
}

/// One constraint case with its own equations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Variant {
    /// `coincident`.
    Coincident,
    /// `point_on_line`.
    PointOnLine,
    /// `point_on_circle` on a circle or an arc.
    PointOnCircle(Curve),
    /// `horizontal` over a line or two points.
    Horizontal(Reach),
    /// `vertical` over a line or two points.
    Vertical(Reach),
    /// `parallel`.
    Parallel,
    /// `perpendicular`.
    Perpendicular,
    /// `equal_length`.
    EqualLength,
    /// `equal_radius` between the two kinds of round.
    EqualRadius(Curve, Curve),
    /// `tangent` of a line and a round, the centre on the sense's side.
    TangentLineCircle(Curve, Sense),
    /// `tangent` of two rounds, in a contact.
    TangentCircles(Curve, Curve, Contact),
    /// `midpoint`.
    Midpoint,
    /// `symmetric`.
    Symmetric,
    /// `fix`.
    Fix,
    /// `distance`.
    Distance,
    /// `point_line_distance` on a side.
    PointLineDistance(Sense),
    /// `radius` of a circle or an arc.
    Radius(Curve),
    /// `angle`.
    Angle,
}

/// A place to draw a [`Variant`]; the module doc lists them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Mode {
    /// Every point in its own cell.
    Generic,
    /// A line, or point pair, [`SHORT`] of the size long.
    ShortLine,
    /// The constraint's equation satisfied.
    On,
    /// The constraint's equation off by [`NEAR`] of the size.
    NearOn,
    /// Entities share a point.
    Shared,
    /// Two lines nearly parallel (or antiparallel).
    NearParallel,
    /// Two radii nearly equal.
    NearEqualRadius,
}

const CURVES: [Curve; 2] = [Curve::Circle, Curve::Arc];
const SENSES: [Sense; 2] = [Sense::Positive, Sense::Negative];

/// Every variant, in a fixed order: 40 of them.
///
/// ```
/// use laman_debug::strategy::configuration::variants;
/// assert!(variants().len() >= 30);
/// ```
pub fn variants() -> Vec<Variant> {
    let contacts = [
        Contact::External,
        Contact::Internal(Sense::Positive),
        Contact::Internal(Sense::Negative),
    ];
    let mut all = vec![Variant::Coincident, Variant::PointOnLine];
    all.extend(CURVES.map(Variant::PointOnCircle));
    for reach in [Reach::Line, Reach::Points] {
        all.push(Variant::Horizontal(reach));
        all.push(Variant::Vertical(reach));
    }
    all.extend([
        Variant::Parallel,
        Variant::Perpendicular,
        Variant::EqualLength,
    ]);
    for a in CURVES {
        for b in CURVES {
            all.push(Variant::EqualRadius(a, b));
        }
    }
    for a in CURVES {
        for sense in SENSES {
            all.push(Variant::TangentLineCircle(a, sense));
        }
    }
    for contact in contacts {
        for a in CURVES {
            for b in CURVES {
                all.push(Variant::TangentCircles(a, b, contact));
            }
        }
    }
    all.extend([
        Variant::Midpoint,
        Variant::Symmetric,
        Variant::Fix,
        Variant::Distance,
    ]);
    all.extend(SENSES.map(Variant::PointLineDistance));
    all.extend(CURVES.map(Variant::Radius));
    all.push(Variant::Angle);
    all
}

impl Variant {
    /// The kind of constraint the variant builds.
    ///
    /// ```
    /// use laman::model::ConstraintKind;
    /// use laman_debug::strategy::configuration::Variant;
    /// assert_eq!(Variant::Angle.kind(), ConstraintKind::Angle);
    /// ```
    pub fn kind(self) -> ConstraintKind {
        match self {
            Variant::Coincident => ConstraintKind::Coincident,
            Variant::PointOnLine => ConstraintKind::PointOnLine,
            Variant::PointOnCircle(_) => ConstraintKind::PointOnCircle,
            Variant::Horizontal(_) => ConstraintKind::Horizontal,
            Variant::Vertical(_) => ConstraintKind::Vertical,
            Variant::Parallel => ConstraintKind::Parallel,
            Variant::Perpendicular => ConstraintKind::Perpendicular,
            Variant::EqualLength => ConstraintKind::EqualLength,
            Variant::EqualRadius(..) => ConstraintKind::EqualRadius,
            Variant::TangentLineCircle(..) => ConstraintKind::TangentLineCircle,
            Variant::TangentCircles(..) => ConstraintKind::TangentCircles,
            Variant::Midpoint => ConstraintKind::Midpoint,
            Variant::Symmetric => ConstraintKind::Symmetric,
            Variant::Fix => ConstraintKind::Fix,
            Variant::Distance => ConstraintKind::Distance,
            Variant::PointLineDistance(_) => ConstraintKind::PointLineDistance,
            Variant::Radius(_) => ConstraintKind::Radius,
            Variant::Angle => ConstraintKind::Angle,
        }
    }

    /// A name that tells variants apart, for a test to list.
    ///
    /// ```
    /// use laman_debug::strategy::configuration::{Curve, Variant};
    /// assert_eq!(Variant::Radius(Curve::Arc).name(), "radius(Arc)");
    /// ```
    pub fn name(self) -> String {
        match self {
            Variant::Coincident => "coincident".into(),
            Variant::PointOnLine => "point_on_line".into(),
            Variant::PointOnCircle(c) => format!("point_on_circle({c:?})"),
            Variant::Horizontal(r) => format!("horizontal({r:?})"),
            Variant::Vertical(r) => format!("vertical({r:?})"),
            Variant::Parallel => "parallel".into(),
            Variant::Perpendicular => "perpendicular".into(),
            Variant::EqualLength => "equal_length".into(),
            Variant::EqualRadius(a, b) => format!("equal_radius({a:?}, {b:?})"),
            Variant::TangentLineCircle(c, s) => format!("tangent_line({c:?}, {s:?})"),
            Variant::TangentCircles(a, b, c) => format!("tangent_circles({a:?}, {b:?}, {c:?})"),
            Variant::Midpoint => "midpoint".into(),
            Variant::Symmetric => "symmetric".into(),
            Variant::Fix => "fix".into(),
            Variant::Distance => "distance".into(),
            Variant::PointLineDistance(s) => format!("point_line_distance({s:?})"),
            Variant::Radius(c) => format!("radius({c:?})"),
            Variant::Angle => "angle".into(),
        }
    }

    /// The modes the variant can be drawn in, [`Mode::Generic`] first.
    ///
    /// ```
    /// use laman_debug::strategy::configuration::{Mode, Variant};
    /// assert!(Variant::Angle.modes().contains(&Mode::NearParallel));
    /// ```
    pub fn modes(self) -> Vec<Mode> {
        use Mode::{Generic, NearEqualRadius, NearOn, NearParallel, On, Shared, ShortLine};
        match self {
            Variant::Coincident | Variant::Fix => vec![Generic],
            Variant::PointOnLine
            | Variant::Midpoint
            | Variant::Symmetric
            | Variant::PointLineDistance(_)
            | Variant::TangentLineCircle(..) => vec![Generic, ShortLine, On, NearOn, Shared],
            Variant::PointOnCircle(Curve::Circle) | Variant::Radius(_) => {
                vec![Generic, On, NearOn]
            }
            Variant::PointOnCircle(Curve::Arc) => vec![Generic, On, NearOn, Shared],
            Variant::Horizontal(_) | Variant::Vertical(_) => vec![Generic, ShortLine],
            Variant::Parallel | Variant::Perpendicular | Variant::Angle => {
                vec![Generic, ShortLine, NearParallel, Shared]
            }
            Variant::EqualLength => vec![Generic, ShortLine, Shared],
            Variant::EqualRadius(Curve::Arc, Curve::Arc) => {
                vec![Generic, NearEqualRadius, Shared]
            }
            Variant::EqualRadius(..) => vec![Generic, NearEqualRadius],
            Variant::TangentCircles(Curve::Circle, Curve::Circle, _) => {
                vec![Generic, NearEqualRadius, On, NearOn]
            }
            Variant::TangentCircles(..) => vec![Generic, NearEqualRadius, On, NearOn, Shared],
            Variant::Distance => vec![Generic, ShortLine, On, NearOn],
        }
    }
}

/// One constraint in a sketch of just the entities it reads.
#[derive(Debug, Clone)]
pub struct Configuration {
    /// The case drawn.
    pub variant: Variant,
    /// Where it was drawn.
    pub mode: Mode,
    /// The sketch: the constraint's entities and nothing else.
    pub sketch: Sketch,
    /// The constraint.
    pub constraint: ConstraintId,
    /// The sketch's size: the largest coordinate magnitude, radii included.
    pub size: f64,
}

impl Configuration {
    /// The lengths a local scale counts: the lines' and the radii of the
    /// entities the constraint names, and the distances between the points
    /// among them and the rounds' centres (a shared point is one point). An
    /// arc's radius is its start's distance from its centre.
    ///
    /// ```
    /// use laman_debug::strategy::configuration::{configuration, Variant};
    /// use proptest::prelude::*;
    /// use proptest::strategy::ValueTree;
    /// use proptest::test_runner::TestRunner;
    ///
    /// let mut runner = TestRunner::deterministic();
    /// let c = configuration(Variant::Distance).new_tree(&mut runner).unwrap().current();
    /// assert!(c.lengths().iter().all(|l| *l > 0.0));
    /// ```
    pub fn lengths(&self) -> Vec<f64> {
        let sk = &self.sketch;
        let xy = |id: EntityId| match sk.entity(id) {
            Some(Entity::Point { x, y }) => Some([*x, *y]),
            Some(Entity::Line { .. } | Entity::Circle { .. } | Entity::Arc { .. }) | None => None,
        };
        let apart = |a: EntityId, b: EntityId| {
            let (p, q) = (xy(a)?, xy(b)?);
            Some(length(sub(p, q)))
        };
        let mut points = BTreeSet::new();
        let mut out = Vec::new();
        let named = sk
            .constraint(self.constraint)
            .map(|c| c.entities())
            .unwrap_or_default();
        for id in named {
            match sk.entity(id) {
                Some(Entity::Point { .. }) => {
                    points.insert(id);
                }
                Some(Entity::Line { a, b }) => {
                    points.insert(a.id());
                    points.insert(b.id());
                    out.extend(apart(a.id(), b.id()));
                }
                Some(Entity::Circle { c, r }) => {
                    points.insert(c.id());
                    out.push(*r);
                }
                Some(Entity::Arc { c, s, .. }) => {
                    points.insert(c.id());
                    out.extend(apart(c.id(), s.id()));
                }
                None => {}
            }
        }
        let points: Vec<_> = points.into_iter().collect();
        for (i, a) in points.iter().enumerate() {
            for b in points.iter().skip(i + 1) {
                out.extend(apart(*a, *b));
            }
        }
        out
    }

    /// The equations' local scale (`laman_debug::fd`): the shortest of
    /// [`lengths`](Self::lengths), capped by the size.
    ///
    /// ```
    /// use laman_debug::strategy::configuration::{configuration, Variant};
    /// use proptest::prelude::*;
    /// use proptest::strategy::ValueTree;
    /// use proptest::test_runner::TestRunner;
    ///
    /// let mut runner = TestRunner::deterministic();
    /// let c = configuration(Variant::Fix).new_tree(&mut runner).unwrap().current();
    /// assert!(c.local_scale() <= c.size);
    /// ```
    pub fn local_scale(&self) -> f64 {
        self.lengths().into_iter().fold(self.size, f64::min)
    }
}

type Pt = (Point, [f64; 2]);

fn add(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] + b[0], a[1] + b[1]]
}

fn sub(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}

fn scale(a: [f64; 2], k: f64) -> [f64; 2] {
    [a[0] * k, a[1] * k]
}

fn dir(angle: f64) -> [f64; 2] {
    [libm::cos(angle), libm::sin(angle)]
}

fn length(a: [f64; 2]) -> f64 {
    libm::hypot(a[0], a[1])
}

fn heading(a: [f64; 2]) -> f64 {
    libm::atan2(a[1], a[0])
}

/// The raw draw a configuration is built from; every field shrinks toward
/// a valid, plain value.
#[derive(Debug, Clone)]
struct Draw {
    size: f64,
    cells: Vec<usize>,
    jitter: [f64; 32],
    q: [f64; 8],
}

fn draw() -> impl Strategy<Value = Draw> {
    (
        select(vec![1.0e-3, 1.0, 1.0e3]),
        1.0..10.0_f64,
        Just((0..16_usize).collect::<Vec<_>>()).prop_shuffle(),
        uniform32(0.0..1.0_f64),
        uniform8(0.0..1.0_f64),
    )
        .prop_map(|(base, mult, cells, jitter, q)| Draw {
            size: base * mult,
            cells,
            jitter,
            q,
        })
}

struct Rd {
    round: Round,
    s: Option<Pt>,
    e: Option<Pt>,
}

struct Build {
    sk: Sketch,
    d: Draw,
    next: usize,
}

impl Build {
    fn point(&mut self, xy: [f64; 2]) -> Result<Pt, ModelError> {
        Ok((self.sk.point(xy[0], xy[1])?, xy))
    }

    /// A point in the next unused cell of the 4×4 grid over the size.
    fn free(&mut self) -> Result<Pt, ModelError> {
        let (n, w) = (self.next, self.d.size / 2.0);
        self.next += 1;
        let cell = self.d.cells.get(n).copied().unwrap_or(n);
        let at = |i: usize, j: f64| (-2.0 + (i as f64 + 0.5) * 1.0) * w + (j - 0.5) * 0.8 * w;
        let jx = self.d.jitter.get(2 * n).copied().unwrap_or(0.5);
        let jy = self.d.jitter.get(2 * n + 1).copied().unwrap_or(0.5);
        self.point([at(cell % 4, jx), at(cell / 4, jy)])
    }

    /// A vector of a short line's length, pointing by the draw.
    fn short(&self) -> [f64; 2] {
        let q = self.d.q;
        scale(dir(TAU * q[6]), SHORT * self.d.size * (1.0 + q[5]))
    }

    /// A line. `fixed` pins one end (0 start, 1 end) to a point; the other
    /// is free, or a short step from it.
    fn line(
        &mut self,
        short: bool,
        fixed: Option<(usize, Pt)>,
    ) -> Result<(Line, Pt, Pt), ModelError> {
        let (at, anchor) = match fixed {
            Some((end, p)) => (end, p),
            None => (0, self.free()?),
        };
        let other = if short {
            let v = self.short();
            let v = if at == 0 { v } else { scale(v, -1.0) };
            self.point(add(anchor.1, v))?
        } else {
            self.free()?
        };
        let (a, b) = if at == 0 {
            (anchor, other)
        } else {
            (other, anchor)
        };
        Ok((self.sk.line(a.0, b.0)?, a, b))
    }

    /// A circle of radius `r`, or an arc from `alpha` through the sweep the
    /// fraction `sweep_q` picks, centred on `c`.
    fn round(
        &mut self,
        curve: Curve,
        c: Pt,
        r: f64,
        alpha: f64,
        sweep_q: f64,
    ) -> Result<Rd, ModelError> {
        match curve {
            Curve::Circle => Ok(Rd {
                round: self.sk.circle(c.0, r)?.into(),
                s: None,
                e: None,
            }),
            Curve::Arc => {
                let s = self.point(add(c.1, scale(dir(alpha), r)))?;
                self.arc_from(c, s, sweep_q)
            }
        }
    }

    /// An arc from an existing start; its end sits at the same radius, the
    /// sweep (0.3 to 2π − 0.3 radians) picked by `sweep_q`.
    fn arc_from(&mut self, c: Pt, s: Pt, sweep_q: f64) -> Result<Rd, ModelError> {
        let v = sub(s.1, c.1);
        let (cos, sin) = (
            libm::cos(0.3 + (TAU - 0.6) * sweep_q),
            libm::sin(0.3 + (TAU - 0.6) * sweep_q),
        );
        let e = self.point(add(c.1, [v[0] * cos - v[1] * sin, v[0] * sin + v[1] * cos]))?;
        Ok(Rd {
            round: self.sk.arc(c.0, s.0, e.0)?.into(),
            s: Some(s),
            e: Some(e),
        })
    }
}

/// Two distinct lines for a two-line constraint, as the mode says.
fn two_lines(
    b: &mut Build,
    mode: Mode,
    short: bool,
    second: bool,
    q: &[f64; 8],
    len: f64,
) -> Result<(Line, Line), ModelError> {
    let (l1, a1, b1) = b.line(short && !second, None)?;
    let l2 = match mode {
        Mode::Shared => {
            let end = usize::from(q[2] > 0.5);
            let pin = if q[3] > 0.5 { a1 } else { b1 };
            b.line(false, Some((end, pin)))?.0
        }
        Mode::NearParallel => {
            let c = b.free()?;
            let eps = (1.0e-6 + 1.0e-3 * q[2]) * if q[3] > 0.5 { 1.0 } else { -1.0 };
            let flip = if q[2] > 0.5 { PI } else { 0.0 };
            let ang = heading(sub(b1.1, a1.1)) + eps + flip;
            let e = b.point(add(c.1, scale(dir(ang), len)))?;
            b.sk.line(c.0, e.0)?
        }
        Mode::Generic | Mode::ShortLine | Mode::On | Mode::NearOn | Mode::NearEqualRadius => {
            b.line(short && second, None)?.0
        }
    };
    Ok((l1, l2))
}

/// The point at fraction `t` along `a → b`, `off` to the left of it.
fn along(a: [f64; 2], b: [f64; 2], t: f64, off: f64) -> [f64; 2] {
    let u = sub(b, a);
    let n = length(u);
    add(add(a, scale(u, t)), scale([-u[1] / n, u[0] / n], off))
}

fn build(variant: Variant, mode: Mode, d: Draw) -> Result<Configuration, ModelError> {
    let (q, s) = (d.q, d.size);
    let mut b = Build {
        sk: Sketch::new(),
        d,
        next: 0,
    };
    let short = mode == Mode::ShortLine;
    let second = q[7] > 0.5;
    let near = if q[4] > 0.5 { NEAR * s } else { -NEAR * s };
    let off = if mode == Mode::NearOn { near } else { 0.0 };
    let on = matches!(mode, Mode::On | Mode::NearOn);
    // Along the line, away from its ends.
    let t = 0.1 + 0.8 * q[0];
    let r_generic = s * (0.1 + 0.4 * q[1]);
    let constraint = match variant {
        Variant::Coincident => {
            let (p, qq) = (b.free()?, b.free()?);
            Constraint::Coincident { p: p.0, q: qq.0 }
        }
        Variant::PointOnLine => {
            let (line, la, lb) = b.line(short, None)?;
            let p = if on {
                b.point(along(la.1, lb.1, t, off))?
            } else if mode == Mode::Shared {
                if q[2] > 0.5 { la } else { lb }
            } else {
                b.free()?
            };
            Constraint::PointOnLine { p: p.0, line }
        }
        Variant::PointOnCircle(curve) => {
            let c = b.free()?;
            let rd = b.round(curve, c, r_generic, TAU * q[2], q[3])?;
            let p = if on {
                // On an arc, away from its start so no pair is close.
                let beta = match curve {
                    Curve::Circle => TAU * q[0],
                    Curve::Arc => TAU * q[2] + 0.3 + (TAU - 0.6) * q[0],
                };
                b.point(add(c.1, scale(dir(beta), r_generic + off)))?
            } else if mode == Mode::Shared {
                match (rd.s, rd.e) {
                    (Some(st), Some(en)) => {
                        if q[2] > 0.5 {
                            st
                        } else {
                            en
                        }
                    }
                    (None, _) | (_, None) => b.free()?,
                }
            } else {
                b.free()?
            };
            Constraint::PointOnCircle {
                p: p.0,
                on: rd.round,
            }
        }
        Variant::Horizontal(reach) | Variant::Vertical(reach) => {
            let span = match reach {
                Reach::Line => Span::Line(b.line(short, None)?.0),
                Reach::Points => {
                    let p = b.free()?;
                    let r = if short {
                        let v = b.short();
                        b.point(add(p.1, v))?
                    } else {
                        b.free()?
                    };
                    Span::Points(p.0, r.0)
                }
            };
            if matches!(variant, Variant::Horizontal(_)) {
                Constraint::Horizontal(span)
            } else {
                Constraint::Vertical(span)
            }
        }
        Variant::Parallel => {
            let (a, c) = two_lines(&mut b, mode, short, second, &q, r_generic)?;
            Constraint::Parallel { a, b: c }
        }
        Variant::Perpendicular => {
            let (a, c) = two_lines(&mut b, mode, short, second, &q, r_generic)?;
            Constraint::Perpendicular { a, b: c }
        }
        Variant::EqualLength => {
            let (a, c) = two_lines(&mut b, mode, short, second, &q, r_generic)?;
            Constraint::EqualLength { a, b: c }
        }
        Variant::Angle => {
            let (a, c) = two_lines(&mut b, mode, short, second, &q, r_generic)?;
            Constraint::Angle {
                a,
                b: c,
                value: 0.01 + 3.1 * q[5],
            }
        }
        Variant::EqualRadius(ca, cb) => {
            let ca_centre = b.free()?;
            let ra = b.round(ca, ca_centre, r_generic, TAU * q[2], q[3])?;
            let gamma = TAU * q[5];
            let shared_start = if q[1] > 0.5 { ra.s } else { ra.e };
            let rb = match (mode, shared_start) {
                (Mode::Shared, Some(st)) => {
                    // The second arc starts where the first does or ends,
                    // its centre a radius away.
                    let c = b.point(add(st.1, scale(dir(gamma), r_generic)))?;
                    b.arc_from(c, st, q[6])?
                }
                (Mode::NearEqualRadius, _) => {
                    let sgn = if q[4] > 0.5 { 1.0 } else { -1.0 };
                    let r = r_generic + sgn * s * (1.0e-5 + 1.0e-3 * q[6]);
                    let c = b.free()?;
                    b.round(cb, c, r, gamma, q[3])?
                }
                _ => {
                    let c = b.free()?;
                    b.round(cb, c, s * (0.1 + 0.4 * q[6]), gamma, q[3])?
                }
            };
            Constraint::EqualRadius {
                a: ra.round,
                b: rb.round,
            }
        }
        Variant::TangentLineCircle(curve, sense) => {
            let (line, la, lb) = b.line(short, None)?;
            let c = if on {
                b.point(along(la.1, lb.1, t, sense.sign() * (r_generic + off)))?
            } else if mode == Mode::Shared {
                if q[2] > 0.5 { la } else { lb }
            } else {
                b.free()?
            };
            let rd = b.round(curve, c, r_generic, TAU * q[2], q[3])?;
            Constraint::TangentLineCircle {
                line,
                round: rd.round,
                sense,
            }
        }
        Variant::TangentCircles(ca, cb, contact) => {
            let ra_len = if on {
                s * (0.3 + 0.2 * q[0])
            } else {
                r_generic
            };
            let near_equal = mode == Mode::NearEqualRadius;
            let mut gap = s * (0.05 + 0.2 * q[3]);
            let mut rb_len = s * (0.1 + 0.4 * q[6]);
            match contact {
                Contact::External => {
                    if near_equal {
                        rb_len = ra_len + s * (1.0e-5 + 1.0e-3 * q[6]);
                    }
                }
                Contact::Internal(sigma) => {
                    if near_equal {
                        gap = s * (1.0e-5 + 1.0e-3 * q[3]);
                    }
                    if on || near_equal {
                        rb_len = ra_len - sigma.sign() * gap;
                    }
                }
            }
            let gamma = TAU * q[5];
            let a_centre = b.free()?;
            // Where the second centre goes when the contact fixes it.
            let placed = match (on, near_equal, contact) {
                (true, _, Contact::External) => Some(ra_len + rb_len + off),
                (true, _, Contact::Internal(_)) => Some(gap + off),
                (false, true, Contact::Internal(_)) => Some(gap * (1.0 + 0.5 * q[2])),
                (false, true, Contact::External) | (false, false, _) => None,
            };
            let (ra, rb);
            match (mode, ca, cb) {
                (Mode::Shared, Curve::Arc, Curve::Arc) => {
                    let first = b.round(ca, a_centre, ra_len, TAU * q[2], q[3])?;
                    let shared = if q[1] > 0.5 { first.s } else { first.e };
                    let st = shared.unwrap_or(a_centre);
                    let c = b.point(add(st.1, scale(dir(gamma), rb_len)))?;
                    rb = b.arc_from(c, st, q[6])?;
                    ra = first;
                }
                (Mode::Shared, Curve::Arc, Curve::Circle) => {
                    let first = b.round(ca, a_centre, ra_len, TAU * q[2], q[3])?;
                    let c = first.s.unwrap_or(a_centre);
                    rb = b.round(cb, c, rb_len, gamma, q[6])?;
                    ra = first;
                }
                (Mode::Shared, Curve::Circle, Curve::Arc) => {
                    // The circle is centred on the arc's start, so the arc
                    // is drawn first.
                    let arc = b.round(cb, a_centre, rb_len, TAU * q[2], q[3])?;
                    let c = arc.s.unwrap_or(a_centre);
                    ra = b.round(ca, c, ra_len, gamma, q[6])?;
                    rb = arc;
                }
                _ => {
                    ra = b.round(ca, a_centre, ra_len, TAU * q[2], q[3])?;
                    let c = match placed {
                        Some(dist) => b.point(add(a_centre.1, scale(dir(gamma), dist)))?,
                        None => b.free()?,
                    };
                    rb = b.round(cb, c, rb_len, TAU * q[1], q[6])?;
                }
            }
            Constraint::TangentCircles {
                a: ra.round,
                b: rb.round,
                contact,
            }
        }
        Variant::Midpoint => {
            let (line, la, lb) = b.line(short, None)?;
            let p = if on {
                let m = scale(add(la.1, lb.1), 0.5);
                b.point(add(m, scale(dir(TAU * q[2]), off.abs())))?
            } else if mode == Mode::Shared {
                if q[2] > 0.5 { la } else { lb }
            } else {
                b.free()?
            };
            Constraint::Midpoint { p: p.0, line }
        }
        Variant::Symmetric => {
            let (axis, la, lb) = b.line(short, None)?;
            let (p, qq) = if on {
                let w = s * (0.05 + 0.4 * q[3]);
                let p = b.point(along(la.1, lb.1, t, w))?;
                let qq = b.point(along(la.1, lb.1, t, -w + off))?;
                (p, qq)
            } else if mode == Mode::Shared {
                let p = if q[2] > 0.5 { la } else { lb };
                (p, b.free()?)
            } else {
                (b.free()?, b.free()?)
            };
            Constraint::Symmetric {
                p: p.0,
                q: qq.0,
                axis,
            }
        }
        Variant::Fix => {
            let p = b.free()?;
            Constraint::Fix {
                p: p.0,
                x: s * (2.0 * q[0] - 1.0),
                y: s * (2.0 * q[1] - 1.0),
            }
        }
        Variant::Distance => {
            let p = b.free()?;
            let value = s * (0.1 + 0.9 * q[0]);
            let other = if on {
                b.point(add(p.1, scale(dir(TAU * q[2]), value + off)))?
            } else if short {
                let v = b.short();
                b.point(add(p.1, v))?
            } else {
                b.free()?
            };
            Constraint::Distance {
                p: p.0,
                q: other.0,
                value,
            }
        }
        Variant::PointLineDistance(sense) => {
            let (line, la, lb) = b.line(short, None)?;
            let value = s * (0.05 + 0.45 * q[3]);
            let p = if on {
                b.point(along(la.1, lb.1, t, sense.sign() * (value + off)))?
            } else if mode == Mode::Shared {
                if q[2] > 0.5 { la } else { lb }
            } else {
                b.free()?
            };
            Constraint::PointLineDistance {
                p: p.0,
                line,
                value,
                sense,
            }
        }
        Variant::Radius(curve) => {
            let c = b.free()?;
            let rd = b.round(curve, c, r_generic, TAU * q[2], q[3])?;
            let value = if on {
                r_generic + off
            } else {
                s * (0.1 + 0.5 * q[5])
            };
            Constraint::Radius {
                round: rd.round,
                value,
            }
        }
    };
    let id = b.sk.add_constraint(constraint)?;
    let size =
        b.sk.coordinates()
            .values()
            .fold(0.0_f64, |m, v| m.max(v.abs()));
    Ok(Configuration {
        variant,
        mode,
        sketch: b.sk,
        constraint: id,
        size,
    })
}

/// Configurations of `variant` in `mode`, which must be one of its
/// [`Variant::modes`] (any other draws it in [`Mode::Generic`]).
///
/// ```
/// use laman_debug::strategy::configuration::{configuration_in, Mode, Variant};
/// use proptest::prelude::*;
/// use proptest::strategy::ValueTree;
/// use proptest::test_runner::TestRunner;
///
/// let mut runner = TestRunner::deterministic();
/// let drawn = configuration_in(Variant::PointOnLine, Mode::NearOn)
///     .new_tree(&mut runner)
///     .unwrap()
///     .current();
/// assert_eq!(drawn.mode, Mode::NearOn);
/// assert!(drawn.sketch.constraint(drawn.constraint).is_some());
/// ```
pub fn configuration_in(variant: Variant, mode: Mode) -> impl Strategy<Value = Configuration> {
    let mode = if variant.modes().contains(&mode) {
        mode
    } else {
        Mode::Generic
    };
    draw().prop_map(move |d| {
        build(variant, mode, d).expect("a drawn configuration is a well-formed sketch")
    })
}

/// Configurations of `variant` in any of its modes, each as often as any
/// other.
///
/// ```
/// use laman_debug::strategy::configuration::{configuration, Variant};
/// use proptest::prelude::*;
/// use proptest::strategy::ValueTree;
/// use proptest::test_runner::TestRunner;
///
/// let mut runner = TestRunner::deterministic();
/// let drawn = configuration(Variant::Angle).new_tree(&mut runner).unwrap().current();
/// assert!(drawn.variant.modes().contains(&drawn.mode));
/// ```
pub fn configuration(variant: Variant) -> impl Strategy<Value = Configuration> {
    let modes = variant.modes();
    (0..modes.len(), draw()).prop_map(move |(i, d)| {
        let mode = modes.get(i).copied().unwrap_or(Mode::Generic);
        build(variant, mode, d).expect("a drawn configuration is a well-formed sketch")
    })
}
