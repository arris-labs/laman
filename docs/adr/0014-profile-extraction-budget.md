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
