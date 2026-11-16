# ADR-0014: Profile extraction has a 1.5 ms budget at 500 entities

- Status: accepted
- Date: 2026-11-08
- Scope: C3 extraction budget (`plans/m10-profile-check`)

## Context

C3's *Accept* left the cost of extraction at 500 entities open, to be named
by the stop rule of ADR-0011 when the first plan measured it. Extraction
(`laman::profile::profiles`) reads declarations and solved coordinates and
does not solve, so it has no algorithm and no tries; it runs once per
solved sketch, not once per frame. Its work is a union-find over the
`coincident` constraints, a walk of the edges, a measure per loop, and
containment: each loop's probe point against every other loop's boundary,
quadratic in the loops.

## Decision

**The number is 1.5 ms**, the worst of three runs rounded up to the next
0.5 ms, for every 500-entity cell of `bench_c2`'s `profiles` operation: the
rectangle chain (498 entities), the slot grid (498), the triangle strip
(501) and a row of 249 disjoint circles (498, built in the bench alone:
the shape the quadratic containment is worst on, every loop a candidate
parent of every other). Measured worst samples in ms, worst of three
runs: rectangles 0.123, slots 0.594, triangles 0.138, circles 1.394. Every
smaller cell is below the 500-entity one of its family.

The 4 ms frame is not the yardstick; extraction is not a frame step. It
fits inside one anyway, so no profile-led pass is made (the plan's step 10
was not needed) and nothing in extraction changes for the budget.

## Consequences

- C3's *Accept* carries 1.5 ms at 500 entities.
- A later change that moves a cell past 1.5 ms is a regression to be
  explained, not a number to widen.
- The bench's `profiles` cells are reported with the others and run as a
  separate worker per family and size, the same 5 s wall limit.

## Alternatives considered

- **Hold extraction to the 4 ms frame**: the looser number, with room for
  a ten-fold slowdown unnoticed.
- **A spatial index for containment**: the circle row would be linear in
  the loops; not built, since the quadratic case is 1.4 ms and the stop
  rule asks for no more.

## Addendum: M14 measured extraction after the split, and intersections (2026-11-16)

`bench_c2` gained the `cell_grid` family (61 and 493 entities, the shared
walls and a slot hole per cell) and an `intersections` operation beside
`profiles`; `Classes::of` is built once, not once per core. Worst samples
in ms, worst of three runs, same machine as above, the cells at about 500
entities:

| cell | profiles before M13 | profiles now | intersections now |
|---|---|---|---|
| rectangles 498 | 0.122 | 1.165 | 4.688 |
| slots 498 | 0.589 | 1.668 | 3.138 |
| triangles 501 | 0.099 | 1.519 | 5.814 |
| circles 498 | 1.468 | 1.973 | 2.708 |
| cells 493 | | 1.623 | 3.837 |

- **Profiles: the number is now 2.0 ms**, the worst of three rounded up
  to the next 0.5 ms, by the stop rule above. M13's split costs: the
  rectangle and triangle cells were a tenth of a millisecond before it
  (the figures "before" were measured at the last commit of M12, with the
  same bench). The split's bookkeeping is per edge and per join and grows
  linearly with the entities (rectangles 0.17, 0.5 and 1.1 ms at 98, 248
  and 498 entities); it is not the containment, whose worst cell, the
  circle row, moved from 1.4 to 2.0. Nothing is changed for the budget:
  2.0 ms is inside the 4 ms of a frame. This is a regression explained,
  not a number widened to hide one: a cell past 2.0 ms is to be explained
  again.
- **Intersections share the 1.5 ms**, the number extraction had: the same
  shape (a read of declarations and solved coordinates, no solve, once per
  solved sketch), and the roadmap's "inside ADR-0014's stop rule" says a
  number, not a second one. Measured above it, 5.8 ms at its worst: every
  pair of non-construction curves is tried (125 000 at 500 entities, about
  40 ns a pair), so the cost is quadratic. The pruning of D4 of
  `plans/m14-check-digest-python` follows (a sweep over bounding boxes
  widened by `L`, bit-equal to all pairs); its measurement is recorded
  below when it lands, and only then does intersections' number stand.

**The sweep landed** (`plans/m14-check-digest-python` step 8): boxes widened
by four length tolerances, sorted by their left edge, bit-equal to all
pairs. Worst samples in ms, worst of three runs: rectangles 0.636, slots
0.781, triangles 0.599, circles 0.666, cells 0.888. **Intersections stand at
1.5 ms**, the number extraction had; a cell past it is a regression to be
explained, not a number to widen.
