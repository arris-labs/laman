//! `check_intersections` on values built by hand: closed-form meetings are
//! clean, and each broken value names its variant (ADR-0018 D2, D3, D5).
#![allow(clippy::unwrap_used)] // tests: a failing unwrap is the test failing (clippy.toml)

use laman::model::intersection::{Intersection, Intersections, Meeting, MeetingKind};
use laman::model::{Coordinate, EntityId, Sketch};
use laman_check::{IntersectionViolation as V, check_intersections};

fn meeting(kind: MeetingKind, at: [f64; 2], on_first: f64, on_second: f64) -> Meeting {
    Meeting {
        kind,
        at,
        on_first,
        on_second,
    }
}

fn one(first: EntityId, second: EntityId, meetings: Vec<Meeting>) -> Intersections {
    Intersections {
        pairs: vec![Intersection {
            first,
            second,
            meetings,
        }],
    }
}

fn violations(sk: &Sketch, found: &Intersections) -> Vec<V> {
    check_intersections(sk, &sk.coordinates(), found).violations
}

/// Two lines crossing at (2, 2), at `scale`.
fn cross_lines(scale: f64) -> (Sketch, EntityId, EntityId, Meeting) {
    let mut sk = Sketch::new();
    let p = [(0.0, 0.0), (4.0, 4.0), (0.0, 4.0), (4.0, 0.0)]
        .map(|(x, y)| sk.point(x * scale, y * scale).unwrap());
    let (a, b) = (sk.line(p[0], p[1]).unwrap(), sk.line(p[2], p[3]).unwrap());
    let m = meeting(MeetingKind::Crossing, [2.0 * scale; 2], 0.5, 0.5);
    (sk, a.id(), b.id(), m)
}

#[test]
fn a_closed_form_crossing_is_clean_at_every_scale() {
    for scale in [1e-6, 1.0, 1e6, 1e300] {
        let (sk, a, b, m) = cross_lines(scale);
        let found = one(a, b, vec![m]);
        let report = check_intersections(&sk, &sk.coordinates(), &found);
        assert!(report.is_clean(), "scale {scale}: {report:?}");
        // A twin moved by a tenth of the lines is named at the same scale.
        let mut twin = found.clone();
        twin.pairs[0].meetings[0].at = [2.4 * scale, 2.0 * scale];
        // The parameters map back to the old point too, so it names both.
        assert_eq!(
            violations(&sk, &twin)[0],
            V::OffCurve {
                first: a,
                second: b,
                meeting: 0
            },
            "scale {scale}"
        );
    }
}

#[test]
fn a_line_through_a_circle_is_clean_and_its_twins_are_named() {
    let mut sk = Sketch::new();
    let (p, q, c) = (
        sk.point(-5.0, 0.0).unwrap(),
        sk.point(5.0, 0.0).unwrap(),
        sk.point(0.0, 0.0).unwrap(),
    );
    let line = sk.line(p, q).unwrap();
    let circle = sk.circle(c, 3.0).unwrap();
    let (l, ci) = (line.id(), circle.id());
    let cross = |at: [f64; 2], on_first: f64, on_second: f64| {
        meeting(MeetingKind::Crossing, at, on_first, on_second)
    };
    let found = one(
        l,
        ci,
        vec![cross([-3.0, 0.0], 0.2, 0.5), cross([3.0, 0.0], 0.8, 0.0)],
    );
    assert_eq!(violations(&sk, &found), []);

    // A kind changed: a touch where the line cuts the circle through.
    let mut twin = found.clone();
    twin.pairs[0].meetings[0].kind = MeetingKind::Touch;
    assert_eq!(
        violations(&sk, &twin),
        [V::Kind {
            first: l,
            second: ci,
            meeting: 0
        }]
    );

    // A parameter changed: on the line, and on the circle.
    let mut twin = found.clone();
    twin.pairs[0].meetings[1].on_first = 0.7;
    assert_eq!(
        violations(&sk, &twin),
        [V::Parameter {
            first: l,
            second: ci,
            meeting: 1
        }]
    );
    let mut twin = found.clone();
    twin.pairs[0].meetings[0].on_second = 0.25;
    assert_eq!(
        violations(&sk, &twin),
        [V::Parameter {
            first: l,
            second: ci,
            meeting: 0
        }]
    );
    // NaN and a parameter out of range.
    for bad in [f64::NAN, 1.0] {
        let mut twin = found.clone();
        twin.pairs[0].meetings[0].on_second = bad;
        assert_eq!(
            violations(&sk, &twin),
            [V::Parameter {
                first: l,
                second: ci,
                meeting: 0
            }]
        );
    }

    // A meeting moved off the circle.
    let mut twin = found.clone();
    twin.pairs[0].meetings[1].at = [3.5, 0.0];
    assert_eq!(
        violations(&sk, &twin)[0],
        V::OffCurve {
            first: l,
            second: ci,
            meeting: 1
        }
    );
}

#[test]
fn a_point_past_an_arcs_sweep_is_outside_it() {
    // A quarter arc from (3, 0) to (0, 3) about the origin; a line along y = -3
    // is nowhere near it, so use the vertical x = 0 line, which meets the circle
    // at (0, 3) (the end) and (0, -3) (past the sweep).
    let mut sk = Sketch::new();
    let c = sk.point(0.0, 0.0).unwrap();
    let s = sk.point(3.0, 0.0).unwrap();
    let e = sk.point(0.0, 3.0).unwrap();
    let arc = sk.arc(c, s, e).unwrap();
    let (a, b) = (sk.point(0.0, -5.0).unwrap(), sk.point(0.0, 5.0).unwrap());
    let line = sk.line(a, b).unwrap();
    let (ar, ln) = (arc.id(), line.id());
    // The arc is the smaller id. At the end (0, 3): the arc's parameter 1,
    // the line's 0.8.
    let good = meeting(MeetingKind::Crossing, [0.0, 3.0], 1.0, 0.8);
    assert_eq!(violations(&sk, &one(ar, ln, vec![good])), []);
    let past = meeting(MeetingKind::Crossing, [0.0, -3.0], 0.0, 0.2);
    assert_eq!(
        violations(&sk, &one(ar, ln, vec![past]))[0],
        V::OutsideSweep {
            first: ar,
            second: ln,
            meeting: 0
        }
    );
}

#[test]
fn an_overlap_is_read_by_its_stretch() {
    let mut sk = Sketch::new();
    let p =
        [(0.0, 0.0), (10.0, 0.0), (6.0, 0.0), (14.0, 0.0)].map(|(x, y)| sk.point(x, y).unwrap());
    let (a, b) = (sk.line(p[0], p[1]).unwrap(), sk.line(p[2], p[3]).unwrap());
    let (a, b) = (a.id(), b.id());
    let overlap = meeting(
        MeetingKind::Overlap {
            to: [10.0, 0.0],
            to_first: 1.0,
            to_second: 0.5,
        },
        [6.0, 0.0],
        0.6,
        0.0,
    );
    assert_eq!(violations(&sk, &one(a, b, vec![overlap])), []);
    // Read as a touch, a crossing, or with its far end's parameter wrong.
    for kind in [MeetingKind::Touch, MeetingKind::Crossing] {
        let mut twin = overlap;
        twin.kind = kind;
        assert_eq!(
            violations(&sk, &one(a, b, vec![twin])),
            [V::Kind {
                first: a,
                second: b,
                meeting: 0
            }]
        );
    }
    let mut twin = overlap;
    twin.kind = MeetingKind::Overlap {
        to: [10.0, 0.0],
        to_first: 1.0,
        to_second: 0.9,
    };
    assert_eq!(
        violations(&sk, &one(a, b, vec![twin])),
        [V::Parameter {
            first: a,
            second: b,
            meeting: 0
        }]
    );
    // The far end off the curves.
    let mut twin = overlap;
    twin.kind = MeetingKind::Overlap {
        to: [10.0, 1.0],
        to_first: 1.0,
        to_second: 0.5,
    };
    assert_eq!(
        violations(&sk, &one(a, b, vec![twin]))[0],
        V::OffCurve {
            first: a,
            second: b,
            meeting: 0
        }
    );
}

#[test]
fn a_touch_and_its_twin() {
    // A circle of radius 2 touching the line y = 0 from above, at (1, 0).
    let mut sk = Sketch::new();
    let (p, q, c) = (
        sk.point(-4.0, 0.0).unwrap(),
        sk.point(6.0, 0.0).unwrap(),
        sk.point(1.0, 2.0).unwrap(),
    );
    let line = sk.line(p, q).unwrap();
    let circle = sk.circle(c, 2.0).unwrap();
    let (l, ci) = (line.id(), circle.id());
    // Angle -pi/2 from +x is 3/4 of a turn.
    let touch = meeting(MeetingKind::Touch, [1.0, 0.0], 0.5, 0.75);
    assert_eq!(violations(&sk, &one(l, ci, vec![touch])), []);
    let mut twin = touch;
    twin.kind = MeetingKind::Crossing;
    assert_eq!(
        violations(&sk, &one(l, ci, vec![twin])),
        [V::Kind {
            first: l,
            second: ci,
            meeting: 0
        }]
    );
}

#[test]
fn the_form_of_the_list_is_held() {
    let (sk, a, b, m) = cross_lines(1.0);
    let ok = Intersection {
        first: a,
        second: b,
        meetings: vec![m],
    };
    // Ids descending.
    let swapped = Intersections {
        pairs: vec![Intersection {
            first: b,
            second: a,
            ..ok.clone()
        }],
    };
    assert_eq!(
        violations(&sk, &swapped),
        [V::Order {
            first: b,
            second: a
        }]
    );
    // An empty pair.
    let empty = Intersections {
        pairs: vec![Intersection {
            meetings: vec![],
            ..ok.clone()
        }],
    };
    assert_eq!(
        violations(&sk, &empty),
        [V::Order {
            first: a,
            second: b
        }]
    );
    // A pair twice, in either id order.
    let twice = Intersections {
        pairs: vec![
            ok.clone(),
            Intersection {
                first: b,
                second: a,
                ..ok.clone()
            },
        ],
    };
    assert_eq!(
        violations(&sk, &twice),
        [V::Repeated {
            first: b,
            second: a
        }]
    );
    // Meetings out of order.
    let late = Meeting {
        on_first: 0.75,
        ..m
    };
    let two = Intersections {
        pairs: vec![Intersection {
            meetings: vec![late, m],
            ..ok.clone()
        }],
    };
    assert!(violations(&sk, &two).contains(&V::Order {
        first: a,
        second: b
    }));
}

#[test]
fn a_pair_that_is_not_two_curves_is_named() {
    let mut sk = Sketch::new();
    let p = [(0.0, 0.0), (4.0, 4.0), (0.0, 4.0), (4.0, 0.0)].map(|(x, y)| sk.point(x, y).unwrap());
    let a = sk.line(p[0], p[1]).unwrap().id();
    let b = sk.line(p[2], p[3]).unwrap().id();
    let c = sk.line(p[0], p[3]).unwrap().id();
    sk.set_construction(c, true).unwrap();
    let m = meeting(MeetingKind::Crossing, [2.0, 2.0], 0.5, 0.5);
    let pair = |first, second| one(first, second, vec![m]);
    // A point, an unknown id, the same curve twice, a construction line.
    assert_eq!(
        violations(&sk, &pair(p[0].id(), b)),
        [V::NotACurve { entity: p[0].id() }]
    );
    assert_eq!(
        violations(&sk, &pair(a, EntityId(999))),
        [V::NotACurve {
            entity: EntityId(999)
        }]
    );
    assert_eq!(violations(&sk, &pair(a, a)), [V::NotACurve { entity: a }]);
    assert_eq!(violations(&sk, &pair(a, c)), [V::NotACurve { entity: c }]);
    // A line shorter than a quarter of its tolerance.
    let mut tiny = Sketch::new();
    let (x, y, z, w) = (
        tiny.point(0.0, 0.0).unwrap(),
        tiny.point(1e-13, 0.0).unwrap(),
        tiny.point(-1.0, 1.0).unwrap(),
        tiny.point(1.0, 1.0).unwrap(),
    );
    let (s, l) = (tiny.line(x, y).unwrap().id(), tiny.line(z, w).unwrap().id());
    let found = one(
        s,
        l,
        vec![meeting(MeetingKind::Crossing, [0.0, 0.0], 0.0, 0.5)],
    );
    assert_eq!(violations(&tiny, &found), [V::NotACurve { entity: s }]);
}

#[test]
fn a_missing_or_non_finite_coordinate_leaves_the_pair_unchecked() {
    let (sk, a, b, m) = cross_lines(1.0);
    let found = one(a, b, vec![m]);
    let mut values = sk.coordinates();
    let key = *values.keys().find(|k| k.1 == Coordinate::X).unwrap();
    values.insert(key, f64::NAN);
    let report = check_intersections(&sk, &values, &found);
    assert!(report.violations.is_empty());
    assert_eq!(report.missing.len(), 1);
    values.remove(&key);
    assert_eq!(check_intersections(&sk, &values, &found).missing.len(), 1);
}

#[test]
fn nothing_panics_on_non_finite_or_huge_meetings() {
    let (sk, a, b, m) = cross_lines(1.0);
    for at in [[f64::NAN; 2], [f64::INFINITY, 0.0], [f64::MAX, -f64::MAX]] {
        let found = one(a, b, vec![Meeting { at, ..m }]);
        assert_eq!(
            violations(&sk, &found)[0],
            V::OffCurve {
                first: a,
                second: b,
                meeting: 0
            }
        );
    }
}
