# Plan: m6-stages

- Started: 2026-11-04
- Milestone: M6 (cycle C2, docs/ROADMAP.md)
- Idea: docs/ideas/c2-decomposition.md (its M6 milestone absorbed; settled: C with S1)

## Goal
Every component is cut into **parts** by the Dulmage–Mendelsohn decomposition
of its compiled equations: an over-determined head, square **stages** in
dependency order, an under-determined **remainder**. `solve` stays stateless
and solves part by part; a satisfied part takes no step, so an edit moves
only its stage and what depends on it, and the upstream bits stay
(`regression/upstream-edit-drift` lifted). DoF, per-entity DoF, the span of
the free directions, redundant and conflict sets equal a whole-component
analysis's on C1, the C2 corpus and the solve properties. On 500-entity
sketches a one-dimension edit, diagnostics included, is ≤ 4 ms; the drag
step is measured, and that number decides M8.

## Non-goals
- Rigid bodies in the remainder (option A), a cache (S2), or splitting the
  remainder by drag drivers: M8, only if step 9's drag cells miss.
- Drag robustness, radius ≥ 0, conflict sets against the exact minimum: M7.
- The conflict analysis's cost and its 256-re-solve bound (C2 *Out*).
- Any public type or signature; `laman-py` is untouched.
- `laman::rigidity` (the pebble game): not used by stages.

## Design deltas
- **New internal module `stage`** between `system` and `solve`
  (ARCHITECTURE.md module table, §Solve pipeline step 2). Per component: a
  maximum matching of equations to free parameters (augmenting paths in
  compiled order), the coarse DM split, Tarjan's SCCs of the square middle in
  topological order, ties to the smallest parameter. Structure is the
  gradient's term keys (coordinate-independent; a contact's regularized
  terms, ADR-0002). A constraint's equations, and an arc with its intrinsic
  equation, never split: parts sharing one merge and re-condense.
  Dulmage & Mendelsohn 1958, Tarjan 1972, cited in the module.
- **Order**: head, stages, remainder. Each part reads only its own and
  earlier parts' parameters; earlier columns are constants to it. A part
  is a `Component` subset carrying its parent's frozen scale, origin and
  acceptance, so no tolerance changes (ADR-0003).
- **Part rank** (ADR-0009): when every part before the last has full
  column rank, rank J = Σ part ranks, the null space lies in the last
  part's columns, and a dependency among rows stays inside one part.
  So DoF and free directions come from per-part QRs (ADR-0003's, rows
  unit-normalized over the whole row, then cut to the part's columns),
  redundancy is a deletion inside the head only, and a square stage is
  never redundant.
- **Fallback**: a part that fails to converge, or ends column-deficient
  before the last, sends its component back to today's whole-component
  solve and analysis, from the component's start, with the budget counters
  restored to their values at that start (caps then bound ≤ 2× a
  component's work). Failed and degenerate components therefore keep
  today's bits, conflict sets and best effort.
- **Goals** are pursued in the remainder only; a goal on a stage or head
  parameter is stationary, as its null projection is today.
- `DragSession` decomposes once at `drag` and reuses it for every frame and
  bisection (the required equations do not change in a session).
- **Reference hook**: `laman` gains a non-default feature `reference`
  exposing `#[doc(hidden)] laman::reference::solve(&Sketch,
  &SolverOptions) -> Solution`, the whole-component path, outside the
  stability promise and the binding; laman-debug's tests enable it to
  compare. The public API is unchanged. Step 7 adds
  `reference::downstream(&Sketch, ConstraintId) -> Vec<(EntityId,
  Coordinate)>`: the parameters a change of one constraint can move.
- **Scale**: `System` indexes equations by source and parameters by column
  (`BTreeMap`), contact recognition is one pass, and evaluating a part
  reads only its rows.
- **Digests**: C1 bases, redundant twins and drag steps move (staged
  descent, part-wise null bases) and are re-blessed with that reason; the
  conflicting twins must not move (fallback). `violations.json` unchanged.
- **Words**: GLOSSARY.md *Stage* sharpened (one of a component's square
  parts), new *Head* (its over-determined part, solved first: where every
  redundancy lives). "Part" stays plain English for head, stage or
  remainder; *block* stays under _Avoid_.

## Steps
Grades: **[1]** routine, the design says what to write; **[2]** careful, a
numeric or geometric case within the design; **[3]** unproven, robustness
or a bound established here. The grade is the tier (`.agents/tiers.md`).

- [x] Step 1 **[2]** — `stage`: matching, DM split, SCCs, constraint
  grouping. Tests by hand: SSS → {horizontal}→`a.y`, {base}→`a.x`,
  {left, right}→`b` (a stage is strongly connected: `horizontal` reads no
  other row's parameter, so it is a stage of its own; *Settled* at step 1);
  its redundant twin all head; fixture 4 with `width` gone → remainder;
  plate's part list by analysis (ten stages: four singletons for P1 and
  P3, `right`, `top`, two midpoints, the slot's four centre equations, its
  eight arc equations). Properties over constructed sketches (`solved.rs`'s
  Henneberg shape, built in the module's tests: laman's unit tests cannot
  share `laman-debug`'s strategies): each part reads only earlier parts,
  stages square, Σ matched = structural rank.
- [x] Step 2 **[3]** — Staged freedom (DoF, per-entity DoF, directions)
  with the column-deficiency fallback, plus the `reference` hook. Equal to
  reference: DoF and per-entity DoF exactly, direction projectors within
  `1e-9`, on C1 at expected coordinates, the corpus at constructed ones and
  the solve properties. Re-bless digests that move. (*Settled* at step 2:
  only a satisfied component is staged, a failed or degenerate one is
  whole, as the fallback delta has its solve; at 60000 random solves that
  makes DoF and per-entity DoF equal at every magnitude and projectors
  within 4e-14, where staging unsatisfied best efforts gave gaps to 8e-3.
  Moved: the drag plate's solved and drag-step digests, its free
  directions only.)
- [x] Step 3 **[2]** — Staged redundancy: head-only deletion on the
  head's columns. Equal to reference: `right-angle`, `same-belt`, `level`,
  and the redundant/conflict extra-distance property. (*Settled* at
  step 3: a constraint with no head equation skips the deletion test, in a
  component whose parts after the head are row-independent at the
  coordinates; a dependent row there, as a point on a line through a point
  it coincides with, hides its redundancy from the structure, so that
  component takes the full test. Each test still compares the whole DoF;
  the skip saves the factorizations.)
- [x] Step 4 **[3]** — Staged solve: parts in order under the shared
  budgets, the failure fallback with restored counters, goals in the
  remainder. Lift `upstream-edit-drift` (LM, DogLeg). C1 coordinates as
  listed; conflict sets equal reference; conflicting twins' digests
  unchanged; others re-blessed (`fixtures:` and why); `tools/py-test.sh`
  and `tools/digest-wasm.sh` green. (*Settled* at
  step 4: a staged result is kept only where the diagnosis stages it
  (`diagnose::stageable`), so a solve and its analysis take one path;
  goals go to the last part, a stage when there is no remainder. Staged
  and whole coordinates differ within acceptance, so the comparisons
  diagnose the staged coordinates whole (`laman::reference::diagnose`)
  and hold conflict sets to the whole solve's. Moved: the solved digests
  of the sss triangle, belt, plate, plate-redundant and plate-drag, and
  every drag step; conflicting twins, the sss and belt redundant twins
  unchanged.)
- [x] Step 5 **[2]** — `DragSession` reuses its decomposition. Fixture 4's
  numbers; every part outside the remainder keeps its bits across all ten
  steps; `bisect-stuck` unchanged; drag digests re-blessed. (*Settled* at
  step 5: the kept cut is the one a frame would make, so no digest moves and
  none is re-blessed; `System` carries an optional `Decompositions`,
  used only for the component whose parameters and sources it was cut
  from. "Outside the remainder" is tested as the entities with no freedom
  at the first step.)
- [x] Step 6 **[2]** — Compile and evaluate at scale (the indexing delta).
  Lift the seven `large_case` ignores in `tests/c2.rs` (each under 10 s in
  debug); the 498 slot grid gets a DogLeg verdict. (*Settled* at step 6:
  the caps are spent in whole-component units, a part's step or evaluation
  its share of the parameters, a whole component one, each descent also
  held to the cap: 147 parts had spent the shared 128 and fallen back
  whole. Staged, the 250/500 strips' edits took a mirror root where an
  edit carried a stage's parents further than its size (point 70 of 84:
  32.9 from the answer, side 20, the exact mirror to 4e-8). So a part that
  held at the component's start and is moved only by earlier parts starts
  from its first-order continuation `−J_x⁺ (F + J_u Δu)`: exact under
  translation, one evaluation, a fresh solve untouched, no digest moved.
  Not a joint solve of the downstream closure (the whole tail's cost),
  nor a branch check with fallback (the whole solve's cost where edits
  are). Edit medians, release, diagnostics included: rectangles 12.4 ms,
  slots 23.0, triangles 10.9 (LM; DogLeg alike), over 4 ms because every
  part's evaluation measures the whole component's extent (O(n²) a solve)
  and redundancy compiles and cuts the placed sketch again; step 9 decides.
  The slot grid converges under DogLeg, 30 ms static.)
- [x] Step 7 **[2]** — Locality held by tests: `c2.rs` and `c2_props.rs`
  compare `unmoved`, and every parameter outside the edited stage's
  downstream closure, by bits instead of tolerance; staged equals
  reference (DoF, redundant, conflicts) on every corpus edit and drag.
  (*Settled* at step 7: the closure is the structure's,
  `laman::reference::downstream`, and never reaches the construction's
  `unmoved`; on rectangles it adds the later cells' y. The reference is
  compared up to 50 entities: the whole analysis is O(n⁴), 19 s in debug
  on the 98-entity slots; the small named cases and every property case
  cover each family's edit and drag, the properties 8 → 34 s in debug.
  A seed found `whole-false-certificate`, red since step 4: see below.)
- [x] Step 8 **[1, tier 3]** — ADR-0009: stages, part rank and its proof
  sketch, the fallback and its budget, the per-part normalization
  amending ADR-0003's rank, the re-bless reasons.
- [x] Step 9 **[2]** — Re-run `bench_c2` and `bench`; record the table
  (ARCHITECTURE.md §Frame budget); state in the idea whether M8 runs.
  (*Settled* at step 9: the 500-entity acceptance is missed, edits 10.8-22.5
  ms and drags 20 ms; the idea recommends removing the two measured cost
  sources before A or S2; the human decides.)

## Acceptance
- `bench_c2`, release, reference machine: the one-dimension edit ≤ 4 ms
  median at 498/498/501 entities under both algorithms, diagnostics
  included; the drag step recorded per cell; `bench` keeps C1 at ≤ 1 ms.
  **Missed** (*Settled* at retirement): edits 10.8–22.5 ms, first drag
  step 20 ms; C1 held (0.05–0.46 ms, drag steps 0.39–0.40 ms).
- `upstream_edit_drift_{lm,dogleg}` and all `large_case` tests pass
  un-ignored; edits keep upstream bits exactly.
- Staged = reference on C1, the corpus and the properties: DoF, per-entity
  DoF, direction span, redundant sets, and conflict sets where the
  reference's are unsatisfiable (a reference set a checker-clean witness
  disproves is the reference's error; *Settled* at retirement).
- C1 fixtures' numbers as ROADMAP.md lists them; checker green; the
  re-blessed digests equal natively, on wasm32-wasip1 and from pytest.

## Docs to update on completion
- `docs/ARCHITECTURE.md` — module table (`stage`; `rigidity` no longer
  "the pre-pass"); §Solve pipeline step 2; §Diagnose (head-only
  redundancy, fallback); §Drag sessions per-step cost; §Frame budget.
- `docs/ROADMAP.md` §C2 — *In* reworded to stages (no pebble game);
  status line M6 done.
- `GLOSSARY.md` — *Head* new, *Stage* sharpened.
- `docs/adr/README.md` — ADR-0009 row.
- `docs/BACKLOG.md` — `upstream-edit-drift` State: fixed (the 498 DogLeg
  line dropped at step 6).
- `docs/ideas/c2-decomposition.md` — cut to M7/M8 (M8 per step 9).
- `CHANGELOG.md` Unreleased — large sketches solve by stages; edits keep
  unrelated geometry's bits; solutions may differ from 0.1.0 in last bits;
  `max_iterations`/`max_evaluations` count a staged component's work in
  whole-component steps (step 6; `laman-py`'s docstrings say "in all,
  across components", still true, untouched).
- `AGENTS.md` current state.

## Open questions
- *Settled* at step 2: rows are normalized over the whole row, as today,
  then cut to the part's columns, not over the part's columns. A row
  dependent on earlier parts has rounding on its own columns (1e-17 of a
  unit row); normalized over them alone it counted a rank the whole path
  does not (seed 226646 in `solved.proptest-regressions`, DoF 2 against
  3; `diagnose::tests::a_row_dependent_on_earlier_parts_adds_no_rank`).
  ADR-0009 records it.
- *Settled* at step 4: the fallback grain is the whole component, from
  its start with the counters restored; it keeps every failure's bits, and
  failures are outside the frame budget.
- *Settled* at step 8 (the option; ADR-0009): staged conflict sets are
  held to the reference's only where the reference's sets are
  unsatisfiable, not by fixing the whole path's filter first. The
  defect is the whole path's (ADR-0005's local certificate names a set a
  converged, checker-clean re-solve disproves, `whole-false-certificate`),
  the staged set is right there, and the filter's fix is M7's conflict
  work (a non-goal here). The fixture stays ignored until M7.
  *Settled* at retirement (the human ruled it technical, the agent's):
  the acceptance reads "conflict sets where the reference's are
  unsatisfiable", and the property applies the same narrowing
  (`support/staged.rs` `assert_same_conflicts`): staged and whole sets
  may differ only where every whole set the staged path lacks has a
  checker-clean witness (the set alone, or the sketch without one
  constraint outside it, from the guesses or the best effort). Not a
  weakening: the witness is the independent checker's, so the reference
  is the wrong side there and equality would assert a known-false answer;
  the staged sets keep the properties' own checks. The seed's full hash
  was never kept, so the gate meets the case deterministically through
  its fixture (`staged.rs`
  `a_whole_conflict_that_holds_together_is_not_the_staged_answer`, seen
  failing under strict equality).
- *Settled* at retirement: the 4 ms acceptance is recorded as missed with
  its numbers (ROADMAP.md §C2, ARCHITECTURE.md §Frame budget). M8 runs as
  profile-then-remove (the per-part extent, the recompile and re-cut, the
  repeated linear scans the review found), not A, S2 or a driver split:
  both measured sources lie outside the remainder's solve. The target
  holds for the 500-entity drag step too, as ROADMAP's *Accept* states;
  only a profile after that removal chooses among A, S2 and the split.
- *Settled* at retirement (review): the cut reads coordinates where an
  equation is degenerate, at a fixed generic position, else its full
  dependencies; coarser, never wrong. ADR-0009's "reads no coordinates"
  corrected; the constants named.
