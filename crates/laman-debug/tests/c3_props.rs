//! Profiles of random nests of rectangles, circles and slots, built by
//! construction (plans/m9-profiles step 8). Shapes are concentric chains,
//! each inside the last's inscribed box, so depth is known without a
//! measure; chains sit side by side, disjoint. Every corner is a shared
//! point or two points joined by `coincident`, every line is drawn in a
//! random direction, and the whole sketch is rotated, scaled and translated.
//! Loop counts, depth parity, orientation, `reversed`, joins and areas are
//! asserted against the construction. A failure is reduced by proptest and
//! committed as a fixture under `tests/fixtures/regression/` with the desired
//! assertion and an `#[ignore]` (`.agents/rules/solver.md`).
#![allow(clippy::unwrap_used, clippy::expect_used)] // tests: a failing unwrap is the test failing (clippy.toml)

use laman::model::{ConstraintId, EntityId, Point, Sketch};
use laman::profile::profiles;
use proptest::prelude::*;
use std::f64::consts::PI;

/// How close an area must be, relative to the plate's (outermost) area. The
/// shapes are built at coordinates the transform rounds by a few ulps, and
/// translations up to a thousand times the extent keep their cross products
/// to about 1e-13 of it.
const AREA_RELATIVE: f64 = 1e-9;

#[derive(Debug, Clone, Copy)]
enum Shape {
    Rectangle,
    Circle,
    Slot,
}

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
}

/// What one built shape must produce.
struct Built {
    area: f64,
    edges: usize,
    first: EntityId,
    /// Whether the first edge is drawn against the counter-clockwise
    /// direction: only a rectangle's line can be. A counter-clockwise loop
    /// then starts reversed, and a clockwise one (a hole) does not.
    against: bool,
    joins: Vec<ConstraintId>,
}

/// Draws one shape centred at `centre` with the bounding half extents
/// `(hw, hh)`, returns its area in design units and the half extents of the
/// box wholly inside it. `flips` picks each line's drawing direction and
/// `coincident` whether each corner is two points joined or one.
fn build(
    sk: &mut Sketch,
    t: &Transform,
    shape: Shape,
    centre: [f64; 2],
    (hw, hh): (f64, f64),
    flips: &[bool],
    coincident: &[bool],
) -> (Built, (f64, f64)) {
    let at = |dx: f64, dy: f64| t.at([centre[0] + dx, centre[1] + dy]);
    let scale = t.scale;
    match shape {
        Shape::Circle => {
            let r = hw.min(hh);
            let c = sk.point(at(0.0, 0.0)[0], at(0.0, 0.0)[1]).unwrap();
            let circle = sk.circle(c, r * scale).unwrap();
            let built = Built {
                area: PI * r * r,
                edges: 1,
                first: circle.id(),
                against: false,
                joins: vec![],
            };
            let inner = r * std::f64::consts::FRAC_1_SQRT_2;
            (built, (inner, inner))
        }
        Shape::Rectangle => {
            let corners = [(-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh)];
            let mut joins = Vec::new();
            // Each corner has an end for the line before it and one for the
            // line after: the same point, or two joined by `coincident`.
            let mut before = Vec::new();
            let mut after = Vec::new();
            for (k, &(dx, dy)) in corners.iter().enumerate() {
                let [x, y] = at(dx, dy);
                let p = sk.point(x, y).unwrap();
                let q = if coincident[k] {
                    let q = sk.point(x, y).unwrap();
                    joins.push(sk.coincident(p, q).unwrap());
                    q
                } else {
                    p
                };
                before.push(p);
                after.push(q);
            }
            let mut first = None;
            for k in 0..4 {
                let (a, b) = (after[k], before[(k + 1) % 4]);
                let line = if flips[k] {
                    sk.line(b, a).unwrap()
                } else {
                    sk.line(a, b).unwrap()
                };
                first.get_or_insert(line.id());
            }
            let built = Built {
                area: 4.0 * hw * hh,
                edges: 4,
                first: first.unwrap(),
                against: flips[0],
                joins,
            };
            (built, (hw, hh))
        }
        Shape::Slot => {
            let r = hh.min(0.4 * hw);
            let half = hw - r; // half the centre separation
            let pt = |sk: &mut Sketch, dx: f64, dy: f64| -> Point {
                let [x, y] = at(dx, dy);
                sk.point(x, y).unwrap()
            };
            let (c1, c2) = (pt(sk, -half, 0.0), pt(sk, half, 0.0));
            // Left arc from its top to its bottom, right arc from bottom to
            // top: both counter-clockwise. Each arc end has a twin for the
            // line that meets it.
            let ends = [
                (-half, r),  // left start, top
                (-half, -r), // left end, bottom
                (half, -r),  // right start, bottom
                (half, r),   // right end, top
            ];
            let mut arc_ends = Vec::new();
            let mut line_ends = Vec::new();
            let mut joins = Vec::new();
            for (k, &(dx, dy)) in ends.iter().enumerate() {
                let p = pt(sk, dx, dy);
                let q = if coincident[k] {
                    let q = pt(sk, dx, dy);
                    joins.push(sk.coincident(p, q).unwrap());
                    q
                } else {
                    p
                };
                arc_ends.push(p);
                line_ends.push(q);
            }
            let left = sk.arc(c1, arc_ends[0], arc_ends[1]).unwrap();
            sk.arc(c2, arc_ends[2], arc_ends[3]).unwrap();
            // Bottom line from the left arc's end to the right arc's start,
            // top line from the right arc's end to the left arc's start.
            let (bottom, top) = ((line_ends[1], line_ends[2]), (line_ends[3], line_ends[0]));
            for (k, (a, b)) in [bottom, top].into_iter().enumerate() {
                if flips[k] {
                    sk.line(b, a).unwrap();
                } else {
                    sk.line(a, b).unwrap();
                }
            }
            let built = Built {
                area: 2.0 * (2.0 * half) * r + PI * r * r,
                edges: 4,
                first: left.id(),
                against: false,
                joins,
            };
            (built, (half, r))
        }
    }
}

/// One chain: its shapes from the outermost in, each shrunk by `factors`
/// into the last one's inscribed box.
#[derive(Debug, Clone)]
struct Chain {
    shapes: Vec<(Shape, f64, [bool; 4], [bool; 4])>,
    half: (f64, f64),
}

fn shape() -> impl Strategy<Value = Shape> {
    prop_oneof![
        Just(Shape::Rectangle),
        Just(Shape::Circle),
        Just(Shape::Slot)
    ]
}

fn chain() -> impl Strategy<Value = Chain> {
    (
        proptest::collection::vec(
            (
                shape(),
                0.3..0.7_f64,
                any::<[bool; 4]>(),
                any::<[bool; 4]>(),
            ),
            1..=4,
        ),
        // A slot needs width over its height; a box too narrow for it is
        // widened, so the aspect ratio stays between 1.25 and 3.
        (8.0..16.0_f64, 1.25..3.0_f64),
    )
        .prop_map(|(shapes, (hh, aspect))| Chain {
            shapes,
            half: (hh * aspect, hh),
        })
}

fn transform() -> impl Strategy<Value = Transform> {
    (
        0.0..(2.0 * PI),
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

/// Builds the chains left to right and checks `profiles` against them.
fn check(chains: &[Chain], t: &Transform) -> Result<(), TestCaseError> {
    let mut sk = Sketch::new();
    // Each loop's expected facts, in construction order: chain, then depth.
    let mut expected: Vec<Vec<Built>> = Vec::new();
    let mut x = 0.0;
    for chain in chains {
        let centre = [x + chain.half.0, 0.0];
        x += 2.0 * chain.half.0 + 4.0;
        let mut half = chain.half;
        let mut built = Vec::new();
        for &(kind, factor, flips, coincident) in &chain.shapes {
            let (b, inner) = build(&mut sk, t, kind, centre, half, &flips, &coincident);
            built.push(b);
            half = (inner.0 * factor, inner.1 * factor);
        }
        expected.push(built);
    }
    let found = profiles(&sk, &sk.coordinates()).unwrap();
    prop_assert!(found.open_chains.is_empty());
    prop_assert!(found.dangling.is_empty());
    prop_assert!(found.branched.is_empty());
    prop_assert!(found.degenerate.is_empty());
    prop_assert!(found.unjoined.is_empty());
    // The checker's own measure, nesting included, agrees with the report.
    let report = laman_check::check_profiles(&sk, &sk.coordinates(), &found);
    prop_assert!(report.is_clean(), "{report:#?}");

    let s2 = t.scale * t.scale;
    let mut at = found.profiles.iter();
    for built in &expected {
        // Even depths are outer loops, each holding the odd one after it.
        for pair in built.chunks(2) {
            let profile = at.next();
            prop_assert!(profile.is_some(), "a profile is missing");
            let profile = profile.unwrap();
            let outer = &pair[0];
            let plate = outer.area * s2;
            let near = |found: f64, want: f64| (found - want).abs() <= AREA_RELATIVE * plate;
            prop_assert_eq!(profile.outer.edges.len(), outer.edges);
            prop_assert_eq!(profile.outer.edges[0].entity, outer.first);
            prop_assert_eq!(profile.outer.edges[0].reversed, outer.against);
            prop_assert!(near(profile.outer.signed_area, plate));
            prop_assert_eq!(&profile.outer.joins, &outer.joins);
            let mut net = plate;
            prop_assert_eq!(profile.holes.len(), pair.len() - 1);
            for (hole, want) in profile.holes.iter().zip(&pair[1..]) {
                let area = want.area * s2;
                prop_assert_eq!(hole.edges.len(), want.edges);
                prop_assert_eq!(hole.edges[0].entity, want.first);
                prop_assert_eq!(hole.edges[0].reversed, !want.against);
                prop_assert!(hole.signed_area < 0.0);
                prop_assert!(near(-hole.signed_area, area));
                prop_assert_eq!(&hole.joins, &want.joins);
                net -= area;
            }
            prop_assert!(near(profile.net_area, net));
        }
    }
    prop_assert!(at.next().is_none(), "more profiles than the construction");
    Ok(())
}

proptest! {
    // No explicit case count: proptest's default and PROPTEST_CASES both apply.
    #[test]
    fn nests_of_shapes_come_out_as_built(
        chains in proptest::collection::vec(chain(), 1..=3),
        t in transform(),
    ) {
        check(&chains, &t)?;
    }
}

/// The same plate with a slotted hole and a circle inside it, drawn clockwise
/// and counter-clockwise, at `scale` with its centre `scale` away from the
/// origin: the structure and every sign survive where the areas overflow or
/// underflow, and no area is ever non-finite.
fn extreme(scale: f64, clockwise: bool) {
    let mut sk = Sketch::new();
    let p =
        |sk: &mut Sketch, x: f64, y: f64| sk.point(scale * (1.0 + x), scale * (1.0 + y)).unwrap();
    let mut corners = [(0.0, 0.0), (10.0, 0.0), (10.0, 6.0), (0.0, 6.0)];
    if clockwise {
        corners.reverse();
    }
    let pts: Vec<Point> = corners.iter().map(|&(x, y)| p(&mut sk, x, y)).collect();
    for k in 0..4 {
        sk.line(pts[k], pts[(k + 1) % 4]).unwrap();
    }
    let (c1, c2) = (p(&mut sk, 3.0, 3.0), p(&mut sk, 7.0, 3.0));
    let (a, b, c, d) = (
        p(&mut sk, 3.0, 4.0),
        p(&mut sk, 3.0, 2.0),
        p(&mut sk, 7.0, 2.0),
        p(&mut sk, 7.0, 4.0),
    );
    sk.arc(c1, a, b).unwrap();
    sk.arc(c2, c, d).unwrap();
    sk.line(b, c).unwrap();
    sk.line(d, a).unwrap();
    let centre = p(&mut sk, 5.0, 3.0);
    sk.circle(centre, 0.5 * scale).unwrap();
    let found = profiles(&sk, &sk.coordinates()).unwrap();
    assert_eq!(found.profiles.len(), 2, "scale {scale:e}");
    let plate = &found.profiles[0];
    assert_eq!(plate.holes.len(), 1);
    assert_eq!(plate.outer.edges.len(), 4);
    assert_eq!(plate.holes[0].edges.len(), 4);
    assert_eq!(plate.outer.edges[0].reversed, clockwise);
    assert!(plate.holes[0].edges[0].reversed);
    assert_eq!(found.profiles[1].outer.edges.len(), 1);
    assert!(!found.profiles[1].outer.edges[0].reversed);
    for area in [
        plate.outer.signed_area,
        plate.holes[0].signed_area,
        plate.net_area,
        found.profiles[1].outer.signed_area,
    ] {
        assert!(area.is_finite(), "scale {scale:e}: {area}");
    }
    assert!(plate.outer.signed_area >= 0.0 && plate.holes[0].signed_area <= 0.0);
    assert!(found.profiles[1].outer.signed_area >= 0.0);
    if scale > 1.0 {
        // The areas saturate at the largest finite value, keeping signs.
        assert_eq!(plate.outer.signed_area, f64::MAX);
        assert_eq!(plate.holes[0].signed_area, -f64::MAX);
        // The net is summed before it saturates: the plate less the slot is
        // still far above f64::MAX, not the 0 of two saturated terms
        // cancelling.
        assert_eq!(plate.net_area, f64::MAX, "scale {scale:e}");
    } else {
        assert!(plate.net_area >= 0.0, "scale {scale:e}");
    }
}

#[test]
fn extreme_scales_keep_the_structure_and_never_overflow() {
    for scale in [1e300, 1e-300] {
        for clockwise in [false, true] {
            extreme(scale, clockwise);
        }
    }
}
