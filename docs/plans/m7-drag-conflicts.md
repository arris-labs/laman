# Plan: m7-drag-conflicts

- Started: 2026-11-04
- Milestone: M7 (cycle C2, docs/ROADMAP.md)
- Idea: docs/ideas/c2-decomposition.md (its M7 part absorbed; M8 remains)

## Goal

No solve or drag returns a negative radius, so every solution applies
back. The whole path's filter names no set that holds together, the
staged DogLeg analysis concludes where the whole one does, and the
property's conflict sets equal the minimal set known by construction. A
frame that carries an arc's sweep past a named fraction of a turn is
bisected, a bisected frame says so in its `Solution`, the drag property
can fail on a flip, and the 500-entity bench times a bisected frame.

## Non-goals

- Meeting the 4 ms frame at 500 entities, bisected frames included: M8
  (its accept covers every 500-entity bench cell, the new one too).
- The conflict analysis's cost and the older inconclusive findings
  (`dogleg-conflict-inconclusive`, `framework-conflict-inconclusive`,
  the `*_dogleg` halves): out of C2 (ROADMAP.md). Lifted here only if
  step 5's fix lifts them unasked; otherwise untouched.
- A minimum-cardinality search in the solver: the exact minimum is a test
  oracle, ADR-0005's inclusion-minimal guarantee stands.
- New constraint kinds; the Python and release backlog.

## Design deltas

- **Radius bound (ADR-0010, step 1).** A circle's radius parameter is held
  `≥ 0` inside both engines (a projected step with an active bound, or a
  step cut at it), never by clamping the answer (that moves residuals by
  the clamp: -7.6e90). The certificate (ADR-0005) reads the projected
  gradient where a radius is active; a radius at 0 is degenerate, as now.
  The fixtures' statuses are the ADR's; `apply` accepting them is held.
  `satisfied-status-degenerate-tangent` is lifted if the bound fixes it.
- **The whole path's false certificate (ADR-0005 amendment, step 4).** A
  test certified at a stationary point of a satisfiable subset must not
  settle: a further deterministic start (the staged re-solve of the
  subset), or every filter test re-solved by the default solve on both
  paths. Either way ADR-0009's witness exemption goes: staged = whole on
  every conflict the property draws.
- **Staged DogLeg inconclusive (step 5).** Root cause first (the staged
  re-solves' DogLeg radius, or a budget share running out per part,
  ADR-0009 §Budgets); the fix stays inside ADR-0009's one path. The
  property's inconclusive-side exemption is removed for any case where
  the other side concludes.
- **Exact minimum, a test oracle (step 6).** The conflict property builds
  a rigid distance framework by construction plus one stretched distance;
  generically the only minimal conflict set is the **fundamental
  circuit** of that edge (the smallest rigid over-braced subgraph holding
  it; Laman 1970, the pebble game of Jacobs & Hendrickson 1997), computed
  in `laman-debug` test support from the graph alone, never from laman's
  solver. Glossary: **fundamental circuit**.
- **Sweep jump (ADR-0006 amendment, step 7).** A drag try whose tracked
  sweep of any arc moves more than `MAX_SWEEP_STEP` (a fraction of a turn,
  `model::tolerance`, value and reason by step 7) is bisected as a flip
  is, without being named as one: it is not a branch change. Closes the
  ADR-0006 Consequences gap (sweep 0.3 → −3.2 read as +3.08) for frames
  only; the static-solve backlog line stays.
- **The bisected mark (public type, Breaking, step 8).** `Diagnostics`
  gains `pub bisection: Option<Bisection>`, `Bisection { reached: [f64; 2],
  halvings: usize }`: `None` for a static solve and an unbisected frame;
  `reached` the target the frame was solved at (the last one's when every
  try flipped). `Diagnostics` is not `#[non_exhaustive]`, so a struct
  literal breaks (`### Breaking`). Binding, `.pyi`, docstring example in
  the same commit; digests re-blessed only where a drag fixture bisects.
  Glossary: **bisected frame**.

## Steps

Grades are tiers (`.agents/tiers.md`): **[1]** routine, the design says
exactly what to write; **[2]** careful, a numeric or geometric case
within a given design; **[3]** unproven, robustness or a bound
established here.

- [x] Step 1 **[3]** — Radius bound in LM and DogLeg, ADR-0010; lift
  `negative-radius-solve` and `drag-negative-radius`; C1 digests
  unchanged or re-blessed with the reason.
- [x] Step 2 **[2]** — Property: every solution any solve and drag
  property draws (extremes included) has every radius `≥ 0` and applies
  back; drop `drag_props.rs`'s `DimensionOutOfRange` exemption.
- [x] Step 3 **[2]** — Shrink seed `4ca64a2b…` to
  `regression/staged-conflict-inconclusive` (inspect skill), ignored,
  desired: staged DogLeg names whole DogLeg's set; finding updated.
- [ ] Step 4 **[3]** — Fix the whole path's false certificate, ADR-0005
  amendment; lift `whole-false-certificate`; staged = whole on conflict
  sets in `solved.rs`, the witness exemption deleted.
- [ ] Step 5 **[3]** — Fix the staged DogLeg inconclusive case; lift its
  fixture; the inconclusive-side exemption removed where one side concludes.
- [ ] Step 6 **[2]** — Fundamental-circuit oracle in test support; the
  conflict property asserts every set named equals it, under both
  algorithms; the C1 twins' sets checked minimum by their analysis.
- [ ] Step 7 **[2]** — Sweep-jump bisection, `MAX_SWEEP_STEP`, ADR-0006
  amendment; fixture `tests/fixtures/drag/sweep-jump` (ADR-0006's 0.3 →
  −3.2 frame: bisected, lands on the short side, no flip).
- [ ] Step 8 **[1]** — `Diagnostics::bisection` and `Bisection`, rustdoc
  with example; binding, `.pyi`, pytest; `bisect-stuck` and `sweep-jump`
  assert it; changelog `### Breaking`.
- [ ] Step 9 **[2]** — The drag property's construction gets an arc on
  the chain (an observed subject): flips empty or the frame bisected and
  marked; the C2 slot-grid drags assert no flip on every frame.
- [ ] Step 10 **[1]** — Bench: a 500-entity slot-grid drag cell whose
  first frame bisects; numbers in ARCHITECTURE.md §Frame budget.

## Acceptance

- `cargo test -p laman-debug --test solve_regression` with
  `negative-radius-solve`, `drag-negative-radius`,
  `whole-false-certificate` and `staged-conflict-inconclusive` not
  ignored, green under both algorithms.
- `solved.rs`'s conflict property at `PROPTEST_CASES=20000`: under both
  algorithms every set named equals the fundamental circuit; staged =
  whole wherever either concludes; no exemption left in the property.
- Every solve and drag property: radius `≥ 0`, `apply` accepts every
  `finish`, at sketch scales and at 1e±300.
- `tests/fixtures/drag/sweep-jump`: bisected (`bisection` set,
  `halvings ≥ 1`), sweep in `(0, 2π)`, flips empty; the C2 slot-grid
  drags: flips empty on every frame.
- The C1 corpus's 50 digests unchanged on native and wasm32-wasip1, or
  re-blessed with the reason in the commit; pytest equal.
- Bench: the bisected 500-entity cell timed and recorded (budget: M8).

## Docs to update on completion

- `docs/ARCHITECTURE.md` §Solve, §LM, §DogLeg (the radius bound),
  §Diagnose (the filter's starts, the exact-minimum check), §Branches and
  §Drag sessions (sweep jump, `bisection`, the negative-radius gap gone),
  §Frame budget (the bisected cell).
- `docs/DATA-MODEL.md` §Entities (radius `≥ 0`), §Diagnostics (`bisection`).
- `docs/ROADMAP.md` §C2 status line: M7 done.
- `docs/BACKLOG.md`: findings `negative-radius-solve`,
  `drag-negative-radius`, `whole-false-certificate`,
  `staged-conflict-inconclusive` → fixed; `satisfied-status-degenerate-tangent`
  if lifted.
- `GLOSSARY.md`: **fundamental circuit**, **bisected frame**; **Bisection**
  sharpened (a sweep jump bisects too).
- `CHANGELOG.md` Unreleased: radius never negative, `bisection`
  (Breaking), fewer false or missing conflict sets.
- `AGENTS.md` current state.

## Open questions

- *Settled* (step 1, ADR-0010): both — a projected step with an active
  set (an active radius's column leaves the model and takes no step) and
  every step cut at 0; the certificate reads the projected gradient. Both
  fixtures end `UnderConstrained` (DoF 5 and 4; `negative-radius-solve`'s
  `expected.json` DoF corrected from 3 by count). C1 digests unchanged.
  `satisfied-status-degenerate-tangent` is not lifted: its radius ends
  positive; its fault is the status at a collapsed contact.
- ⚠ OPEN: the false certificate's fix (another start or one re-solve path
  for both filters). The agent, step 4, in the ADR-0005 amendment.
- ⚠ OPEN: `MAX_SWEEP_STEP`'s value. A move of `f` turns is read
  correctly up to half a turn, so any `f < ½` catches true moves in
  `(f, 1 − f)` turns and misses the rest; the agent picks it in step 7
  with that bound and the bisection's cost measured.
- ⚠ OPEN: does a frame bisected only for a sweep jump move the session's
  `reached`? The agent, step 7; default yes, as a flip's bisection does.
