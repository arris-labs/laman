# Plan: m14-check-digest-python

- Started: 2026-11-16
- Milestone: M14 (cycle C4, docs/ROADMAP.md), the last of C4
- Idea: none; the human: "M14", as scoped in the non-goals of plans
  m12-intersections and m13-branched-profiles.

## Goal

`laman_check::check_intersections` re-measures what `intersections()`
reported, with its own code, and names each wrong meeting and each missed
pair. `check_profiles` re-derives the split of a branched core (bridges,
open and dangling edges, each loop one bounded area) instead of only
counting how often an edge is listed. Every C4 fixture is checked clean and
every broken twin is named. The digest covers intersections and the C4
profiles, equal natively and on wasm32-wasip1. Extraction and intersections
at 500 entities are measured against ADR-0014's stop rule. `laman-py`
exposes `intersections`, its types and `check_intersections`, with stubs,
and pytest reproduces every C4 number. C4's *Accept* is then met, and
`/close-cycle` can run.

## Non-goals

- Any change to the sketch: split, trim, extend (C5).
- regression/`below-floor-uncovered-edges` stays ignored: below
  `LENGTH_FLOOR` nothing is resolved (ADR-0003); see *Open questions*.
- A spatial index beyond a sorted sweep that changes no result; nesting of
  crossing loops stays unspecified (ADR-0017 D5).
- The other `check_profiles` backlog lines (fixed parameters, `ANGLE`).

## Design deltas

- **D1. `check_intersections`** (new, `laman-check`, ADR-0018):
  `check_intersections(&Sketch, &Coordinates, &Intersections) ->
  IntersectionReport { violations, missing }`. Per reported meeting, by
  the checker's own distances (point to line, point to arc): the point
  within `L` of both curves; each parameter mapped back to the point within
  `L`; the kind from the checker's own reading (ADR-0018 decides: tangent
  directions at the point against `L` over the curves' lengths, as
  ADR-0017 D3 reads ties); an arc's parameter inside its sweep. Per pair:
  ids ordered, the curves non-construction and distinct, the order of
  ADR-0016 D7. Completeness: every place where a pair decidedly meets, by
  the checker's own minimisation (roots bracketed between the distance's
  extremes and bisected, common normals, ends crossing the other curve,
  parallel stretches), has a reported meeting of the pair within `4L`;
  places nearer an end than `4L` or at a shared join are undecided. The
  undecided band is `L/4` to `4L` (ADR-0018 D1). In the frame of a
  power of two, so 1e±300 never overflows. The thresholds "within `L`"
  above read as ADR-0018 D2–D4 settle them.
  `IntersectionViolation`: `OffCurve`, `Parameter`, `Kind`, `OutsideSweep`,
  `Order`, `NotACurve`, `Repeated`, `Missed { first, second }`, `AtJoin`.
  Step 2 lands the first seven (a meeting's variants carry `first`,
  `second` and its `meeting` index, no measured numbers); `Missed` and
  `AtJoin` come with step 3.
- **D2. `check_profiles` re-derives the split** (ADR-0018): its own
  pruning gives open and dangling edges, its own connectivity test (an
  edge whose removal disconnects its core, by union-find without it: a
  route other than extraction's low links) gives bridges; each listed
  class must equal the derived one. A loop is one bounded area: no other
  edge of its piece has its probe strictly inside the loop (undecided
  within `L`, and on a piece whose curves cross, ADR-0018 D7). A hole is
  tested against its sibling holes of other pieces too (backlog, m10
  review). New `ProfileViolation` variants `Misclassified { entity }`
  and `NotOneArea { first }`: a public change, under `### Breaking`.
- **D3. Digest**: `laman_debug::digest::of_intersections` and the C4
  profiles; `tests/fixtures/c4/digests.txt` under `c4-` names, read by
  `determinism.rs`, so `tools/digest-wasm.sh` covers them unchanged.
- **D4. Cost**: `bench_c2` gains an `intersections` operation and the
  `cell_grid` family (b311912). `Classes::of` hoisted out of the per-core
  loop (backlog, m13 review). If a 500-entity cell exceeds 1.5 ms, pairs
  are pruned by a sweep over x-sorted bounding boxes widened by `L`, held
  to bit-equality with all pairs. The numbers are measured by step 7 and
  recorded as an ADR-0014 addendum, which also names the stop rule for
  intersections.
- **D5. `laman-py`**: `laman.intersect.intersections`, `Intersections`,
  `Intersection`, `Meeting`, `MeetingKind`, `Along`, `IntersectError`;
  `laman.check.check_intersections`, `IntersectionReport`,
  `IntersectionViolation`; the new `ProfileViolation` variants;
  `laman.debug.digest_intersections`, `cell_grid`. Stubs and docstring
  examples; `tools/py-test.sh`.
- **Glossary**: *Missed pair* and *Undecided band* (ADR-0018), written
  with it in step 1.

## Steps

Grades (`.agents/tiers.md`): **[1]** routine; **[2]** careful; **[3]** unproven.

- [x] Step 1 **[3]** — ADR-0018: the checker's measure of intersections
  (D1: per-meeting tests, how a kind is read, completeness by closest
  distance, the undecided band) and the re-derived split (D2). Reads
  ADR-0013, 0016, 0017; no code.
- [x] Step 2 **[2]** — `check_intersections`, per-meeting and per-pair
  tests (D1 less completeness): types, rustdoc example, closed-form tests
  on the C4 intersection corpus and hand-broken values.
- [x] Step 3 **[3]** — completeness: closest distance of line/arc/circle
  pairs in the frame, the undecided band, `Missed` and `AtJoin`; held at
  1e±300 and at 1e-6/1e6 shifted far from the origin. Fuzz target
  `check_intersections_sketch`.
- [x] Step 4 **[2]** — `check_profiles` re-derives open, dangling and
  bridge edges, tests one bounded area per loop and sibling holes (D2);
  c3 and c4 profile corpora and `c4_profile_props` stay clean.
- [x] Step 5 **[2]** — broken twins for the C4 corpus: a meeting moved,
  dropped, its kind or parameter changed, a pair dropped; two bounded
  areas merged, a bridge put in a loop, a hole inside a sibling. Each
  names its variant. The intersection corpus also runs scaled 1e-6 and
  1e6 and shifted (backlog, m12 review); the m12 property runs the check.
- [x] Step 6 **[1]** — digests (D3): C4 profiles and intersections,
  blessed, native and wasm32-wasip1 equal by `tools/digest-wasm.sh`.
- [ ] Step 7 **[2]** — cost (D4): bench cells, `Classes::of` hoisted,
  ADR-0014 addendum with the worst of three runs at 500 entities.
- [ ] Step 8 **[2]** — the sorted sweep, only if step 7 finds a cell past
  its number: results bit-equal to all pairs on the corpus and the
  property; skipped with a line here otherwise.
- [ ] Step 9 **[2]** — `laman-py` over D5: classes, stubs, docstrings,
  `py-test.sh` green.
- [ ] Step 10 **[2]** — pytest: the C4 corpus (profiles and
  intersections, every number), digests equal to the blessed ones, the
  checker's twins, 1e±300 bit for bit against Rust, a hypothesis property
  of crossing cutters.

## Acceptance

- `cargo test -p laman-debug --test c4`: every C4 fixture (shared wall,
  window, tangent branch, figure eight, bridged plate, branched in plate,
  the eight intersection fixtures) solved, `check`, `check_profiles` and
  `check_intersections` clean; every broken twin names its variant.
- The intersection corpus at 1e-6, 1e6, shifted, and its 1e±300 cases,
  checked clean; `c4_props` and `c4_profile_props` run the checkers over
  5000 cases.
- `tools/digest-wasm.sh`: every c1–c4 digest identical natively and on
  wasm32-wasip1.
- `bench_c2`: extraction and intersections at 500 entities inside 1.5 ms,
  or the number ADR-0014's addendum records by its stop rule.
- `tools/py-test.sh`: pytest reproduces every C4 number, stubtest clean.

## Docs to update on completion

- `docs/ARCHITECTURE.md` §The checker (`check_intersections`, the split
  re-derived, the sentence "is not re-derived" removed), §Intersections
  (cost, the sweep if built), §Determinism (C4 digests), §The Python
  binding (`laman.intersect`).
- `docs/ROADMAP.md` C4: M14 landed; C4 ready for `/close-cycle`.
- `AGENTS.md` current state.
- `CHANGELOG.md` `Unreleased`: `check_intersections`, Python
  `laman.intersect`, the re-derived profile check; `### Breaking`: the new
  `ProfileViolation` variants.
- `docs/BACKLOG.md`: drop the lines absorbed (re-derived classification,
  sibling holes, `Classes::of`, the C4 corpus at other scales).

## Open questions

- *Settled* (step 1, ADR-0018 D1): the undecided band is `L/4` to `4L`
  (`BAND = 4`), twice the `2L` a meeting moved to an end can shift.
  Completeness is per place, not per pair: a closest distance within `L`
  is not a meeting at a shallow end (ADR-0018 Context), and a pair
  reported at all could still drop one of its meetings.
- *Settled* (step 3): "held at 1e±300" is held as the kernel's own tests
  hold it. At 1e300 (shifted 300 sizes) `check_intersections` runs on
  `intersections()`'s result: clean, every dropped meeting or pair
  `Missed`. At 1e-300 a sketch is below `LENGTH_FLOOR`, so through
  `Tolerances::at` every curve is negligible: nothing is reported, nothing
  missed, a listed pair is `NotACurve` (tested so). The measure itself is
  held at 1e-300 by the module's own tests, the tolerance passed at the
  scale (`1e-9·s`), closed forms at 1e-300 to 1e300 shifted 1e4 sizes.
  `AtJoin` carries the meeting's index, like the other per-meeting variants.
- ⚠ OPEN: whether intersections share extraction's 1.5 ms or get a
  number of their own by ADR-0011's stop rule. Agent, step 7; the
  roadmap's "inside ADR-0014's stop rule" is the rule, not the number.
- ⚠ OPEN: C4 closes with regression/`below-floor-uncovered-edges` still
  ignored (geometry below `LENGTH_FLOOR`, not in C4's *Accept*). The
  human, before `/close-cycle`; this plan assumes yes.
