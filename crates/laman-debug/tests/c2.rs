//! The generated C2 corpus: expectations come from construction, never a solve.
#![allow(clippy::unwrap_used)]

#[path = "support/c2.rs"]
mod support;

use laman::solution::{Algorithm, SolverOptions};
use laman_check::check;
use laman_debug::corpus::{bisecting_drag, rect_chain, slot_grid, triangle_strip};
use support::run_case;

#[test]
fn rect_chain_three() {
    let case = rect_chain(3).unwrap();
    assert_eq!(case.sketch.entities().count(), 18);
    run_case("rect-chain-3", &case);
    // The edit and each drag expectation satisfy the geometry independently.
    for step in &case.drag.as_ref().unwrap().drag.after {
        assert!(check(&case.drag.as_ref().unwrap().sketch, &step.coordinates).is_clean());
    }
}

#[test]
fn slot_grid_two_by_two() {
    let case = slot_grid(2, 2).unwrap();
    assert_eq!(case.sketch.entities().count(), 48);
    // C1's local slot coordinates are (40,30), (40,35), (40,25),
    // (80,30), (80,25), (80,35), translated into each grid cell.
    for i in 0..4 {
        let mut centres = Vec::new();
        for side in ["left", "right"] {
            let tag = format!("slot-{i}-{side}");
            let (_, entity) = case
                .sketch
                .entities()
                .find(|(id, _)| case.sketch.tag_of(*id) == Some(tag.as_str()))
                .unwrap();
            assert!(matches!(entity, laman::model::Entity::Arc { .. }));
            if let laman::model::Entity::Arc { c, s, e } = entity {
                let xy = |p: laman::model::Point| {
                    [
                        case.expected.coordinates[&(p.id(), laman::model::Coordinate::X)],
                        case.expected.coordinates[&(p.id(), laman::model::Coordinate::Y)],
                    ]
                };
                let centre = xy(*c);
                let (start, end) = (xy(*s), xy(*e));
                let sign = if side == "left" { 1.0 } else { -1.0 };
                assert_eq!(start, [centre[0], centre[1] + sign * 5.0]);
                assert_eq!(end, [centre[0], centre[1] - sign * 5.0]);
                centres.push(centre);
            }
        }
        assert_eq!(centres[1], [centres[0][0] + 40.0, centres[0][1]]);
    }
    run_case("slot-grid-2-2", &case);
    for step in &case.drag.as_ref().unwrap().drag.after {
        assert!(check(&case.drag.as_ref().unwrap().sketch, &step.coordinates).is_clean());
    }
}

#[test]
fn triangle_strip_six_and_its_rigid_tail_edit() {
    let case = triangle_strip(6).unwrap();
    assert_eq!(case.sketch.entities().count(), 21);
    run_case("triangle-strip-6", &case);
    assert!(case.drag.is_none());
}

// Nominal entity sizes use the closest construction counts: 5n+3,
// 10rc+8, and 3n+3 cannot all equal 100, 250, or 500 exactly.
macro_rules! large_case {
    ($name:ident, $case:expr, $entities:expr) => {
        #[test]
        fn $name() {
            let case = $case.unwrap();
            assert_eq!(case.sketch.entities().count(), $entities);
            run_case(stringify!($name), &case);
        }
    };
}

large_case!(rect_chain_100, rect_chain(19), 98);
large_case!(rect_chain_250, rect_chain(49), 248);
large_case!(rect_chain_500, rect_chain(99), 498);
large_case!(slot_grid_100, slot_grid(3, 3), 98);
large_case!(slot_grid_250, slot_grid(4, 6), 248);
large_case!(slot_grid_500, slot_grid(7, 7), 498);
large_case!(triangle_strip_100, triangle_strip(32), 99);
large_case!(triangle_strip_250, triangle_strip(82), 249);
large_case!(triangle_strip_500, triangle_strip(166), 501);

/// The bench's bisect cell makes three tries, two halvings, under both
/// algorithms: the frame's cost is its tries, so a damping that lets a try
/// swing the arc further shows here first (plans/m8-pursuit step 1).
#[test]
fn the_bisect_cell_halves_twice() {
    let case = slot_grid(7, 7).unwrap();
    let (sketch, dragged, target) = bisecting_drag(&case).unwrap().unwrap();
    for algorithm in [Algorithm::LevenbergMarquardt, Algorithm::DogLeg] {
        let mut options = SolverOptions::default();
        options.algorithm = algorithm;
        let frame = sketch.drag(dragged, &options).unwrap().to(target);
        let bisection = frame.diagnostics.bisection.unwrap();
        assert_eq!(bisection.halvings, 2, "{algorithm:?}");
        assert!(frame.diagnostics.flips.is_empty(), "{algorithm:?}");
    }
}
