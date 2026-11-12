//! Intersections: where two curves of a sketch meet without a shared point
//! or a declared join, measured and classified, never joined (ADR-0016).
//!
//! The pair-by-pair geometry is the kernel in `kernel`.

// The operation that calls the kernel lands in plans/m12-intersections
// step 6; until then only its tests do. `expect` fails the build once it is
// called.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "called by intersections(), plans/m12-intersections step 6"
    )
)]
mod kernel;
