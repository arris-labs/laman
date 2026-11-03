//! Regressions for impossible corpus sizes, refused before building geometry.

use laman_debug::corpus::{
    CorpusError, MAX_CONSTRUCTION_ITEMS, rect_chain, rect_chain_with, slot_grid, slot_grid_with,
    triangle_strip, triangle_strip_with,
};

#[test]
fn triangle_counts_refuse_before_allocating_positions() {
    // MAX - 2 permits n + 2 but overflows Vec<[f64; 2]>'s capacity.
    // The following two counts also overflow the point-count addition.
    for n in [usize::MAX - 2, usize::MAX - 1, usize::MAX] {
        assert!(matches!(
            triangle_strip(n),
            Err(CorpusError::Count("triangles"))
        ));
        assert!(matches!(
            triangle_strip_with(n, 30.0),
            Err(CorpusError::Count("triangles"))
        ));
    }
}

#[test]
fn rectangle_counts_refuse_before_building_entities() {
    for n in [usize::MAX / 5, usize::MAX - 2, usize::MAX] {
        assert!(matches!(
            rect_chain(n),
            Err(CorpusError::Count("rectangles"))
        ));
        assert!(matches!(
            rect_chain_with(n, 30.0, 12.0),
            Err(CorpusError::Count("rectangles"))
        ));
    }
}

#[test]
fn slot_counts_refuse_product_and_derived_size_overflow() {
    for (rows, cols) in [
        (usize::MAX, 2),
        (2, usize::MAX),
        (usize::MAX, usize::MAX),
        (1, usize::MAX / 10),
        (1, usize::MAX - 2),
    ] {
        assert!(matches!(
            slot_grid(rows, cols),
            Err(CorpusError::Count("slot count"))
        ));
        assert!(matches!(
            slot_grid_with(rows, cols, 40.0, 8.0),
            Err(CorpusError::Count("slot count"))
        ));
    }
}

#[test]
fn counts_refuse_the_first_size_outside_model_id_domains() {
    // Minted ids cover 0..=u32::MAX. All three families exhaust entity
    // ids first: 5n+3 rectangles, 3n+3 triangles, and 10rc+8 slots.
    let ids = u64::from(u32::MAX) + 1;
    let rectangles = ((ids - 3) / 5 + 1) as usize;
    let triangles = ((ids - 3) / 3 + 1) as usize;
    let slots = ((ids - 8) / 10 + 1) as usize;
    assert!(matches!(
        rect_chain(rectangles),
        Err(CorpusError::Count("rectangles"))
    ));
    assert!(matches!(
        rect_chain_with(rectangles, 30.0, 12.0),
        Err(CorpusError::Count("rectangles"))
    ));
    assert!(matches!(
        triangle_strip(triangles),
        Err(CorpusError::Count("triangles"))
    ));
    assert!(matches!(
        triangle_strip_with(triangles, 30.0),
        Err(CorpusError::Count("triangles"))
    ));
    assert!(matches!(
        slot_grid(1, slots),
        Err(CorpusError::Count("slot count"))
    ));
    assert!(matches!(
        slot_grid_with(1, slots, 40.0, 8.0),
        Err(CorpusError::Count("slot count"))
    ));
}

#[test]
fn counts_that_fit_ids_but_not_allocation_are_refused() {
    // Both fit u32 ids; without the item cap, wasm32 panics on the position
    // vector and 64-bit tries ~23 GB. Refusal must precede any allocation.
    for n in [200_000_000, 1_431_655_764] {
        assert!(matches!(
            triangle_strip(n),
            Err(CorpusError::Count("triangles"))
        ));
        assert!(matches!(
            triangle_strip_with(n, 30.0),
            Err(CorpusError::Count("triangles"))
        ));
    }
    let past_cap = MAX_CONSTRUCTION_ITEMS / 3 + 1;
    assert!(matches!(
        triangle_strip(past_cap),
        Err(CorpusError::Count("triangles"))
    ));
    assert!(matches!(
        rect_chain(MAX_CONSTRUCTION_ITEMS / 5 + 1),
        Err(CorpusError::Count("rectangles"))
    ));
    assert!(matches!(
        slot_grid(1, MAX_CONSTRUCTION_ITEMS / 10 + 1),
        Err(CorpusError::Count("slot count"))
    ));
}
