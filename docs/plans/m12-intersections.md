# Plan: m12-intersections

- Started: 2026-11-13
- Milestone: M12 (cycle C4, docs/ROADMAP.md), the first of three
- Idea: none; verbatim from the human: "/plan C4" (the roadmap's C4 section
  as revised in 1aac738)

## Goal

A tangent contact joined by `coincident` solves to the coordinate tolerance,
as a shared point does: regression/`coincident-contact-drift` is lifted and
`c3/slotted-plate-coincident` checks its solved coordinates.
`laman::intersect::intersections(&Sketch, &Coordinates)` reports every place
where two non-construction lines, arcs or circles meet without a shared
point or a declared join: each pair named by its entities, each meeting
classified as a crossing, a touch or an overlap, placed by its parameter
along both edges, an arc's meetings bounded by its sweep. Every C4
intersection fixture comes out at its closed form, at scales 1e-6 to 1e6
and near 1e±300.

## Non-goals

- **M13** (next plan): profiles of a branched core: edges at a join
  ordered by tangent direction then curvature, one profile per bounded
  area, an edge that bounds a profile on both sides; the C4 profile
  fixtures (shared wall, window, tangent slot, figure eight, slot grid with
  shared walls).
- **M14**: `laman-check`'s independent measure of intersections and of
  branched profiles, with broken twins; the digest over both, native and
  wasm32-wasip1; the 500-entity cost under ADR-0014's stop rule; `laman-py`
  over the new API with stubs, pytest reproducing every number.
- Any change to the sketch (C5): an intersection joins nothing (ADR-0012).
- Ellipses, splines, booleans; a spatial index (M14 measures first).

## Design deltas

**D1 — ADR-0015, contacts through join classes (amends ADR-0002).**
ADR-0002 recognizes a tangent contact only from shared ids, so a line end
joined to an arc end by `coincident` keeps the raw tangent equation, whose
root is double: the contact slides along the circle by √(residual · r)
(3.7e-5 on the fixture). The planned decision: recognize the contact when
the line end is in the arc end's join class (ADR-0012's union-find over
`coincident`), and regularize it as ADR-0002 does. The proof carries over
with membership holding through the `coincident`. Fallback if the zero sets
or the side domain do not carry over: a last step on the regularized
equation before acceptance. *Settled* in step 1 (ADR-0015): recognition
through join classes, shared id first so no existing contact changes; the
last step is recorded as rejected.

**D2 — ADR-0016, "Intersections are measured, classified, never joined".**
Settled in step 3 (ADR-0016 holds the detail; it refines what follows: a
round's frame origin is its centre, a degenerate line or round meets
nothing, a touch is one point at the foot or on the line of centres):
- *Which pairs*: non-construction lines, arcs and circles, each unordered
  pair once, the smaller id first. A meeting within the length tolerance
  of an end the two edges share by id or by join class is that join, not
  an intersection; other meetings of the same pair are reported.
- *Kinds*: a **crossing** (simple root), a **touch** (`|d − r|`, or
  `|d − (r₁ ± r₂)|`, within the length tolerance, reported as one point),
  an **overlap** (collinear lines, or concentric circles of one radius,
  sharing a stretch longer than the length tolerance, reported as its two
  ends). Tolerances come from `Tolerances::at` (ADR-0004); no new constant.
- *An end on another edge* (a T, or an arc end on a line): classified by
  the same tests, a crossing or a touch, its parameter on its own curve 0
  or 1 and its point the end's own (ADR-0016 D4).
- *Below the floor*: `L` is `Tolerances::at`'s, floor included, so at
  1e-300 `intersections()` classifies at ADR-0003's floor (everything a
  touch or an overlap); the 1e-300 closed forms are held in the kernel,
  which takes `L` as an input (ADR-0016 D2).
- *Arcs*: a meeting of the arc's circle counts when its angle lies in the
  sweep, widened at each end by the length tolerance over the radius.
- *Order*: each meeting carries its parameter on both edges (a line's `t`
  in [0, 1] from `a`, an arc's fraction of its sweep from its start, a
  circle's angle from +x over 2π). The meetings on one edge are ordered by
  parameter, ties by the other edge's id.
- *Numerics*: each pair is measured in a frame translated to the first
  edge's start and scaled by an exact power of two, as in ADR-0012 D3, so
  nothing overflows near 1e±300 and a small pair far from the origin keeps
  its digits. Transcendentals through `libm`.

**D3 — Public types (design delta, CHANGELOG).** In `laman::model::intersection`,
so the checker can name them in M14: `Intersections { pairs: Vec<Intersection> }`,
`Intersection { first, second: EntityId, meetings: Vec<Meeting> }`,
`Meeting { kind: MeetingKind, at: [f64; 2], on_first, on_second: f64 }`,
`MeetingKind::{Crossing, Touch, Overlap { to: [f64; 2], to_first, to_second: f64 }}`,
and `Intersections::along(EntityId) -> Vec<…>` for the order on one edge.
Names and the `along` signature are settled in step 6. The operation is
`laman::intersect::intersections(&Sketch, &Coordinates) -> Result<Intersections,
IntersectError>`, with `Missing` and `NonFinite` naming the entity as
`ProfileError` does. No existing signature changes.

**D4 — Fixtures.** New home `tests/fixtures/c4/<slug>/`; `expected.json`
gains an `intersections` section read by `laman_debug::fixture`: per pair,
the ids and each meeting's kind, point and parameters, with a tolerance
whose README cites ADR-0016. Every number from a closed form.

## Steps

Grades: **[1]** routine, the design says exactly what to write; **[2]**
careful, a numeric or geometric case within the design; **[3]** unproven,
an algorithm whose robustness or bound is established here.

- [x] Step 1 **[3]** — D1: reproduce `coincident-contact-drift` with
  `inspect`, decide recognition through join classes or the last step,
  ADR-0015, implement it with a finite-difference test of the new contact
  gradient, lift the ignore.
- [x] Step 2 **[2]** — `c3/slotted-plate-coincident` checks its solved
  coordinates at the shared-point plate's accuracy. Re-run every digest;
  any that moves is re-blessed in its own `fixtures:` commit saying why.
  *Settled* in step 1: the gate fails on a moved digest, so its two solved
  digests (`c3-solved-{lm,dogleg}-slotted-plate-coincident`, the only ones
  that moved) are re-blessed in step 1's commit; step 2 re-runs the rest
  (wasm32 included) and adds the coordinate check.
- [x] Step 3 **[3]** — ADR-0016 (D2), the end-on-edge case included.
  GLOSSARY: *Intersection* cites it; *Meeting* added if step 6 keeps it.
- [x] Step 4 **[3]** — The kernel: line–line, line–circle, circle–circle
  in the D2 frame, classified. Closed-form tests at scales 1e-6 to 1e6,
  translations to 1e4 times the scale, magnitudes near 1e±300, and pairs
  near tangency on both sides of the tolerance. *Settled* here: the kernel
  (`intersect/kernel.rs`) classifies a pair whole, so it already holds the
  line–line overlap clipped to both lines, the coincident circles, the
  parameters on lines and circles and D4's end snap; ADR-0012 D3's frame
  moved to `crate::frame`, shared with the profile geometry. A line's root
  counts when within `L` of the segment along it.
- [ ] Step 5 **[2]** — Arcs: sweep bounds widened by `L/r`, an arc's
  parameter and ends (D4), overlaps of arcs clipped to both sweeps (two
  stretches possible).
- [ ] Step 6 **[2]** — D3 types and `intersections()`: pairs, the join
  exclusion, `IntersectError`, `along`, rustdoc with runnable examples, a
  fuzz target, CHANGELOG `Unreleased` bullet.
- [ ] Step 7 **[2]** — D4 fixture form and the C4 intersection corpus, each
  solved, checked by `laman-check`, then intersected: two crossing lines; a
  line through a circle; two circles at two points; a tangent touch; a
  collinear overlap; an arc whose circle crosses a line outside its sweep
  (no intersection); a T whose end lies on a line.
- [ ] Step 8 **[3]** — A property test: random lines, circles and arcs
  built through chosen meeting points (and chosen tangencies and overlaps),
  rotated, scaled and translated; kinds, counts, points and order against
  the construction; one 1e±300 case. Shrinks into `regression/`.

## Acceptance

`cargo test` on `tests/fixtures/c4/*` and the lifted
`regression/coincident-contact-drift`. Coincident slotted plate: its eight
contacts within the default coordinate tolerance of the closed form, as the
shared-point plate. Crossing lines: one crossing at the constructed point
with both `t` from the construction. Line through a circle: two crossings,
ordered along the line. Two circles: two crossings at the closed-form
points. Tangent touch: one touch at the foot of the perpendicular. Collinear
overlap: one overlap, its stretch the closed form. Arc outside its sweep:
no pair. Step 8 passes, 1e±300 included. C1 and C3 digests unchanged except
those step 2 re-blesses.

## Docs to update on completion

- `docs/ARCHITECTURE.md`: §Solve's contact paragraph (join classes,
  ADR-0015); a new §Intersections (D2, the module, the frame); fuzzer
  targets.
- `docs/DATA-MODEL.md`: a new §Intersections (D3, parameters, kinds).
- `docs/ROADMAP.md` §C4: confirmed (no longer "proposed"); M12 recorded as
  landed in the in-list.
- `GLOSSARY.md`: *Intersection* cites ADR-0016; *Meeting* if kept.
- `AGENTS.md` current state: C4 under way, M12 landed, next M13.
- `docs/BACKLOG.md`: drop the tangent double-root line if one remains.

## Open questions

- *Settled* in step 3 (ADR-0016 D4): an end that lies on another edge's
  interior (a T) is a crossing when the curves' directions differ there
  and a touch when they are tangent, the meeting's parameter at the end 0
  or 1; not a kind of its own, since C5 splits it as any meeting.
- *Settled* in step 3 (ADR-0016 D2): "near 1e±300" holds the closed form
  at 1e300 everywhere and at 1e-300 in the kernel; `intersections()` on
  1e-300 geometry classifies at ADR-0003's length floor, as the solver
  and checker do, and promises finite output, not the closed form.
- ⚠ OPEN: C4's order. This plan puts intersections and the double root
  first (the solver change may move digests, and a touch is classified on
  it) and branched profiles second. Agent's pick; the roadmap's theme and
  in/out split are the human's, and `/plan C4` is read as confirming them.
