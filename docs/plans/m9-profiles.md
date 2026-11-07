# Plan: m9-profiles

- Started: 2026-11-07
- Milestone: M9 (cycle C3, docs/ROADMAP.md), the first of three
- Idea: docs/ideas/profile-join-rule.md (absorbed: Q1 B, Q2 yes, Q3 A)

## Goal

`laman::profile::profiles(sketch, values)` turns any coordinates, a
solution's included, into profiles: every outer loop counter-clockwise with
its holes clockwise, nested by containment, each with its signed and net
area. It also reports the open chains, dangling edges, branched cores and
unjoined coincidences, each by the caller's ids. Joins are declared
(ADR-0012). Every C3 *Accept* number that needs no checker, digest or binding
comes out within tolerance, at scales 1e-6 to 1e6 and near 1e±300.

## Non-goals

- **M10** (next plan): `laman-check`'s independent measure and broken twins;
  the digest over profiles, native and wasm32-wasip1; the 500-entity budget
  (ROADMAP ⚠ OPEN, ADR-0011's stop rule).
- **M11**: `laman-py` over the API with stubs; pytest reproduces every number.
- Faces of a branched core: two rectangles sharing an edge are reported,
  not split into two regions (backlog line, D2).
- Curves crossing or touching without a shared point (C3 *Out*): the result
  is deterministic and panic-free, but its nesting is not specified there.
- A serde form of the result, and any change to the solve or `Solution`.

## Design deltas

**D1 — ADR-0012, "Profiles join by declaration".** The idea's decision: a
join is one point id or a `coincident` chain (union-find, each class
represented by its smallest point id). It is never measured. A violated
joining `coincident` still closes its loop, and the loop names it (Q3 A).
ADR-0002 is cited as the precedent, with the reason profiles go further:
rank reads coordinates, but a join only reads what the caller wrote. D2–D4 are
also recorded there. ⚠ OPEN closed: ROADMAP §C3, the join rule.

**D2 — Topology.** An **edge** is a non-construction line or arc; its ends
are join classes. A non-construction circle is a loop by itself. Edges are
pruned at degree-1 classes, repeatedly, in id order. A component pruned away
entirely is an **open chain**. A pruned edge whose component keeps a core is a
**dangling edge**. A core in which every class has degree 2 is a **loop**,
ordered from its smallest edge id. A core with a class of degree > 2 is
**branched** (the roadmap's self-touching loop, and edges shared by two loops):
its edges and those classes are reported, and none of its edges enters a
profile. An edge with both ends in one class is **degenerate**, reported and
left out.

**D3 — Geometry (`laman::profile`, private kernel).** Area: Green's
`½∮(x dy − y dx)` in loop order, on coordinates translated to the loop's
first join and scaled by an exact power of two. A line gives its cross
product; an arc gives chord plus segment `r²(θ − sin θ)/2`, θ the
DATA-MODEL sweep, negated when reversed. Below a sweep of 1 rad, `θ − sin θ`
is its eight-term Taylor series (*Settled* in step 2: measured against a
double-double reference, the series is within 1.6 ulps below 1, the direct
form within 1.2 from 1 up and 33 ulps at 0.25). Orientation is the scaled sum's sign. The area
saturates at ±`f64::MAX`, never non-finite. Containment: the winding number
at the midpoint of the inner loop's first edge, from each edge's subtended
angle (`libm::atan2`), plus 2π for an arc when the point lies between its
chord and the arc. Even depth is outer (counter-clockwise), odd is a hole
(clockwise). A hole's parent is its innermost container, and an island is
its own profile.

**D4 — Unjoined coincidences (Q2).** Among open ends only (degree-1
classes), a pair of classes whose points lie within the smaller of the two
edges' length tolerances (`Tolerances::at`, ADR-0004; no new constant) is
reported with both point ids. It never joins them. Pairs are found by sorting
on x, so the cost stays below quadratic.

**D5 — Public types (design delta, CHANGELOG).** In `laman::model::profile`,
so that M10's checker can name them under the layer rule: `Profiles {
profiles, open_chains, dangling, branched, degenerate, unjoined }`, `Profile {
outer: Loop, holes: Vec<Loop>, net_area }`, `Loop { edges: Vec<Traversal>,
signed_area, joins: Vec<ConstraintId> }` (the joining `coincident`s), `Traversal {
entity, reversed }`. The lists' elements (settled in step 3): `open_chains:
Vec<Vec<EntityId>>` (edge ids), `dangling` and `degenerate: Vec<EntityId>`,
`branched: Vec<Branched { edges, joins }>` (`joins` the hub points, by smallest
point id), `unjoined: Vec<Unjoined { first, second }>` (point ids). All are
`Debug, Clone, PartialEq`, and every list is ordered by smallest id. The extraction is `laman::profile::profiles(&Sketch,
&Coordinates) -> Result<Profiles, ProfileError>`, with `ProfileError::Missing {
entity }` and `NonFinite { entity }` naming the caller's id. It takes no
`Solution`: a solution's `coordinates` is the input. No existing signature
changes.

**D6 — Fixtures.** The new home is `tests/fixtures/c3/<slug>/`. `expected.json`
gains a `profiles` section, read by `laman_debug::fixture`: per profile, the
outer loop's and each hole's edge ids in order with `reversed`, signed area and
net area, each with its tolerance; and the reported lists by id. Areas come in
closed form: a slot is `2·length·r + πr²`.

## Steps

Grades: **[1]** routine, the design says exactly what to write; **[2]**
careful, a numeric or geometric case within the design; **[3]** unproven, an
algorithm whose robustness or bound is established here.

- [x] Step 1 **[2]** — ADR-0012 (D1–D4). Close ROADMAP §C3's join ⚠ OPEN.
  GLOSSARY: *Join* points at the ADR; add *Profile*, *Loop*, *Edge*,
  *Open chain*, *Dangling edge*, *Branched*.
- [x] Step 2 **[3]** — The D3 kernel on directed edge lists: area, orientation
  and winding number. Tests: closed forms for a rectangle, a slot, a circle, a
  D shape, and an arc at sweeps near 0, π and 2π. Scales 1e-6 to 1e6,
  translations up to 1e4 times the scale, magnitudes near 1e±300, and
  points near an arc's chord and arc. Measures the small-θ threshold.
- [x] Step 3 **[2]** — D5 types and D2 topology. Join classes, pruning,
  loops, branched and degenerate cores, still without geometry. Unit
  tests: a rectangle, the same joined by `coincident`, an open chain, a
  dangling edge, two rectangles sharing an edge, a figure-8, a self-joined
  line, construction edges left out.
- [x] Step 4 **[2]** — `profiles()`: orientation applied to traversal, area,
  nesting, holes and islands, `ProfileError`, rustdoc with runnable
  examples. CHANGELOG `Unreleased` bullet.
- [x] Step 5 **[2]** — D4 unjoined coincidences. Fixture: a rectangle with
  one corner made of two distinct ground points at the same coordinates.
- [x] Step 6 **[2]** — D6 fixture form and the C3 corpus, each solved, then
  checked by `laman-check`, then extracted: rectangle; slotted plate and its
  `coincident` twin; plate with an island in a hole; two disjoint loops; a
  circle alone; an open chain; a dangling edge; shared-edge rectangles
  (branched).
- [ ] Step 7 **[1]** — Slot grids (`corpus::slot_grid`, 1×1 to 7×7 and the
  498-entity one). Exactly one profile per grid, with `rows·cols` holes and
  net area `W·H − n(2·length·r + πr²)`.
- [ ] Step 8 **[2]** — A property test. Random disjoint, nested rectangles,
  circles and slots, built with random shared or `coincident` joins, then
  rotated, scaled and translated. It asserts loop counts, depth parity,
  orientation and areas against the construction, and shrinks into
  `tests/fixtures/regression/` on failure. It also runs one 1e±300 case.

## Acceptance

`cargo test` on `tests/fixtures/c3/*`. Rectangle 7200, counter-clockwise.
Slotted plate and its `coincident` twin: one clockwise hole, net 6721.46
(7200 − 400 − 25π); the twin names its 4 joins. Island: plate 7200 − 400π,
island 100π. Two disjoint loops give 2 profiles; a circle alone gives πr².
Open chain and dangling edge are named by id; shared-edge rectangles are
branched. Slot grids: `rows·cols` holes, net areas within 1e-9 of the plate's
size. Step 8 passes, 1e±300 included. C1 digests are unchanged.

## Docs to update on completion

- `docs/ARCHITECTURE.md`: a new §Profiles (D2–D4, the module, and why the
  types sit under `model`). §The sketch and its ids: construction excluded,
  with "a later cycle" removed.
- `docs/DATA-MODEL.md` §Entities: the join sentence covers `coincident`
  chains. A new §Profiles: the result, conventions and area formulas.
- `docs/ROADMAP.md` §C3: the join ⚠ OPEN closed (step 1). M9 recorded as
  landed in the in-list.
- `GLOSSARY.md`: the step 1 entries.
- `AGENTS.md` current state: C3 under way, M9 landed, next M10.
- `docs/BACKLOG.md`: faces of a branched core; a re-pointing edit, so a shared
  point can be detached without removing its entity (the idea's reason for B).

## Open questions

- *Settled* (step 2): the small-θ threshold for the segment area is 1 rad,
  measured and recorded in D3 (`SMALL_SWEEP` in `laman::profile`'s kernel).
- ⚠ OPEN: whether a branched core is reported (D2, this plan) or split into
  faces. The agent picked reporting, because the roadmap names self-touching
  loops as reported, and faces need angular ordering of curved edges at a
  join. The human may overrule this before step 3, which would change C3's
  scope.
