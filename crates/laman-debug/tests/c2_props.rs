//! C2 families at random small sizes and dimensions, checked against their
//! construction through a static solve and a warm-started dimension edit.
#![allow(clippy::unwrap_used)]

#[path = "support/c2.rs"]
mod support;

use laman_debug::corpus::{rect_chain_with, slot_grid_with, triangle_strip_with};
use proptest::prelude::*;
use support::run_case;

proptest! {
    // No explicit case count: proptest's default and PROPTEST_CASES both apply.
    #[test]
    fn rectangle_construction_and_edit(
        n in 1..=6_usize,
        width in 10.0..40.0_f64,
        height in 5.0..20.0_f64,
    ) {
        let mut case = rect_chain_with(n, width, height).unwrap();
        case.drag = None;
        run_case("property-rectangles", &case);
    }

    #[test]
    fn slot_construction_and_edit(
        rows in 1..=2_usize,
        cols in 1..=2_usize,
        length in 20.0..60.0_f64,
        radius in 3.0..8.0_f64,
    ) {
        let mut case = slot_grid_with(rows, cols, length, radius).unwrap();
        case.drag = None;
        run_case("property-slots", &case);
    }

    #[test]
    fn triangle_construction_and_edit(
        n in 1..=8_usize,
        side in 10.0..30.0_f64,
    ) {
        let case = triangle_strip_with(n, side).unwrap();
        run_case("property-triangles", &case);
    }
}
