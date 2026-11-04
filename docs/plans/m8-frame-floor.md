# Plan: m8-frame-floor

- Started: 2026-11-04
- Milestone: M8 (cycle C2, docs/ROADMAP.md), its second plan
- Idea: none. Verbatim from the human: "a second M8 plan for rigid bodies
  in the remainder (option A)", rescoped by the human at planning time to
  "floor first, A after". The edit cells have no remainder, so A can't
  reach them (*Design deltas*, first bullet).

## Goal
The cost every 500-entity frame pays whatever it moves, the work over all
of a component's parts, is measured, and the whole-component and per-part
costs that grow faster than the parts' own work are removed. After that
the 500-entity edits (rectangles 498, slots 498, triangles 501, LM and
DogLeg) have a worst sample ≤ 4 ms in `bench_c2`. The drag and bisect
cells get no slower, and the share of each that sits in the remainder is
measured: that number is what the rigid-bodies plan (option A) starts
from. One path, no public type, and the same bits everywhere.

## Non-goals
- **Rigid bodies in the remainder (A).** It's the next M8 plan, written
  only if a drag or bisect cell still misses after this one, from step 1's
  remainder share and step 7's table. ideas/c2-decomposition's estimate
  was about three plans (find clusters and their motion group, solve by
  bodies, diagnose by bodies). The first of them is written then, not now.
- A diagnosis cache (S2) or a driver split: both were declined in
  ARCHITECTURE.md §Frame budget.
- A change to what any diagnostic means, or to which components are staged
  (ADR-0009). Only how the same numbers are reached changes.
- The static solve's cells: reported, not budgeted, as before. The
  conflict analysis's cost (`docs/BACKLOG.md`).
- A timing assertion in the gate. The bench is the acceptance.

## Design deltas
No public type or signature change. No new glossary term.

- **Correction: A doesn't reach the edits.** Every edit and static sketch
  of the families is DoF 0 with nothing redundant. Its matching is
  perfect, so its Dulmage–Mendelsohn split has no head and no remainder:
  only stages. A can reach only the drag and bisect cells, and at best it
  takes each down to its edit's cost: rectangles 498 drag 17.4 against
  edit 7.3, slots 498 drag 12.1 against edit 11.3. ARCHITECTURE.md
  §Frame budget's "reaches all four cell kinds" is wrong and gets
  corrected.
- **Suspects read from the code**, to be confirmed or dropped by step 1:
  1. *Whole-component dense Jacobians.* `solve::satisfied` builds the
     component's dense Jacobian (rows × params, about 1 000 × 1 000) only
     to read a bool. `staged_checked` builds it again through
     `evaluate_indexed`, runs `unit_row` over full-width rows and scans
     each one in full for the triangular pattern. That is O(rows × params)
     per call, several calls a frame. Profile matches: `unit_row` 7–8 %,
     `malloc`/`free` ~8 %, `evaluate_within` 6 %.
  2. *`Splitter::split`'s joins*, O(parts²) a solve (8.5 % on the
     rectangle edit).
  3. *Per-part, per-evaluation set-up*: `rows_of`'s sort and dedup, and
     the `column` `BTreeMap` that `evaluate_within` builds on every
     evaluation of a part (`BTreeMap` lookups ~6 %, `entity_params`
     3–5 %).
  4. *The diagnosis's `stage::cut`* (6 % on rectangles).
- **Sparse staged rank** (removes 1 in the diagnosis). `staged_checked`
  builds each part's block from `System::rows`' sparse terms: each row
  unit-normalized over its nonzero entries in component column order,
  the triangular test run on its terms' parts. Bit-equal: a zero entry
  adds +0 to the running max of |x| and to the sum of squares, and the
  nonzero entries are visited in the dense order. The dense
  implementation stays under `cfg(test)` as the reference.
- **Satisfaction without a Jacobian** (removes 1 in the solve).
  `solve::satisfied` reads `System::rows`' satisfied and valid flags. The
  validity term equals the dense one: the dense row's finite entries are
  exactly its in-component `term × factor`.
- **Splitter in O(parts)** (removes 2). The joins are max, min and
  extent: associative and commutative, so bit-exact in any grouping.
  Outside part `k` is the fixed terms, joined with a running join of the
  groups already moved (measured after they moved) and a suffix join of
  the groups not yet moved (measured at the start, which still holds for
  them). The bit-equality property `split_frame_measures_what_the_whole_does`
  holds it, f64's extremes included.
- **Per-part set-up once per descent** (removes 3): a part's rows (already
  ascending in the decomposition) and its column map are built when its
  descent starts and reused for every evaluation in it. Removed only if
  step 1 shows ≥ 5 % for it.
- **Thresholds.** A suspect is removed when step 1 measures it at ≥ 5 % of
  a 500-entity edit, or finds it grows faster than the part count
  (98 → 248 → 498 scaling). Below that it's dropped and the step struck
  through with the number.

## Steps
Grades are tiers (`.agents/tiers.md`): **[1]** routine, **[2]** careful,
**[3]** unproven.

- [x] Step 1 **[2]** — Profile, using step 1's method from the first M8
  plan (`perf record --call-graph fp`, frame pointers, a separate target
  dir, `--samples 20`). Cells: the 500-entity edits, slots and rectangles
  drag, the bisect, plus rectangles 98/248/498 edit for scaling. Record
  under *Profile* below each suspect's self and inclusive share, how it
  scales with size, anything unnamed ≥ 5 %, and each drag or bisect
  cell's share inside the remainder's descent, pursuit and null basis
  (A's ceiling). Reorder or strike steps 2–5 to match. Commit:
  `docs(plan)`.
- [x] Step 2 **[2]** — Sparse staged rank, as in the deltas. Test: rank,
  null basis and every `None` bit-equal to the dense reference on the C1
  fixtures, every generated family and size, and a proptest over the
  corpus strategies at f64's extremes.
- [ ] Step 3 **[1]** — Satisfaction without a Jacobian, as in the deltas.
  Test: equal to the dense `evaluate(..).satisfied` on the same inputs,
  non-finite and degenerate rows included; digests unchanged.
- [ ] Step 4 **[2]** — Splitter in O(parts), as in the deltas. Test: the
  existing bit-equality property over a sequence of parts moving in turn,
  plus a 500-part case.
- [ ] Step 5 **[1]** — Per-part set-up once per descent, if step 1 keeps
  it. Test: the solve suites and digests unchanged.
- [ ] Step 6 **[2]** — Re-profile the 500-entity edits. Remove any
  remaining source ≥ 5 % that is not the solve's own arithmetic, each
  with its own equality test (`stage::cut` in the diagnosis is the named
  candidate), or split it into a step of its own if it's larger.
- [ ] Step 7 **[1]** — Run `bench_c2` in full. Replace the table in
  ARCHITECTURE.md §Frame budget, with the attribution and A's measured
  ceiling per drag and bisect cell. Correct "reaches all four cell
  kinds". If an edit misses, record the miss and what the profile names
  next. Commit: `docs`.

## Acceptance
- `cargo run --release -p laman-debug --example bench_c2` on the reference
  machine (AMD Ryzen 9 7950X): the rectangles 498, slots 498 and
  triangles 501 edits, LM and DogLeg, each with a **worst** sample ≤ 4 ms
  out of five.
- The 500-entity drag cells and the slot grid's bisect cell are no slower
  than in the current table (medians within run-to-run noise or below).
  Their remaining distance to 4 ms and the remainder's share of it are
  recorded: that is the input to the A plan, not a pass/fail here.
- The C1 bench keeps its 1 ms budget, and no 98/99 or 248/249 cell gets
  slower.
- Bits: the C1 corpus's 50 digests are unchanged, natively and on
  wasm32-wasip1. The M6 locality test and the staged-against-whole
  properties (DoF, per-entity DoF, direction span, redundant and conflict
  sets) are green at `PROPTEST_CASES=1024`. No fixture expectation
  changes.
- `tools/gate.sh` and `tools/py-test.sh` are green.

## Docs to update on completion
- `docs/ARCHITECTURE.md` §Frame budget: the new table, the attribution,
  the corrected reach of A (drag and bisect only) and its measured
  ceiling. §Diagnose: the staged rank read from sparse rows. §Drag,
  *Per-step diagnostics*: the 500-entity sentence.
- `docs/ROADMAP.md` §C2: the status line, and the M8 line saying "the
  frame floor first, then A if a drag cell still misses".
- `AGENTS.md` current state.
- `CHANGELOG.md` under Unreleased: the 500-entity edit and drag times, if
  they changed. No API change.

## Profile
Step 1, 2026-11-04, AMD Ryzen 9 7950X, LM (DogLeg agreed within 2 points
in the first plan's profile and is not re-run). Method: `perf record -F 4000
--call-graph fp` of `bench_c2 --worker F S LM OP --samples 20`, built with
`RUSTFLAGS="-C force-frame-pointers=yes"` and
`CARGO_PROFILE_RELEASE_DEBUG=true` in a separate target dir. Inlining puts
the whole tree under `main`, so an inclusive share is `perf report
--children` (the fraction of samples whose stack holds the function,
inlined frames included); the shares overlap and do not sum. Percent of
all samples (the timed operation is 91-95 % of an edit's, 53-73 % of a
drag's: the rest is the worker's set-up, outside the timing).

| source (inclusive) | rect 98 edit | rect 248 edit | rect 498 edit | slots 498 edit | tri 501 edit | rect 498 drag | slots 498 drag | slots 498 bisect |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `stageable` (diagnosis' staged rank) | 13.5 | 15.5 | 17.1 | 35.4 | 19.4 | 13.5 | 33.7 | 37.0 |
| `satisfied` (solve; dense Jacobian) | 9.3 | 6.7 | 8.9 | 17.7 | 8.9 | 3.8 | 23.0 | 18.2 |
| `Splitter::split` (self) | 0 | 3.4 | 7.4 | 3.7 | 2.1 | 2.8 | 2.7 | 3.3 |
| `evaluate_within` (per-evaluation) | 15.2 | 16.4 | 15.5 | 26.6 | 19.0 | 9.5 | 33.5 | 35.9 |
| `entity_params` (column map) | 7.3 | 12.4 | 9.2 | 7.0 | 3.0 | 6.2 | 6.6 | 6.3 |
| `rows_of` | 1.4 | 1.6 | 1.3 | 0.6 | 0.6 | 0.3 | 0.3 | 0.6 |
| `stage::cut` (decompose) | 15.5 | 13.8 | 12.2 | 9.1 | 22.3 | 1.7 | 2.3 | 1.4 |
| `Frame::terms` (self) | n/a | 7.4 | 7.5 | 2.2 | 4.2 | 1.1 | 1.9 | 4.6 |
| `redundant`/`head_constraints` | 4.2 | 2.4 | 2.0 | 1.7 | 1.2 | 1.8 | 1.7 | 1.0 |
| `unit_row` (self) | n/a | 2.0 | 3.1 | 5.3 | 2.3 | 2.1 | 8.1 | 7.8 |

Suspects. (1) Confirmed on both sides: the solve's `satisfied` is 7-23 %
(its dense `evaluate` is most of it) and the diagnosis' `stageable` 14-37 %,
the largest single inclusive source in every cell but the rectangle drag;
`malloc`/`free` (15-18 % inclusive) sit under both. (2) `Splitter::split`
grows faster than the parts (0, 3.4, 7.4 % over 98, 248, 498): kept. (3)
`entity_params` is 3-12 % and `evaluate_within` 10-36 % inclusive; `rows_of`
is under 2 %, so only the column map is kept, not the rows. (4) `stage::cut`
is 12 % (rectangles) to 22 % (triangles) of an edit and under 3 % of a
drag or bisect: kept as step 6's named candidate. Unnamed source at or over
5 %: `Frame::terms`, `Splitter::split`'s callee, 7.5 % self on the
rectangle edits (4.6 % on the bisect), which step 4 reaches. No step is
struck or reordered.

The remainder (A's ceiling, rectangles 498 first drag step): dense
`Qr::new` 22.6 % self, nalgebra's SVD (`try_new`) 18.6 %, its Householder
reflections 9.8 %: about 51 % of the samples, so about 70 % of the timed
operation, is the remainder's rank and least-squares steps; `null_basis` is
0.1 %. The slot drag (`Qr`/SVD not in its top self entries) and bisect have
no dominant remainder cost: their top self entries are `unit_row` 8 %,
`evaluate_within` 6-7 %, `factorize` 4 %, i.e. the same per-part work this
plan removes; A's share there is not measured by this profile and needs the
descent/pursuit split step 7 asks for.

## Open questions
- Settled by the human (planning, 2026-11-04): this plan does the floor;
  A is the next M8 plan and runs only if a drag or bisect cell still
  misses.
- ⚠ OPEN (agent, step 1): the 5 % threshold. If the whole profile is flat
  below it, the step records the five largest sources and steps 2–5 run
  only for the suspects that grow faster than the part count.
- ⚠ OPEN (agent, step 7): if the edits meet 4 ms but the drag cells still
  miss by less than the remainder's measured share, the A plan's goal is
  set from that share. If they miss by more, the step records that A
  alone won't close M8 and names what the profile points to.
