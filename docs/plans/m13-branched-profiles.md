# Plan: m13-branched-profiles

- Started: 2026-11-13
- Milestone: M13 (cycle C4, docs/ROADMAP.md), the second of three
- Idea: none; the roadmap's C4 section as confirmed by `/plan C4`, and
  verbatim from the human: "/plan m13"

## Goal

`laman::profile::profiles` splits a branched core into its profiles. At
each join of three or more edge ends the edges are ordered by tangent
direction, then by curvature where two leave tangent. Every bounded area of
the core is a loop, and a core edge with one profile on both sides (a
bridge) is reported by id and enters no loop; every C4 profile fixture
comes out at its closed form. Joins stay declared (ADR-0012 D1): the
coordinates order edges at a join and never make one.

## Non-goals

- **M14** (next plan): `laman-check`'s independent measure of branched
  profiles and of intersections, with broken twins; the digest over
  intersections, native and wasm32-wasip1; the 500-entity cost of both under
  ADR-0014's stop rule; `laman-py` over `intersections`. This plan changes
  the checker and the binding only as far as step 4's type change requires.
- A profile that uses an undeclared crossing (C5's split declares it). A
  core whose curves cross gives a deterministic result whose nesting is not
  specified, as `profiles()` already says; no loop is dropped.
- Ellipses, splines, booleans, filling.

## Design deltas

**D1 — ADR-0017, "A branched core splits at its joins" (amends ADR-0012
D2).** Settled by step 1; the ADR is the reference where it is more
precise than this summary (the tie tests, the bridge search, pieces):
- *Bridges*: core edges whose removal disconnects the core, found in id
  order. They are reported in `Profiles::bridges` and removed before the
  loops are walked. With them gone, a rectangle tied to a rectangle inside
  it by a line is a plate with a hole.
- *Order at a join*: each edge end leaving the join gets the tangent
  direction it leaves in (a line toward its other end; an arc perpendicular
  to its radius, turning with its traversal) and its signed curvature
  (0 for a line, ±1/r for an arc). Ends are sorted counter-clockwise by
  direction, then by curvature. Two directions are equal when the ends
  they lead to stay within `L` of one another (the length tolerance), so
  the comparison needs no new constant. Exact ties (an overlap) fall back
  to edge id, which is deterministic, and their nesting is not specified.
- *Loops*: from each directed edge, the walk takes the next edge clockwise
  after the reverse direction. Each directed edge lies in exactly one loop.
  In each connected piece, the loop with the most negative area (ties
  broken by smallest edge id) is its **outline**. That loop is used for
  containment and never becomes a profile. Every other loop of the piece
  is one of its bounded areas.
- *Nesting*: a piece's bounded areas all take one role, an outer boundary
  or a hole, by the parity of the innermost loop of another piece that
  contains the outline. So a branched core inside a plate gives the plate
  one hole per bounded area. A plain loop is a piece whose one bounded
  area is its outline reversed, so ADR-0012 D3 holds for it unchanged.

**D2 — Public types (design delta, CHANGELOG `### Breaking`).** In
`laman::model::profile`, `Branched` and `Profiles::branched` are removed,
and `Profiles::bridges: Vec<EntityId>` is added. A line or arc may now
appear in two loops, once in each direction. The fix for callers is
"read `profiles`; a core no longer stays unsplit". `laman-py` and its stub
follow in the same commit. `laman_check::check_profiles`'s rule that each
edge is listed once becomes: each edge is listed at most once in each
direction, a bridge in no loop, and every other edge once in some list.
Its `Repeated` violation keeps its name.

**D3 — Fixtures.** `tests/fixtures/c4/<slug>/` with a `profiles` section,
in the form `laman_debug::fixture` already reads. The new `bridges` list
replaces `branched`. `c3/shared-edge` is re-blessed: two profiles, each
40 × 30. Its digests move.

**D4 — Generator.** `laman_debug::corpus::cell_grid(rows, cols)`: a grid of
rectangular cells whose shared walls are single lines, with a C1 slot as a
hole in each cell. It gives `rows · cols` profiles with net area
`cell − (40 × 10 + 25π)`, and it is the M14 cost fixture at 500 entities.

## Steps

Grades: **[1]** routine, the design says exactly what to write; **[2]**
careful, a numeric or geometric case within the design; **[3]** unproven,
an algorithm whose robustness or bound is established here.

- [x] Step 1 **[3]** — ADR-0017 (D1). It reads the reference trees for
  how they order edges at a vertex, if they do, and names the files read.
  It settles the open questions below and records ADR-0012 as amended.
  GLOSSARY: *Bridge* and *Outline* are added, *Branched* and *Core*
  rewritten.
- [x] Step 2 **[3]** — The join order: direction and signed curvature of
  an edge end in the `crate::frame` frame, and the comparator at a join
  with D1's tangent test. Closed-form tests: line against line, line
  against arc tangent on either side, two arcs tangent with different
  radii and with equal radii (the id tie), at scales 1e-6 to 1e6, with
  translations, and near 1e±300.
- [x] Step 3 **[2]** — Topology: bridges, then the walk over each piece's
  directed edges with step 2's order, and the outline per piece. Unit tests
  on the shared wall, the window, the figure eight and two rectangles
  joined by a bridge (loop counts, edge order, the bridge's id).
- [x] Step 4 **[2]** — D1's nesting in `profiles()`, D2's type change
  through `laman`, `laman-check`, `laman-debug`'s fixture reader and
  digest, `laman-py`, the stub and its tests. Rustdoc and its examples,
  with a branched one. CHANGELOG `### Breaking`. `c3/shared-edge` and its
  digests are re-blessed in this commit, and the message says why.
  `tools/py-test.sh` passes.
- [x] Step 5 **[2]** — The C4 profile corpus (D3), solved, checked, then
  extracted, every number from a closed form. Fixtures: shared wall, two
  60 × 60; window, a degree-4 join and four panes; tangent branch, an arc
  and a line leaving one join tangent, ordered by curvature, with two
  profiles of closed-form area; figure eight, two lobes at one declared
  join; bridged plate, one profile with a hole and the bridge reported;
  branched core in a plate, the plate holed once per bounded area.
- [ ] Step 6 **[2]** — D4's `cell_grid`, with a runnable example. Its 2 × 2
  and the C2 `rect_chain` give their known profile counts and areas.
- [ ] Step 7 **[3]** — A property test: planar subdivisions built by
  construction (a polygon split by chords and arcs, some joins degree 4,
  some tangent), then rotated, scaled and translated. Checked: the profile
  count; the areas summing to the outline's; each shared edge in two loops,
  once in each direction; the bridges. It includes one 1e±300 case,
  shrinks into `regression/`, and adds seeds of branched cores to the
  `profiles_sketch` fuzz corpus.

## Acceptance

`cargo test` over `tests/fixtures/c3/*` and `c4/*` and step 7's
properties. Shared wall: two profiles, 3600 each. Window: four profiles,
each a quarter of the frame's area. Figure eight: two profiles, each at
its lobe's closed form. Tangent branch: the order at the join by
curvature, and both areas at the closed form. Bridged plate: one profile,
one hole, the bridge's id in `bridges`. Cell grid 2 × 2: four profiles,
each net area `cell − (400 + 25π)`. `rect_chain(98)`: 98 profiles. Every
fixture passes `check_profiles` (D2's rule). C1 digests unchanged; C3
digests unchanged except `shared-edge`'s, re-blessed in step 4.

## Docs to update on completion

- `docs/ARCHITECTURE.md` §Profiles: the topology bullet (bridges, the
  order at a join, outlines, nesting of pieces); §The checker (the
  listed-once rule).
- `docs/DATA-MODEL.md` §Profiles: `bridges`, `branched` gone, an edge in two
  loops.
- `docs/adr/0012-…`: status line "amended by ADR-0017".
- `GLOSSARY.md`: *Bridge* and *Outline* added; *Branched* and *Core*
  sharpened (step 1).
- `docs/ROADMAP.md` §C4: M13 recorded as landed.
- `AGENTS.md` current state: M13 landed, next M14.
- `docs/BACKLOG.md`: the `check_profiles` re-derivation line now names
  bridges instead of branched.

## Open questions

All four settled by step 1 in ADR-0017.

- *Settled* (D7): `Branched` is removed. A tie still gives loops in id
  order, and crossing curves are already unspecified.
- *Settled* (D6): one hole per bounded area, so the roles stay uniform per
  piece and an island inside one of them nests the same way.
- *Settled* (D3): directions tie when `s · |sin Δθ| ≤ L`, `s` the shorter
  edge's whole length (an arc's `r` times its sweep); curvatures tie when
  `s² |Δκ| / 2 ≤ L`. Step 2's closed-form tests are its measurement; a case
  they order wrongly is an addendum to ADR-0017, not a new constant.
- *Settled* (D5): with crossing curves the most negative loop (ties by
  smallest edge id) is still the one outline, deterministically, every
  other loop gets the piece's role and none is dropped; its nesting is
  unspecified. Step 7 tries it against crossing constructions.
