# Corpus count overflow

`triangle_strip(usize::MAX - 2)` panicked with `capacity overflow` while
collecting point positions: adding two did not overflow, but the positions
could not fit a vector or the model's entity ids. No representable sketch
exists for this input, so this fixture records constructor parameters
instead of sketch JSON. Counts that fit the model's ids but whose
vectors cannot be allocated (`triangle_strip(200_000_000)` on wasm32,
`triangle_strip(1_431_655_764)` on 64-bit, about 23 GB) are the same defect.

`crates/laman-debug/tests/c2_counts.rs` asserts the desired typed refusal,
`CorpusError::Count`, through both default and `_with` constructors. Its
inputs include the three largest `usize` triangle counts, rectangle size
overflow, slot product/size overflow, and the first count outside the
model's `0..=u32::MAX` entity id domain for each family, and counts past
`MAX_CONSTRUCTION_ITEMS` (2^22 entities or constraints, the one documented
maximum all families share). The latter come
from the construction totals `5n + 3`, `3n + 3`, and `10rc + 8`, never from
solver output. All refusals happen before construction or allocation: the cap bounds
every per-count vector.
