# Plan: m8-frame-budget

- Started: 2026-11-04
- Milestone: M8 (cycle C2, docs/ROADMAP.md)
- Idea: docs/ideas/c2-decomposition.md (absorbed: its M8 decision,
  profile-then-remove; the file is deleted)

## Goal
The 500-entity edit, first drag step and bisected drag frame each take
≤ 4 ms in every sample on the reference machine, under both algorithms,
measured by `bench_c2`. A profile names where the 11–42 ms go before
anything is cut, and only what it confirms is removed: one path, S1
kept, no public type changes, and the same bits everywhere (C1 digests,
the corpus's locality and staged-against-whole equalities unchanged).

## Non-goals
- Rigid bodies in the remainder (option A), a diagnosis cache (S2), a
  driver split of the remainder. If step 6 still misses, its profile
  picks one of them in a second M8 plan. The target stays 4 ms.
- A change to what redundancy means: the same rank test on the same
  rows at the same tolerances. Only how often it is compiled changes.
  One left-null basis of the head for every candidate would change the
  test; it is not done here.
- The conflict analysis's cost (`docs/BACKLOG.md`). The static solve's
  cells are reported but not budgeted.
- A timing assertion in the gate: wall-clock numbers depend on the
  machine. The bench is the acceptance.

## Design deltas
No public type or signature changes. No new glossary term.

- **The measured sources** (step 1's *Profile*, below). Of the four
  suspects ARCHITECTURE.md §Frame budget named, only (1) `scale_at` is
  confirmed (12-24 %). (2) the redundancy recompile never runs on the
  500-entity cells (`dof_only` 0 %: the generated families have no head
  candidate), so its mask is dropped from M8; what costs there is
  `head_constraints`' own `staged_checked` (12-21 %). (3) the
  `sources.contains` test and (4) `pursue`'s `position` are under 1 % and
  are dropped. Two sources nobody named are over 10 %: the staged rank
  (`staged_checked`) runs three times per frame on the same component
  (solve's `stageable`, `freedom_on`, `head_constraints`), 30-55 % in all,
  and `Tolerances::at` runs six or seven times per frame (15-28 %).
- **One staged rank per component and frame** (removes the repeats).
  `staged_checked` at a frame's final coordinates is computed once and the
  solve's `stageable`, the analysis's `freedom_on` and `head_constraints`
  read it. Same matrices in the same order, so the same bits; a cache
  keyed by component and values, not a new path. Kept on the solve's
  `Outcome` (`diagnose::StagedRanks`), matched by params, sources, scale
  bits, parts and the bits of every coordinate the component reads.
- **One `Tolerances::at` per frame.** The tolerances of a sketch at one
  set of coordinates are made once and shared by `System::new` (solve,
  analysis, redundancy), `observe`, `flips` and `degeneracy`, where the
  coordinates are the same. A different set of coordinates remakes them.
- **Extent split** (removes 1). `Frame` gets a measure taken over
  everything outside a part, made once per part solve, and joined with a
  measure of the part's own entities at each evaluation. Every term is a
  max, a min or a bounding-box extent, so joining them gives the same
  result as measuring everything at once. The origin is still the first
  point by id. The result must be bit for bit what `Frame::measure`
  returns; a property test extends `frame_measures_what_extent_does` to
  cover the joined measure.
- **Bisected frames.** A bisected frame's extra tries are solves without
  diagnostics. Once the plain frame meets the budget, step 5 measures
  whether bisection still misses it; reusing anything across tries is
  allowed only if that profile shows the need.

## Steps
Grades are tiers (`.agents/tiers.md`): **[1]** routine, **[2]**
careful, **[3]** unproven.

- [x] Step 1 **[2]** — Profile the worker under `perf record --call-graph
  dwarf` (release with `CARGO_PROFILE_RELEASE_DEBUG=true`, no Cargo.toml
  change) on rectangles-498 edit, slots-498 edit, slots-498 drag and
  bisect, and triangles-501 edit, LM and DogLeg. Add a `--samples N`
  worker flag if five samples are too few. Record each cell's inclusive
  share per suspect, plus anything unnamed above 10 %, under *Profile*
  below, and reorder steps 2–5 to match. Commit: `docs(plan)` plus the
  bench flag.
- [x] Step 2 **[3]** — One staged rank per component and frame, as in the
  deltas. Test: the solve, DoF, per-entity DoF, direction span and
  redundant sets are bit-equal to the three-pass code (kept under
  `cfg(test)` as the reference) on the C1 fixtures and every generated
  family and size, plus a proptest over the corpus strategies.
- [x] Step 3 **[2]** — One `Tolerances::at` per frame, as in the deltas.
  Test: the existing solve, diagnose and drag suites pass and the digests
  are unchanged.
- [x] Step 4 **[2]** — Extent split, as in the deltas. Test: the joined
  measure equals `Frame::measure` bit for bit (proptest over the corpus
  strategies, plus f64's extremes near 1e±300 per `solver.md`).
- [x] Step 5 **[2]** — Re-profile the 500-entity cells. If any of them
  still has a removable source above 10 %, remove it here with its own
  equality test, or split it into its own step if it is larger. The
  candidates the first profile left: the dense Jacobian's allocation in
  `evaluate_within` (`malloc`/`calloc` ~15-20 %), the `BTreeMap`
  coordinate lookups (26-35 % of samples), `unit_row` (8 %), and reuse
  across bisection tries when the bisect cell alone misses.
- [ ] Step 6 **[1]** — Re-run `bench_c2` in full and replace the post-M6
  table in ARCHITECTURE.md §Frame budget with the new one, the profile's
  attribution included. If a cell misses, record the miss and the profile
  that chooses between A, S2 and the driver split. Commit: `perf(debug)`
  or `docs`.

## Acceptance
- `cargo run --release -p laman-debug --example bench_c2` on the
  reference machine (AMD Ryzen 9 7950X): rectangles 498, slots 498 and
  triangles 501, LM and DogLeg. The edit, the first drag step and the
  slot grid's bisect cell must each have a **worst** sample ≤ 4 ms out
  of five.
- The C1 bench keeps its 1 ms budget, and the 98/99 and 249 cells do not
  get slower.
- Bits: the C1 corpus's 50 digests are unchanged natively and on
  wasm32-wasip1. The M6 locality test (edit keeps every bit outside its
  downstream closure) and the staged-against-whole properties (DoF,
  per-entity DoF, direction span, redundant sets, conflict sets) are
  green at `PROPTEST_CASES=1024`. No fixture expectation changes.
- `tools/gate.sh` and `tools/py-test.sh` are green.

## Docs to update on completion
- `docs/ARCHITECTURE.md` §Frame budget: the new table and the confirmed
  attribution, with the paragraph that calls the two sources unprofiled
  replaced. §Diagnose: the staged rank computed once per component and frame. §Drag, *Per-step diagnostics*: the 20 ms sentence.
- `docs/ROADMAP.md` §C2: the status line, and the M8 line's pointer to
  the deleted idea now points to this plan's outcome. If the budget is
  met, C2 is ready for `/close-cycle`.
- - `AGENTS.md` current state.
- `CHANGELOG.md` under Unreleased: 500-entity edits and drags inside a
  4 ms frame (a consumer sees the speed; no API change).

## Profile
Step 1, 2026-11-04, AMD Ryzen 9 7950X. Method: `perf record -F 4000
--call-graph fp` of `bench_c2 --worker F 2 ALGO OP --samples 20` built with
`RUSTFLAGS="-C force-frame-pointers=yes"` and `CARGO_PROFILE_RELEASE_DEBUG=true`
(a separate target dir; `--call-graph dwarf` truncates every stack after
its first function). The release build inlines the whole call tree into
`main`, so a share is the fraction of samples whose stack holds a frame
of that function, inlined frames included. The denominator is the timed
operation's samples (`black_box` of the solve for an edit, `DragSession::to`
for a drag and a bisect); the edit's set-up is outside it (the worker now
builds the edited sketch once and clones it per sample). `--samples N` is
the new worker flag. Percent of the timed operation, LM / DogLeg:

| cell | diagnose (`conclude`) | solve (`attempt`) | `staged_checked` | `scale_at` | `Tolerances::at` | `redundant` | `dof_only` |
|---|---|---|---|---|---|---|---|
| rectangles-498 edit | 42 / 43 | 52 / 52 | 31 / 30 | 23 / 22 | 28 / 27 | 18 / 18 | 0 / 0 |
| slots-498 edit | 55 / 55 | 40 / 41 | 52 / 52 | 12 / 12 | 20 / 19 | 22 / 23 | 0 / 0 |
| slots-498 drag | 57 / 56 | 43 / 43 | 55 / 55 | 13 / 14 | 17 / 16 | 24 / 24 | 0 / 0 |
| slots-498 bisect | 28 / 28 | 72 / 72 | 43 / 44 | 24 / 24 | 16 / 15 | 12 / 12 | 0 / 0 |
| triangles-501 edit | 43 / 45 | 53 / 51 | 37 / 39 | 21 / 19 | 17 / 18 | 18 / 20 | 0 / 0 |

LM and DogLeg agree within 2 points in every cell. Inside the slot edit,
`staged_checked` is 18 % under `head_constraints`, 18 % under `freedom_on`
and 15 % under `stageable` (the solve's); `redundant` is `head_constraints`
(84 % of it) plus one `System::new`; `scale_at` is all under `acceptance`
in `evaluate_within`; `Tolerances::at` is called from `System::new`
(solve, analysis, redundancy), `observe`, `flips` and `degeneracy`, 3-5 %
each. Other shares over 10 %: `malloc`/`calloc` 15-20 % (10 % of the slot
edit under `evaluate_within` in the solver loop, the rest under the three
`staged_checked`s), `BTreeMap` lookups 26-35 %, `unit_row` 8 %.

Suspects: (1) confirmed. (2) the recompile is never reached on these cells
(no head candidate); `head_constraints`' own rank is. (3) `sources.contains`
under 1 %. (4) `position` 0.2-1.0 %. Steps 2-5 are reordered to match: the
old 2 (mask) and the old 4, 5 are dropped; the shared staged rank and the
shared tolerances are new; the extent split stays. Step 5 re-profiles.

Step 5, after steps 2-4 (self time, `perf report --no-children`, the
inclusive shares of step 1 are not comparable: inlining had put the
once-per-part-solve measure of step 4 under every caller). The re-profile
found one removable source: `Frame::split` still cost 12 % of the slot edit,
because it measured everything outside a part at every part, O(parts x
entities) (a slot grid has a part per stage). `Splitter` (a design delta of
this step, crate-private) sorts the frame's entities once by the parts that
move them and keeps each group's terms until its part is solved, so the
parts of one component measure each entity about once; an entity several
parts move (an arc across two) is kept the same way. Bit-equal to the whole
measure (`split_frame_measures_what_the_whole_does`, now also over a
sequence of parts moving in turn, at f64's extremes). A/B, alternating the
two builds on a machine at load ~17 (15 samples, LM, median ms, before ->
after): rectangles edit 11.5 -> 11.2, slots edit 17.5 -> 16.5, slots drag
21 -> 18, slots bisect 38-52 -> 33-43 (the pair measured together), triangles
edit 9.7 -> 9.6. What is left is flat, no self-time symbol over 9 %:
`Splitter::split` 8.5 % on the rectangle edit (the joins, O(parts^2)),
`unit_row` 7-8 %, `evaluate_within` 6 %, `entity_params` 3-5 %, `malloc`/`free`
~8 %, `BTreeMap` lookups ~6 %, `stage::cut` 6 % on rectangles. No cell is
near 4 ms (edits 10-17 ms, drag 18 ms, bisect 33+ ms), so step 6 will record
a miss; the remaining cost is spread over many sources of a few percent
each, none of them a single removable one.

## Open questions
- Settled by the agent: `perf_event_paranoid` is 1; step 1 used `perf`.
- Settled by the agent (step 2): `head_constraints` compiled its own
  `System::new(&working)`, whose component scales are measured at the
  solve's coordinates, not the sketch's; its rows (ratio rows are scaled)
  and acceptance differ from the analysis's, so no cache keyed soundly by
  scale could share it. Its first call now reads the analysis's own system,
  the one `held` and `dof` were judged on (the recompiles after a naming
  are unchanged). Held bit-equal to the three-pass reference on C1, every
  family and size and the corpus proptests. A drag frame still factorizes
  twice: the solve accepts before `pursue` moves the last part. A/B on a
  loaded machine (load 17, 15 samples, LM): slot edit 35 → 22 ms, drag 34
  → 22, bisect 64 → 54; triangle edit 17.5 → 12.8; rectangle edit 17.8 →
  15.3, drag 35 → 30.
- Settled by the agent: the acceptance is the worst sample, not the median,
  because the budget applies to every frame, not to the average.
