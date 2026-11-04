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
  driver split of the remainder. If step 7 still misses, its profile
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

- **The measured sources, profile first.** ARCHITECTURE.md §Frame budget
  names four suspects and nobody has profiled them yet:
  (1) `System::acceptance` → `tolerance::scale_at` →
  `Frame::measure` walks the whole component on every part evaluation;
  (2) `diagnose::analysis::redundant` clones and places the sketch, and
  for each head candidate clones it again, removes the constraint and runs
  `dof_only`, which calls `System::new` and ranks every component; it also
  reruns `head_constraints` after every redundant constraint it names;
  (3) `head_constraints` tests `component.sources.contains` for every
  equation of every component;
  (4) `solve::pursue` looks each goal up with `component.params...position`,
  and `system.rs` tests `params.contains`. Step 1's profile reorders
  steps 2–5 or drops some of them, and records the change here.
- **Extent split** (removes 1). `Frame` gets a measure taken over
  everything outside a part, made once per part solve, and joined with a
  measure of the part's own entities at each evaluation. Every term is a
  max, a min or a bounding-box extent, so joining them gives the same
  result as measuring everything at once. The origin is still the first
  point by id. The result must be bit for bit what `Frame::measure`
  returns; a property test extends `frame_measures_what_extent_does` to
  cover the joined measure.
- **Redundancy without a recompile** (removes 2). A candidate is tested on
  the compiled `System` with its rows masked out, and only its own
  component is ranked (the DoF is a sum over components that do not
  touch each other). The recompile is still used whenever skipping it could change
  the answer: the removal disconnects the component, changes its scale
  (`Frame::stated`, the largest stated dimension), or removes or
  touches a contact (ADR-0002 regularization). `head_constraints` is
  rerun only for the component that lost a constraint. This becomes an
  amendment to ADR-0009, written in step 2's commit.
- **Indexed lookups** (removes 3, 4). `System::rows_of` or a source → row
  map takes the place of `sources.contains`; a param → column index the
  component builds once takes the place of `position` and `contains`.
- **Bisected frames.** A bisected frame's extra tries are solves without
  diagnostics. Once the plain frame meets the budget, step 6 measures
  whether bisection still misses it; reusing anything across tries is
  allowed only if that profile shows the need.

## Steps
Grades are tiers (`.agents/tiers.md`): **[1]** routine, **[2]**
careful, **[3]** unproven.

- [ ] Step 1 **[2]** — Profile the worker under `perf record --call-graph
  dwarf` (release with `CARGO_PROFILE_RELEASE_DEBUG=true`, no Cargo.toml
  change) on rectangles-498 edit, slots-498 edit, slots-498 drag and
  bisect, and triangles-501 edit, LM and DogLeg. Add a `--samples N`
  worker flag if five samples are too few. Record each cell's inclusive
  share per suspect, plus anything unnamed above 10 %, under *Profile*
  below, and reorder steps 2–5 to match. Commit: `docs(plan)` plus the
  bench flag.
- [ ] Step 2 **[3]** — Redundancy without a recompile, as in the deltas,
  with its fallbacks, and the ADR-0009 amendment. Tests: redundant sets
  are equal to the recompiling pass's (kept under `cfg(test)` as the
  reference) on the C1 fixtures and every generated family and size,
  plus a proptest over the corpus strategies. That includes a removal
  that splits a component, one that lowers the stated scale, and one on
  a tangent contact.
- [ ] Step 3 **[2]** — Extent split, as in the deltas. Test: the joined
  measure equals `Frame::measure` bit for bit (proptest over the corpus
  strategies, plus f64's extremes near 1e±300 per `solver.md`).
- [ ] Step 4 **[1]** — `head_constraints` reads rows by source map. Test:
  the head sets are unchanged on every generated family.
- [ ] Step 5 **[1]** — Index the param lookups in `pursue` and `system.rs`.
  Test: the existing solve and drag suites pass, and the digests are
  unchanged.
- [ ] Step 6 **[2]** — Re-profile the 500-entity cells. If any of them
  still has a removable source above 10 % that steps 2–5 did not reach,
  remove it here with its own equality test, or split it into its own
  step if it is larger. That includes reuse across bisection tries when
  the bisect cell alone misses.
- [ ] Step 7 **[1]** — Re-run `bench_c2` in full and replace the post-M6
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
  replaced. §Diagnose, *Redundancy*: when the compile is reused and when
  it is redone. §Drag, *Per-step diagnostics*: the 20 ms sentence.
- `docs/ROADMAP.md` §C2: the status line, and the M8 line's pointer to
  the deleted idea now points to this plan's outcome. If the budget is
  met, C2 is ready for `/close-cycle`.
- `docs/adr/0009-stages-and-part-rank.md`: the amendment (step 2).
- `AGENTS.md` current state.
- `CHANGELOG.md` under Unreleased: 500-entity edits and drags inside a
  4 ms frame (a consumer sees the speed; no API change).

## Profile
Filled in by step 1, then by step 6.

## Open questions
- ⚠ OPEN: `kernel.perf_event_paranoid` is 4 on the reference machine,
  which refuses `perf record` to a user. The human decides by step 1:
  lower it for the session (`sudo sysctl kernel.perf_event_paranoid=1`),
  or let step 1 use temporary `std::time` spans in an uncommitted
  worktree. Spans attribute only the suspects they wrap, so the
  unnamed > 10 % column would be blind.
- ⚠ OPEN (agent, step 2): if the fallbacks fire often enough on the corpus
  that the recompile still dominates, can the scale fallback be narrowed
  to "the rank tolerance actually moves"? Decided by step 2's measurement
  and recorded in the amendment.
- Settled by the agent: the acceptance is the worst sample, not the median,
  because the budget applies to every frame, not to the average.
