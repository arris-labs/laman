//! Intersections of random sketches built by construction
//! (plans/m12-intersections step 8, ADR-0016). A base curve (a line, a
//! circle or an arc) has eight slots along it; each slot holds nothing or a
//! cutter built through chosen meeting points: a line crossing the base, a
//! line ending on it (a T), a round crossing it twice, a line or round
//! touching it, a collinear line or concentric arc overlapping it, or an arc
//! whose circle crosses the base outside the arc's sweep. Every cutter stays
//! inside its slot's window, the windows are disjoint, and the arc base's
//! last two slots lie outside its sweep, so the construction says every
//! meeting: its kind, its point and its parameter on both curves. The whole
//! sketch is rotated, scaled by 1e-6 to 1e6 and translated up to a thousand
//! times its scale; the base is drawn first or last, so it is either curve of
//! its pairs. Kinds, counts, points, parameters and the order along the base
//! are asserted against the construction.
//!
//! Each margin of the construction (a crossing's angle, a touch's distance
//! from crossing, the gap between windows) is at least a hundredth of the
//! design's units, far above the length tolerance (ADR-0003). A failure is
//! reduced by proptest and committed as a fixture under
//! `tests/fixtures/regression/` with the desired assertion and an `#[ignore]`
//! (`.agents/rules/solver.md`), never a wider tolerance here.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // tests: a failing unwrap is the test failing (clippy.toml)

use laman::intersect::intersections;
use laman::model::intersection::{Meeting, MeetingKind};
use laman::model::{EntityId, Point, Sketch};
use proptest::prelude::*;
use std::f64::consts::{PI, TAU};

/// How close a meeting's point must be to the construction's, relative to
/// the sketch's size with its translation. The points are built in design
/// units and rounded once by the transform; a crossing at the shallowest
/// built angle (0.3 rad) loses a few ulps of the largest coordinate to it.
const POINT_RELATIVE: f64 = 1e-9;
/// How close a parameter must be to the construction's. Parameters are
/// dimensionless and measured in each pair's frame, so they keep their
/// digits at any scale and translation.
const PARAM: f64 = 1e-9;
/// A circle's parameter this close to 0 or 1 may come out on either side of
/// the wrap; the order is then not asserted (the meetings still are).
const WRAP_GUARD: f64 = 1e-6;

/// The base line's length, the base round's radius, and the arc base's sweep
/// counter-clockwise from (R, 0): its last two of eight slots lie outside.
const LINE_LEN: f64 = 200.0;
const R: f64 = 100.0;
const ARC_SWEEP: f64 = 1.5 * PI;
const SLOTS: usize = 8;
/// The most a slot's point moves from the slot's centre. A cutter reaches
/// at most 8 from its slot's point, and slots are 25 (line) or 78 (round)
/// apart, so no two cutters meet.
const JITTER: f64 = 2.0;

/// The similarity every coordinate goes through.
#[derive(Debug, Clone, Copy)]
struct Transform {
    angle: f64,
    scale: f64,
    shift: [f64; 2],
}

impl Transform {
    fn at(&self, [x, y]: [f64; 2]) -> [f64; 2] {
        let (s, c) = libm::sincos(self.angle);
        [
            self.scale * (c * x - s * y) + self.shift[0],
            self.scale * (s * x + c * y) + self.shift[1],
        ]
    }

    /// The size a point's error is measured against: the design's extent
    /// and the translation.
    fn size(&self) -> f64 {
        self.scale * 3.0 * R + self.shift[0].abs().max(self.shift[1].abs())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum BaseKind {
    Line,
    Circle,
    Arc,
}

#[derive(Debug, Clone, Copy)]
enum CutKind {
    /// A line through the slot's point.
    CrossLine,
    /// A line ending at the slot's point.
    TLine,
    /// A circle or an arc through two points of the base.
    CrossRound,
    /// A line (round bases only), circle or arc tangent at the slot's point.
    Touch,
    /// A collinear line, or a concentric arc of the base's radius.
    Overlap,
    /// An arc whose circle crosses the base twice, its sweep clear of both.
    Outside,
}

/// One slot's cutter: its kind and the free numbers of its construction,
/// each in [0, 1).
#[derive(Debug, Clone, Copy)]
struct Cut {
    kind: CutKind,
    form: u8,
    a: f64,
    b: f64,
    c: f64,
    flip: bool,
    side: bool,
    jitter: f64,
}

/// A curve's parameter at a meeting, and whether it is a circle's (an angle
/// over 2π, compared across the wrap).
#[derive(Debug, Clone, Copy)]
struct Param {
    value: f64,
    circular: bool,
}

impl Param {
    fn near(&self, found: f64) -> bool {
        if self.circular {
            let d = (found - self.value).rem_euclid(1.0);
            d.min(1.0 - d) <= PARAM
        } else {
            (found - self.value).abs() <= PARAM
        }
    }

    fn by_wrap(&self) -> bool {
        let v = self.value.rem_euclid(1.0);
        self.circular && (v < WRAP_GUARD || v > 1.0 - WRAP_GUARD)
    }
}

/// A place on both curves, in design units.
#[derive(Debug, Clone, Copy)]
struct Place {
    at: [f64; 2],
    on_base: Param,
    on_cut: Param,
}

#[derive(Debug, Clone, Copy)]
enum Want {
    Crossing(Place),
    Touch(Place),
    /// From the cutter's start to its end.
    Overlap(Place, Place),
}

fn wrap(turns: f64) -> f64 {
    turns.rem_euclid(1.0)
}

fn add(p: [f64; 2], q: [f64; 2]) -> [f64; 2] {
    [p[0] + q[0], p[1] + q[1]]
}

fn mul(k: f64, p: [f64; 2]) -> [f64; 2] {
    [k * p[0], k * p[1]]
}

fn polar(centre: [f64; 2], r: f64, angle: f64) -> [f64; 2] {
    let (s, c) = libm::sincos(angle);
    [centre[0] + r * c, centre[1] + r * s]
}

fn angle_of(centre: [f64; 2], p: [f64; 2]) -> f64 {
    libm::atan2(p[1] - centre[1], p[0] - centre[0])
}

/// The base in design units: a point at arc length `s` from its origin,
/// the unit tangent there and the left normal (inward on a round).
struct Base {
    kind: BaseKind,
    /// The line drawn from its far end.
    flip: bool,
    rotation: f64,
}

impl Base {
    fn point(&self, s: f64) -> [f64; 2] {
        match self.kind {
            BaseKind::Line => [s, 0.0],
            BaseKind::Circle | BaseKind::Arc => polar([0.0; 2], R, s / R),
        }
    }

    fn tangent(&self, s: f64) -> [f64; 2] {
        match self.kind {
            BaseKind::Line => [1.0, 0.0],
            BaseKind::Circle | BaseKind::Arc => {
                let (sn, cs) = libm::sincos(s / R);
                [-sn, cs]
            }
        }
    }

    fn normal(&self, s: f64) -> [f64; 2] {
        let t = self.tangent(s);
        [-t[1], t[0]]
    }

    fn param(&self, s: f64) -> Param {
        match self.kind {
            BaseKind::Line => Param {
                value: if self.flip {
                    1.0 - s / LINE_LEN
                } else {
                    s / LINE_LEN
                },
                circular: false,
            },
            BaseKind::Circle => Param {
                value: wrap((s / R + self.rotation) / TAU),
                circular: true,
            },
            BaseKind::Arc => Param {
                value: s / R / ARC_SWEEP,
                circular: false,
            },
        }
    }

    /// Slot `i`'s point along the base, before its jitter.
    fn slot(&self, i: usize) -> f64 {
        match self.kind {
            BaseKind::Line => 12.5 + 25.0 * i as f64,
            BaseKind::Circle | BaseKind::Arc => R * (i as f64 + 0.5) * TAU / SLOTS as f64,
        }
    }

    /// Whether slot `i` lies in the base: the arc's last two do not.
    fn holds(&self, i: usize) -> bool {
        match self.kind {
            BaseKind::Line | BaseKind::Circle => true,
            BaseKind::Arc => i < 6,
        }
    }
}

/// A round cutter of centre `centre` and radius `rho`: a circle, or the arc
/// from the angle `start` sweeping `sweep` counter-clockwise.
struct Round {
    centre: [f64; 2],
    rho: f64,
    arc: Option<(f64, f64)>,
}

impl Round {
    fn param(&self, p: [f64; 2], rotation: f64) -> Param {
        let a = angle_of(self.centre, p);
        match self.arc {
            None => Param {
                value: wrap((a + rotation) / TAU),
                circular: true,
            },
            Some((start, sweep)) => Param {
                value: (a - start).rem_euclid(TAU) / sweep,
                circular: false,
            },
        }
    }

    fn build(&self, sk: &mut Sketch, t: &Transform) -> EntityId {
        let c = pt(sk, t, self.centre);
        match self.arc {
            None => sk.circle(c, self.rho * t.scale).unwrap().id(),
            Some((start, sweep)) => {
                let s = pt(sk, t, polar(self.centre, self.rho, start));
                let e = pt(sk, t, polar(self.centre, self.rho, start + sweep));
                sk.arc(c, s, e).unwrap().id()
            }
        }
    }
}

fn pt(sk: &mut Sketch, t: &Transform, p: [f64; 2]) -> Point {
    let [x, y] = t.at(p);
    sk.point(x, y).unwrap()
}

/// A line from `from` to `to`, drawn the other way when `flip`.
fn line(sk: &mut Sketch, t: &Transform, from: [f64; 2], to: [f64; 2], flip: bool) -> EntityId {
    let (a, b) = if flip { (to, from) } else { (from, to) };
    let (a, b) = (pt(sk, t, a), pt(sk, t, b));
    sk.line(a, b).unwrap().id()
}

/// A line's parameter at the fraction `f` of the way from `from`.
fn line_param(f: f64, flip: bool) -> Param {
    Param {
        value: if flip { 1.0 - f } else { f },
        circular: false,
    }
}

/// The arc through `p1` and `p2` about `centre` holding both with a margin,
/// or, `clear`, one of the two arcs between them, clear of both.
fn sweep_through(centre: [f64; 2], p1: [f64; 2], p2: [f64; 2], clear: Option<bool>) -> (f64, f64) {
    const MARGIN: f64 = 0.15;
    let (a1, a2) = (angle_of(centre, p1), angle_of(centre, p2));
    let ccw = (a2 - a1).rem_euclid(TAU);
    match clear {
        None if ccw <= PI => (a1 - MARGIN, ccw + 2.0 * MARGIN),
        None => (a2 - MARGIN, TAU - ccw + 2.0 * MARGIN),
        Some(true) => (a1 + MARGIN, ccw - 2.0 * MARGIN),
        Some(false) => (a2 + MARGIN, TAU - ccw - 2.0 * MARGIN),
    }
}

/// Builds the cutter of slot `i` and returns its id with the meetings the
/// construction says it has with the base.
fn cutter(
    sk: &mut Sketch,
    t: &Transform,
    base: &Base,
    i: usize,
    cut: &Cut,
) -> (EntityId, Vec<Want>) {
    let s = base.slot(i) + (2.0 * cut.jitter - 1.0) * JITTER;
    let (p, tan, nor) = (base.point(s), base.tangent(s), base.normal(s));
    let round_base = !matches!(base.kind, BaseKind::Line);
    let (id, wants) = match cut.kind {
        CutKind::CrossLine => {
            let phi = 0.4 + cut.a * (PI - 0.8);
            let (sn, cs) = libm::sincos(phi);
            let d = add(mul(cs, tan), mul(sn, nor));
            let (alpha, beta) = (1.0 + 5.0 * cut.b, 1.0 + 5.0 * cut.c);
            let id = line(
                sk,
                t,
                add(p, mul(-alpha, d)),
                add(p, mul(beta, d)),
                cut.flip,
            );
            let place = Place {
                at: p,
                on_base: base.param(s),
                on_cut: line_param(alpha / (alpha + beta), cut.flip),
            };
            (id, vec![Want::Crossing(place)])
        }
        CutKind::TLine => {
            let phi = 0.4 + cut.a * (PI - 0.8);
            let (sn, cs) = libm::sincos(phi);
            let sn = if cut.side { sn } else { -sn };
            let d = add(mul(cs, tan), mul(sn, nor));
            let end = add(p, mul(1.0 + 6.0 * cut.b, d));
            let id = line(sk, t, p, end, cut.flip);
            let place = Place {
                at: p,
                on_base: base.param(s),
                on_cut: line_param(0.0, cut.flip),
            };
            (id, vec![Want::Crossing(place)])
        }
        CutKind::CrossRound | CutKind::Outside => {
            let delta = 1.0 + 2.0 * cut.a;
            let (s1, s2) = (s - delta, s + delta);
            let (p1, p2) = (base.point(s1), base.point(s2));
            let mid = mul(0.5, add(p1, p2));
            let centre = add(mid, mul(6.0 * cut.b - 3.0, nor));
            let rho = libm::hypot(p1[0] - centre[0], p1[1] - centre[1]);
            let outside = matches!(cut.kind, CutKind::Outside);
            let arc = if outside {
                Some(sweep_through(centre, p1, p2, Some(cut.flip)))
            } else if cut.form % 2 == 1 {
                Some(sweep_through(centre, p1, p2, None))
            } else {
                None
            };
            let round = Round { centre, rho, arc };
            let id = round.build(sk, t);
            let wants = if outside {
                vec![]
            } else {
                [(p1, s1), (p2, s2)]
                    .into_iter()
                    .map(|(q, sq)| {
                        Want::Crossing(Place {
                            at: q,
                            on_base: base.param(sq),
                            on_cut: round.param(q, t.angle),
                        })
                    })
                    .collect()
            };
            (id, wants)
        }
        CutKind::Touch => {
            let side = if cut.side { 1.0 } else { -1.0 };
            let rho = 1.0 + 3.0 * cut.a;
            let form = cut.form % if round_base { 3 } else { 2 };
            if form == 2 {
                // A line along the round base's tangent.
                let (alpha, beta) = (1.0 + 5.0 * cut.b, 1.0 + 5.0 * cut.c);
                let id = line(
                    sk,
                    t,
                    add(p, mul(-alpha, tan)),
                    add(p, mul(beta, tan)),
                    cut.flip,
                );
                let place = Place {
                    at: p,
                    on_base: base.param(s),
                    on_cut: line_param(alpha / (alpha + beta), cut.flip),
                };
                (id, vec![Want::Touch(place)])
            } else {
                let centre = add(p, mul(side * rho, nor));
                let at = angle_of(centre, p);
                let arc = (form == 1).then(|| {
                    let (m1, m2) = (0.3 + 0.9 * cut.b, 0.3 + 0.9 * cut.c);
                    (at - m1, m1 + m2)
                });
                let round = Round { centre, rho, arc };
                let id = round.build(sk, t);
                let place = Place {
                    at: p,
                    on_base: base.param(s),
                    on_cut: round.param(p, t.angle),
                };
                (id, vec![Want::Touch(place)])
            }
        }
        CutKind::Overlap => {
            let (lo, hi) = (s - 1.0 - 5.0 * cut.a, s + 1.0 + 5.0 * cut.b);
            let (from, to) = (base.point(lo), base.point(hi));
            if round_base {
                let c = pt(sk, t, [0.0; 2]);
                let (a, b) = (pt(sk, t, from), pt(sk, t, to));
                let id = sk.arc(c, a, b).unwrap().id();
                let fixed = |v: f64| Param {
                    value: v,
                    circular: false,
                };
                let start = Place {
                    at: from,
                    on_base: base.param(lo),
                    on_cut: fixed(0.0),
                };
                let mut end = Place {
                    at: to,
                    on_base: base.param(hi),
                    on_cut: fixed(1.0),
                };
                if base.kind == BaseKind::Circle {
                    // A stretch on a circle runs counter-clockwise from its
                    // start's angle, past 1 when it wraps.
                    end.on_base.value = start.on_base.value + (hi - lo) / R / TAU;
                }
                (id, vec![Want::Overlap(start, end)])
            } else {
                let id = line(sk, t, from, to, cut.flip);
                let start = Place {
                    at: from,
                    on_base: base.param(lo),
                    on_cut: line_param(0.0, cut.flip),
                };
                let end = Place {
                    at: to,
                    on_base: base.param(hi),
                    on_cut: line_param(1.0, cut.flip),
                };
                (id, vec![Want::Overlap(start, end)])
            }
        }
    };
    let wants = if base.holds(i) { wants } else { vec![] };
    (id, wants)
}

/// A wanted meeting as the pair reports it: `base_first` says whose
/// parameter is `on_first`.
struct Expect {
    kind: u8,
    at: [f64; 2],
    on_first: Param,
    on_second: Param,
    /// An overlap's other end.
    to: Option<([f64; 2], Param, Param)>,
    /// The parameter on the base, for the order along it.
    on_base: Param,
}

fn expect(want: &Want, base_first: bool, base_circle: bool) -> Expect {
    let ordered = |p: &Place| {
        if base_first {
            (p.on_base, p.on_cut)
        } else {
            (p.on_cut, p.on_base)
        }
    };
    match want {
        Want::Crossing(p) | Want::Touch(p) => {
            let (on_first, on_second) = ordered(p);
            Expect {
                kind: if matches!(want, Want::Crossing(_)) {
                    0
                } else {
                    1
                },
                at: p.at,
                on_first,
                on_second,
                to: None,
                on_base: p.on_base,
            }
        }
        Want::Overlap(start, end) => {
            // From the end of lower parameter on the first curve; on a
            // circle, from the cutter's start, counter-clockwise.
            let first_circle = base_first && base_circle;
            let (lo, hi) = if first_circle || ordered(start).0.value <= ordered(end).0.value {
                (start, end)
            } else {
                (end, start)
            };
            let (on_first, on_second) = ordered(lo);
            let (to_first, to_second) = ordered(hi);
            // Along the base, from its end of lower parameter there.
            let on_base = if base_circle || start.on_base.value <= end.on_base.value {
                start.on_base
            } else {
                end.on_base
            };
            Expect {
                kind: 2,
                at: lo.at,
                on_first,
                on_second,
                to: Some((hi.at, to_first, to_second)),
                on_base,
            }
        }
    }
}

fn kind_of(m: &Meeting) -> u8 {
    match m.kind {
        MeetingKind::Crossing => 0,
        MeetingKind::Touch => 1,
        MeetingKind::Overlap { .. } => 2,
    }
}

fn build_base(sk: &mut Sketch, t: &Transform, base: &Base) -> EntityId {
    match base.kind {
        BaseKind::Line => line(sk, t, [0.0; 2], [LINE_LEN, 0.0], base.flip),
        BaseKind::Circle => {
            let c = pt(sk, t, [0.0; 2]);
            sk.circle(c, R * t.scale).unwrap().id()
        }
        BaseKind::Arc => {
            let c = pt(sk, t, [0.0; 2]);
            let s = pt(sk, t, [R, 0.0]);
            let e = pt(sk, t, [0.0, -R]);
            sk.arc(c, s, e).unwrap().id()
        }
    }
}

/// Builds the base and its cutters and checks `intersections` against them.
fn check(
    base_kind: BaseKind,
    base_flip: bool,
    base_last: bool,
    cuts: &[Option<Cut>],
    t: &Transform,
) -> Result<(), TestCaseError> {
    let base = Base {
        kind: base_kind,
        flip: base_flip,
        rotation: t.angle,
    };
    let mut sk = Sketch::new();
    let mut base_id = (!base_last).then(|| build_base(&mut sk, t, &base));
    let mut built = Vec::new();
    for (i, cut) in cuts.iter().enumerate() {
        if let Some(cut) = cut {
            built.push(cutter(&mut sk, t, &base, i, cut));
        }
    }
    let base_id = *base_id.get_or_insert_with(|| build_base(&mut sk, t, &base));

    let found = intersections(&sk, &sk.coordinates()).unwrap();
    let wanted: Vec<_> = built.iter().filter(|(_, w)| !w.is_empty()).collect();
    prop_assert_eq!(
        found.pairs.len(),
        wanted.len(),
        "pairs {:?}",
        found
            .pairs
            .iter()
            .map(|p| (p.first, p.second))
            .collect::<Vec<_>>()
    );
    let near = |found: [f64; 2], want: [f64; 2]| {
        let w = t.at(want);
        libm::hypot(found[0] - w[0], found[1] - w[1]) <= POINT_RELATIVE * t.size()
    };
    let base_circle = base_kind == BaseKind::Circle;
    let mut along_base = Vec::new();
    for (cut_id, wants) in &wanted {
        let base_first = base_id < *cut_id;
        let (first, second) = if base_first {
            (base_id, *cut_id)
        } else {
            (*cut_id, base_id)
        };
        let pair = found
            .pairs
            .iter()
            .find(|p| (p.first, p.second) == (first, second));
        prop_assert!(pair.is_some(), "no pair {first} {second}");
        let pair = pair.unwrap();
        let mut expects: Vec<Expect> = wants
            .iter()
            .map(|w| expect(w, base_first, base_circle))
            .collect();
        prop_assert_eq!(pair.meetings.len(), expects.len(), "{:?}", pair.meetings);
        expects.sort_by(|x, y| x.on_first.value.total_cmp(&y.on_first.value));
        let ordered = !expects.iter().any(|e| e.on_first.by_wrap());
        for (k, e) in expects.iter().enumerate() {
            let m = pair.meetings.iter().position(|m| near(m.at, e.at));
            prop_assert!(m.is_some(), "no meeting at {:?}: {:?}", e.at, pair.meetings);
            let m = m.unwrap();
            if ordered {
                prop_assert_eq!(m, k, "order: {:?}", pair.meetings);
            }
            let m = &pair.meetings[m];
            prop_assert_eq!(kind_of(m), e.kind, "{:?}", m);
            prop_assert!(
                e.on_first.near(m.on_first),
                "on_first {:?} {:?}",
                e.on_first,
                m
            );
            prop_assert!(
                e.on_second.near(m.on_second),
                "on_second {:?} {:?}",
                e.on_second,
                m
            );
            if let (
                MeetingKind::Overlap {
                    to,
                    to_first,
                    to_second,
                },
                Some((want, f, s)),
            ) = (m.kind, e.to)
            {
                prop_assert!(near(to, want), "to {:?} {:?}", want, m);
                prop_assert!(f.near(to_first), "to_first {:?} {:?}", f, m);
                prop_assert!(s.near(to_second), "to_second {:?} {:?}", s, m);
            }
            along_base.push((e.on_base, *cut_id));
        }
    }
    // Along the base: every meeting, ordered by its parameter there.
    along_base.sort_by(|x, y| x.0.value.total_cmp(&y.0.value).then(x.1.cmp(&y.1)));
    let seen = found.along(base_id);
    prop_assert_eq!(seen.len(), along_base.len());
    if !along_base.iter().any(|(p, _)| p.by_wrap()) {
        for (s, (p, other)) in seen.iter().zip(&along_base) {
            prop_assert_eq!(s.other, *other);
            prop_assert!(p.near(s.meeting.on_first), "along {:?} {:?}", p, s);
        }
    }
    Ok(())
}

fn cut() -> impl Strategy<Value = Cut> {
    (
        prop_oneof![
            Just(CutKind::CrossLine),
            Just(CutKind::TLine),
            Just(CutKind::CrossRound),
            Just(CutKind::Touch),
            Just(CutKind::Overlap),
            Just(CutKind::Outside),
        ],
        any::<u8>(),
        (0.0..1.0_f64, 0.0..1.0_f64, 0.0..1.0_f64, 0.0..1.0_f64),
        any::<(bool, bool)>(),
    )
        .prop_map(|(kind, form, (a, b, c, jitter), (flip, side))| Cut {
            kind,
            form,
            a,
            b,
            c,
            flip,
            side,
            jitter,
        })
}

fn transform() -> impl Strategy<Value = Transform> {
    (
        0.0..TAU,
        -6.0..6.0_f64,
        -1000.0..1000.0_f64,
        -1000.0..1000.0_f64,
    )
        .prop_map(|(angle, exponent, sx, sy)| {
            let scale = libm::pow(10.0, exponent);
            Transform {
                angle,
                scale,
                shift: [sx * scale, sy * scale],
            }
        })
}

proptest! {
    // No explicit case count: proptest's default and PROPTEST_CASES both apply.
    #[test]
    fn cutters_meet_the_base_as_built(
        base in prop_oneof![Just(BaseKind::Line), Just(BaseKind::Circle), Just(BaseKind::Arc)],
        (base_flip, base_last) in any::<(bool, bool)>(),
        cuts in proptest::collection::vec(proptest::option::weighted(0.8, cut()), SLOTS),
        t in transform(),
    ) {
        check(base, base_flip, base_last, &cuts, &t)?;
    }
}

/// Every cutter kind and form in some slot, the same for each base.
fn every_cut() -> Vec<Option<Cut>> {
    let at = |kind, form, flip, side| {
        Some(Cut {
            kind,
            form,
            a: 0.37,
            b: 0.61,
            c: 0.29,
            flip,
            side,
            jitter: 0.5,
        })
    };
    vec![
        at(CutKind::CrossLine, 0, false, true),
        at(CutKind::TLine, 0, true, false),
        at(CutKind::CrossRound, 0, false, true),
        at(CutKind::CrossRound, 1, true, false),
        at(CutKind::Touch, 1, false, true),
        at(CutKind::Overlap, 0, true, true),
        at(CutKind::Touch, 2, true, false),
        at(CutKind::Outside, 0, false, true),
    ]
}

/// Near 1e300 the construction holds to its closed form, translated by
/// hundreds of times its scale. Near 1e-300 the whole sketch is below the
/// length floor (ADR-0003), so `intersections()` promises finite, typed
/// output and not the closed form (ADR-0016 D2); the kernel's own tests hold
/// that form at 1e-300.
#[test]
fn extreme_scales_hold_the_construction_or_stay_finite() {
    for base in [BaseKind::Line, BaseKind::Circle, BaseKind::Arc] {
        for base_last in [false, true] {
            let big = Transform {
                angle: 0.7,
                scale: 1e300,
                shift: [3e302, -2e302],
            };
            check(base, true, base_last, &every_cut(), &big).unwrap();

            let small = Transform {
                angle: 0.7,
                scale: 1e-300,
                shift: [3e-298, -2e-298],
            };
            let mut sk = Sketch::new();
            let b = Base {
                kind: base,
                flip: false,
                rotation: small.angle,
            };
            let base_id = build_base(&mut sk, &small, &b);
            for (i, cut) in every_cut().iter().enumerate() {
                cutter(&mut sk, &small, &b, i, cut.as_ref().unwrap());
            }
            let found = intersections(&sk, &sk.coordinates()).unwrap();
            for pair in &found.pairs {
                assert!(pair.first < pair.second);
                for m in &pair.meetings {
                    let mut numbers = vec![m.at[0], m.at[1], m.on_first, m.on_second];
                    if let MeetingKind::Overlap {
                        to,
                        to_first,
                        to_second,
                    } = m.kind
                    {
                        numbers.extend([to[0], to[1], to_first, to_second]);
                    }
                    assert!(numbers.iter().all(|v| v.is_finite()), "{m:?}");
                }
            }
            for along in found.along(base_id) {
                assert!(along.meeting.on_first.is_finite());
            }
        }
    }
}
