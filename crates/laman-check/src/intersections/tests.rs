//! Completeness (ADR-0018 D4) against closed forms, with the tolerance an
//! input at the scale, as the kernel's own tests pass it: at 1e-300 a sketch
//! is below `LENGTH_FLOOR` and `Tolerances::at` would make every curve
//! negligible, so the measure itself is held there this way. Scales 1e-300 to
//! 1e300, shifted up to 1e4 times the construction's size: every closed-form
//! meeting covers its place, and each one dropped is missed.

use super::*;

/// The scales and shifts (in units of the construction's size).
const SCALES: [f64; 5] = [1e-300, 1e-6, 1.0, 1e6, 1e300];
const SHIFTS: [f64; 3] = [0.0, 1.0, 1e4];

/// The length tolerance passed, relative to the scale: the order of what
/// `Tolerances::at` gives a sketch of this extent above the floor.
const REL: f64 = 1e-9;

#[derive(Clone, Copy)]
struct Case {
    s: f64,
    shift: f64,
}

impl Case {
    fn p(&self, (x, y): Xy) -> Xy {
        (self.s * (x + self.shift), self.s * (y + 0.7 * self.shift))
    }

    fn line(&self, a: Xy, b: Xy) -> Raw {
        Raw::Line {
            a: self.p(a),
            b: self.p(b),
        }
    }

    fn circle(&self, c: Xy, r: f64) -> Raw {
        Raw::Circle {
            c: self.p(c),
            r: self.s * r,
        }
    }

    fn at(&self, kind: MeetingKind, v: Xy, on_first: f64) -> Meeting {
        let p = self.p(v);
        Meeting {
            kind,
            at: [p.0, p.1],
            on_first,
            on_second: 0.0,
        }
    }

    fn overlap(&self, from: Xy, to: Xy, on_first: f64, to_first: f64) -> Meeting {
        let q = self.p(to);
        let kind = MeetingKind::Overlap {
            to: [q.0, q.1],
            to_first,
            to_second: 0.0,
        };
        self.at(kind, from, on_first)
    }
}

/// Distinct join classes for every end: nothing is joined.
fn classes(raw: &Raw, base: u32) -> Vec<EntityId> {
    (0..raw.ends().len() as u32)
        .map(|k| EntityId(base + k))
        .collect()
}

/// Pairs and their closed-form meetings, every one a decided place.
fn pairs(c: &Case) -> Vec<(Raw, Raw, Vec<Meeting>)> {
    use MeetingKind::{Crossing, Touch};
    let root = libm::sqrt(8.75);
    vec![
        (
            c.line((0.0, 0.0), (4.0, 4.0)),
            c.line((0.0, 4.0), (4.0, 0.0)),
            vec![c.at(Crossing, (2.0, 2.0), 0.5)],
        ),
        (
            c.line((-5.0, 0.5), (5.0, 0.5)),
            c.circle((0.0, 0.0), 3.0),
            vec![
                c.at(Crossing, (-root, 0.5), 0.0),
                c.at(Crossing, (root, 0.5), 0.0),
            ],
        ),
        (
            c.circle((0.0, 0.0), 5.0),
            c.circle((6.0, 0.0), 5.0),
            vec![
                c.at(Crossing, (3.0, 4.0), 0.0),
                c.at(Crossing, (3.0, -4.0), 0.0),
            ],
        ),
        (
            c.line((-4.0, 0.0), (6.0, 0.0)),
            c.circle((1.0, 2.0), 2.0),
            vec![c.at(Touch, (1.0, 0.0), 0.0)],
        ),
        (
            c.line((0.0, 0.0), (4.0, 0.0)),
            c.line((2.0, 0.0), (2.0, 3.0)),
            vec![c.at(Crossing, (2.0, 0.0), 0.5)],
        ),
        (
            c.line((0.0, 0.0), (10.0, 0.0)),
            c.line((6.0, 0.0), (14.0, 0.0)),
            vec![c.overlap((6.0, 0.0), (10.0, 0.0), 0.6, 1.0)],
        ),
        (
            c.circle((1.0, -1.0), 3.0),
            c.circle((1.0, -1.0), 3.0),
            vec![c.overlap((4.0, -1.0), (4.0, -1.0), 0.0, 1.0)],
        ),
    ]
}

#[test]
fn closed_forms_cover_their_places_and_each_dropped_is_missed() {
    for s in SCALES {
        for shift in SHIFTS {
            let c = Case { s, shift };
            let l = REL * s;
            for (k, (ra, rb, meetings)) in pairs(&c).into_iter().enumerate() {
                let m = Measured::new(&ra, &rb, &classes(&ra, 0), &classes(&rb, 2), l, l);
                assert!(!m.missed(&meetings), "{s} {shift} pair {k}");
                for drop in 0..meetings.len() {
                    let mut less = meetings.clone();
                    less.remove(drop);
                    assert!(m.missed(&less), "{s} {shift} pair {k} drop {drop}");
                }
            }
        }
    }
}

#[test]
fn a_shared_end_is_a_join_and_never_a_place() {
    // The T's stem ends on the bar's end: one join class, nothing to find.
    for s in SCALES {
        let c = Case { s, shift: 1e4 };
        let (bar, stem) = (
            c.line((0.0, 0.0), (4.0, 0.0)),
            c.line((4.0, 0.0), (6.0, 3.0)),
        );
        let joined = [EntityId(0), EntityId(1)];
        let m = Measured::new(
            &bar,
            &stem,
            &joined,
            &[EntityId(1), EntityId(2)],
            REL * s,
            REL * s,
        );
        assert!(!m.missed(&[]), "{s}");
        assert_eq!(m.joins().len(), 2, "{s}");
    }
}
