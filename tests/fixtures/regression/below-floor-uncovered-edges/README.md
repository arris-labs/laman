# Below the length floor, a branched core gives no profile

A 10 × 10 square and one diagonal, scaled to 1e-299 (lines 4–7 the sides
in order, line 8 the diagonal from point 0 to point 2). No constraints:
the coordinates are the input. Below `LENGTH_FLOOR` (ADR-0003) the length
tolerance is larger than the sketch, so the ends at each corner tie and
are ordered by edge id (ADR-0017, addendum). With these ids the walk makes
one loop of all ten directed edges; it is the outline (ADR-0017 D5), so
`profiles()` reports no profile and no edge, and
`laman_check::check_profiles` names all five `Uncovered`. Drawn in another
order (the diagonal first), the same square gives two profiles.

Found by `crates/laman-debug/tests/c4_profile_props.rs` at 1e-300
(plans/m13-branched-profiles step 7): its fixed subdivisions, at that
scale, leave edges uncovered the same way.

Desired: every edge that is not a bridge is in some reported loop, and
`check_profiles` is clean on what `profiles()` returns (ADR-0017,
Consequences: each such edge enters two loops, at most one of them the
outline). Which loops come out below the floor stays unspecified.
