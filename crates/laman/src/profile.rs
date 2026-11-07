//! Profiles: closed loops and their holes, read from a sketch's edges and
//! any coordinates (ADR-0012).
//!
//! Joins are declared, never measured; coordinates give only area,
//! orientation and nesting, through the geometry kernel in `geometry`.

// The extraction that calls the kernel lands in plans/m9-profiles step 4;
// until then only its tests do. `expect` fails the build once it is called.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "called by the extraction, plans/m9-profiles step 4"
    )
)]
mod geometry;
