# Plan: m8-setup

- Started: 2026-11-04
- Milestone: M8 (cycle C2, docs/ROADMAP.md), its third plan
- Idea: none. Verbatim from the human: "next m8 part", which is the roadmap's
  M8 line: "a third M8 plan … on the set-up costs the profile names
  (`parts_of`'s quadratic lookups, `stage::cut`, the allocator,
  `Frame::terms`)".

## Goal
A frame does not set itself up twice. An edit cuts its components into
parts once, and the solve and the diagnosis share that cut. A part's
parameters and sources are read off the cut in one pass, with no
per-part scan of the component and no copy of the decomposition. What
step 1 finds the allocator is called for in the frame's set-up is gone.
After that, the 500-entity edits (rectangles 498, slots 498, triangles
501, LM and DogLeg) have a worst
sample ≤ 4 ms in `bench_c2`; the slot grid's first drag step is recorded. The rectangle drag and the bisect cell get
no slower, and their distance to 4 ms is recorded: the rigid-bodies plan
(A) starts from it. One path, no public type, and the same bits
everywhere.

## Non-goals
- **Rigid bodies in the remainder (A)**: the next M8 plan. The rectangle
  drag (15.0 ms, pursuit 63 %) and the bisect (15.0 ms) are out of this
  plan's reach alone (ARCHITECTURE.md §Frame budget, A's ceiling).
- The pursuit's dense `Qr::new` and null basis (A's cost). A diagnosis
  cache (S2) or a driver split (declined in §Frame budget).
- Coordinates as a dense slice, not a `BTreeMap` (too large for a step,
  `plans/m8-frame-floor` step 6): its own plan if step 7 finds
  `Frame::terms`' lookups are what's left.
- A change to what a diagnostic means, which components are staged or
  how they are cut (ADR-0009); the conflict analysis's cost; a timing
  assertion in the gate.

## Design deltas
No public type or signature change. No new glossary term. No ADR: each
step is the same decomposition reached with less work, held by equality.

- **One cut a frame.** Without kept decompositions (an edit, a static
  solve), `solve_under`'s `System` cuts every component on first ask
  (`System::cut`), and `analyse_under` compiles a new `System` of the same
  sketch under the same frozen tolerances and cuts it again. The cut reads
  the sketch's stored coordinates and its structure only, so the two cuts
  are equal. The solve hands its cut on: `Outcome` (or `Attempt`) takes
  `system.cut.take()`, and `conclude` passes it as `kept` when the caller
  had none. `Decompositions::get` already refuses an entry whose
  parameters or sources differ, so if a component doesn't match it is
  just cut again, never wrong.
- **`decompose` borrows.** It returns `Cow<'_, Decomposition>`, borrowed
  from the kept or cached cut and owned only on the uncached fallback. It
  no longer clones every part's rows and parameters on each call (four
  callers a frame: `parts_of`, `stageable`, the staged rank, and
  `head_constraints`).
- **Parts in one pass.** `Part` gains `sources: Vec<Source>`, filled by
  `cut` from its rows (equations are in source order, so a dedup of
  consecutive sources). A component's parameters come sorted (the
  sketch's coordinate keys) and its sources in `Source` order (arcs, then
  constraints, by id). So `part.params` and `part.sources` already are
  the filter `parts_of` computes over the whole component for each part.
  `parts_of` copies them. The old `parts_of` stays under `cfg(test)` as
  the reference.
- **The cut's own work**, if step 1 measures it ≥ 5 % after the sharing:
  `support_at` evaluates a constraint's equations once per source rather
  than once per equation, and builds the generic position once per cut
  rather than once per degenerate row. The matching gets a greedy first
  pass only if `augment` alone is ≥ 5 %. That's safe because the coarse
  Dulmage–Mendelsohn split, and the strongly connected components of its
  square middle, don't depend on which maximum matching is found. The
  parts, their order, kinds and `matched` counts are the same. The
  current `cut` stays under `cfg(test)` as the reference.
- **Allocation in the set-up**, the sources step 1 attributes: candidates
  read from the code are `Row::terms` as a `BTreeMap` built per row per
  evaluation (a sorted `Vec` with the same entries in the same order, the
  map's last-wins on a repeated key kept), and the `Coordinates` clones
  per descent and try (`solve_staged`'s start, `descend`'s `next`).
- **Thresholds** (the frame-floor plan's): removed at ≥ 5 % of a
  500-entity edit or the slot drag, or growing faster than the parts;
  otherwise the step is struck through with the number.

## Steps
Grades are tiers (`.agents/tiers.md`): **[1]** routine, **[2]** careful,
**[3]** unproven.

- [x] Step 1 **[2]**: Profile with the frame-floor method (`perf record
  -F 4000 --call-graph fp`, frame pointers, a separate target dir,
  `--samples 20`). Cells: the six 500-entity edits, the slots and
  rectangles drag, the bisect. Record under *Profile*: how many times an
  edit runs `stage::cut` and the share each takes, `parts_of` split into
  `decompose`'s clone and the filter, and malloc/free split by caller
  (`perf report --children` on the allocator, by caller). Strike or
  reorder steps 2–6 to match. Commit: `docs(plan)`.
- [x] Step 2 **[2]**: One cut a frame, as in the deltas. Test: the
  diagnosis's decompositions equal a fresh cut on the C1 fixtures and
  every family and size; an edit cuts once (a `cfg(test)` count); digests
  unchanged.
- [x] Step 3 **[1]**: parts come out in one pass, as in the deltas
  (`decompose` borrowing struck: its clone is 0.3-2.9 %). Test: `parts_of` equals the reference on the C1
  fixtures, every family and size, and the corpus proptest strategies.
- [x] Step 4 **[2]**: The cut's own work (kept: 8-15 % of an edit after
  sharing); the greedy first pass only for the triangles (see *Profile*). Test:
  `cut` equals the reference (parts, kinds, order, `matched`, rank) on
  the same set, degenerate starts included (coincident ends, the generic
  position's fallback), and at `PROPTEST_CASES=1024`.
- [x] Step 5 **[2]**: The largest allocation source step 1 names: the
  `Coordinates` clones per descent and try (`Row::terms` struck: no map is
  built there). Test: the solve suites and digests unchanged.
- ~~Step 6~~ struck: no other allocation source reaches 5 % (the next:
  `descend` 3.9 % on the triangle DogLeg edit, the cut's own 0.9-3.6 %,
  the staged rank 0.9-2.0 %).
- [x] Step 7 **[2]**: Re-profile the 500-entity edits and the slot drag.
  Remove any remaining source ≥ 5 % that isn't the solve's own
  arithmetic, each with its own equality test, or split it into a step
  of its own (7a) if it's larger.
- [x] Step 8 **[1]**: Run `bench_c2` in full. Replace the table in
  ARCHITECTURE.md §Frame budget, with the attribution and A's measured
  ceiling for the rectangle drag and the bisect, restated. If a cell in
  *Acceptance* misses, record the miss and what the profile names next.
  Commit: `docs`.

## Acceptance
- `cargo run --release -p laman-debug --example bench_c2` on the reference
  machine (AMD Ryzen 9 7950X): the rectangles 498, slots 498 and
  triangles 501 edits, LM and DogLeg, each with a **worst** sample ≤ 4 ms
  out of five. The slots 498 first drag step is recorded, not asserted
  (*Open questions*, step 1).
- The rectangles 498 drag and the slot grid's bisect cell are no slower
  than in the current table (15.0–16.0 ms, 15.0–15.8 ms). Their distance
  to 4 ms and the pursuit's share of it are recorded: that's the A plan's
  input, not a pass/fail here.
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
  A's restated ceiling. §Solve, where the staged solve and the diagnosis
  are described: one cut a frame, shared by the solve and the diagnosis.
- `docs/ROADMAP.md` §C2: the status line and the M8 line (set-up done;
  what's left, and whether A is next).
- `AGENTS.md` current state.
- `CHANGELOG.md` under Unreleased: the 500-entity edit and drag times, if
  they changed. No API change.

## Profile
Step 1, 2026-11-05, AMD Ryzen 9 7950X. Method: `perf record -F 4000
--call-graph fp` of `bench_c2 --worker F 2 ALGO OP --samples 20`, built with
`RUSTFLAGS="-C force-frame-pointers=yes"` and
`CARGO_PROFILE_RELEASE_DEBUG=true` in a separate target dir. Inlined frames
make shares inclusive (they overlap, they do not sum); percent of all
samples (the timed operation is about 91-95 % of an edit's). The cut count
is from a throw-away `eprintln!` in `stage::cut`, not committed.

`stage::cut` runs **twice** per component in an edit: 4, 2, 2 calls a
sample on rectangles, slots, triangles (2 per component, one for the solve,
one for the diagnosis), and the two shares are equal. A drag or bisect
frame cuts 0 times (kept from the session).

| inclusive % | rect edit LM/DL | slots edit LM/DL | tri edit LM/DL | rect drag | slots drag | slots bisect |
|---|---:|---:|---:|---:|---:|---:|
| `stage::cut`, solve | 7.9 / 8.1 | 8.8 / 8.7 | 14.6 / 14.0 | 0 | 0 | 0 |
| `stage::cut`, diagnosis | 8.1 / 8.8 | 8.8 / 8.0 | 15.1 / 15.7 | 0 | 0 | 0 |
| of the cuts: `augment` (matching) | 7.4 / 6.1 | 0.3 / 0.8 | 21.6 / 19.7 | 0.3 | 0.1 | 0.1 |
| `parts_of` | 18.2 / 18.6 | 20.6 / 20.5 | 21.9 / 20.9 | 3.6 | 12.9 | 14.0 |
| `parts_of` without the cut (the filter) | 10.1 / 10.0 | 11.3 / 11.5 | 6.9 / 6.9 | 3.3 | 12.8 | 13.9 |
| `decompose`'s clone | 1.6 / 1.2 | 0.8 / 2.9 | 1.4 / 1.5 | 0.8 | 1.1 | 0.3 |
| `Frame::terms` (with callers inlined) | 11.9 / 11.9 | 5.3 / 4.1 | 7.4 / 5.4 | 1.2 | 4.0 | 9.8 |
| `stageable` (staged rank; not set-up) | 11.7 / 10.5 | 17.3 / 17.6 | 7.4 / 9.2 | 12.3 | 18.4 | 20.5 |
| malloc/free | 14.7 / 12.4 | 12.1 / 11.3 | 15.3 / 13.2 | 6.0 | 11.1 | 10.7 |
| of it under `Coordinates` clones | 2.9 / 1.6 | 0.8 / 0.4 | 5.4 / 2.0 | 0.3 | 0.5 | 1.4 |
| of it under the cut | 1.9 / 1.8 | 3.1 / 3.6 | 0.9 / 1.4 | 0.3 | 0.9 | 0.1 |

Reading it against the steps. (2) Kept: the second cut is 8-15 % of an
edit (16-30 % for both). (3) The filter is 7-11 % of an edit and 13-14 % of
the slot drag and bisect (over the threshold; quadratic in the parts);
`decompose`'s clone is 0.3-2.9 %, so only the one-pass parts stay. (4)
Kept: after sharing the cut is 8-15 % of an edit; its matching (`augment`)
is over 5 % only on the triangles (about 10 % after sharing), 3-4 % on the
rectangles, under 1 % on the slots, so the greedy first pass is the
triangles' alone and `support_at` is what is left of the cut elsewhere.
(5) `Row::terms` is an accessor on a fixed array, not a map: nothing is
built there. The `Coordinates` clones (`clone_subtree`) are inclusive 10.6 %
of the triangle edit (malloc 5.4 of it), 4.1 % of the rectangle edit, 1.4 %
of the slot edit: step 5, for the triangles and rectangles. No other
allocator caller is at 5 %: step 6 struck. `Frame::terms` is 4-12 % (its
lookups in the coordinate map): the dense `Coordinates` plan, if step 7
still finds it.

Reach. All edits: sharing the cut (-8 to -15 points) and the filter (-7 to
-11) leave 4.6, 6.4 and 4.7 ms worst at about 3.6-4.0, 4.9-5.3 and 3.7-4.0:
the rectangle and triangle edits can pass, **the slot edit (staged rank
17 %, not set-up) probably cannot**. The slot drag's first step has no cut
to share and its set-up is the filter (13 %) and `Frame::terms` (4 %): at
most 17 % of the samples against a 6.0 ms step needing -33 %.

Step 7, 2026-11-05, after steps 2-5, same method (`--samples 20`, inclusive
percent of all samples). `stage::cut` runs once per component and is 7.4-11.0
% of an edit (the diagnosis no longer cuts); `parts_of` is 7.8-11.7 %, nearly
all of it that cut (its filter is 0-0.8 %, `decompose`'s clone 1.2-4.6 %);
the `Coordinates` clones are 0.2-2.6 % of the malloc share (step 5 took the
per-try one), the whole allocator 11.8-17.0 % on the edits and 7.0-13.7 % on
the drags.

| inclusive % | rect edit LM/DL | slots edit LM/DL | tri edit LM/DL | rect drag | slots drag | slots bisect |
|---|---:|---:|---:|---:|---:|---:|
| `stage::cut` | 9.9 / 8.4 | 11.0 / 9.5 | 7.4 / 9.3 | 0 | 0 | 0 |
| `Frame::terms` | 14.2 / 13.8 | 5.7 / 5.8 | 9.8 / 11.3 | 1.1 | 4.9 | 11.3 |
| `stageable` (staged rank) | 14.4 / 13.6 | 22.9 / 21.2 | 10.5 / 11.4 | 12.2 | 19.9 | 24.1 |
| malloc/free | 16.3 / 17.0 | 11.8 / 11.8 | 16.4 / 15.3 | 7.0 | 13.0 | 13.7 |

Settled (agent, step 7): no source left is both at least 5 % and removable by
a set-up step. The cut is the one cut a frame now, its matching 0-1.6 %, its
own work already reduced in step 4. `stageable` is the staged rank, the
diagnosis proper (A's and the staged-rank cost, not set-up). `Frame::terms`
(5.7-14.2 % on the edits, 11.3 % on the bisect) is the coordinate map's
lookups: the dense `Coordinates` plan of *Non-goals*, a plan of its own
(7a), not a step here. Nothing is removed in this step.

## Open questions
- Settled (agent, step 1): the slot drag is not in reach. Its
  diagnosis is mostly the staged rank (`stageable` 18 %), and the set-up
  it can lose is the filter and `Frame::terms` (about 17 %, against the
  -33 % it needs). It moves from *Acceptance* to "recorded", with its
  number, and the A plan takes it. The slot edit may also miss for the same
  reason (staged rank 17 %): step 8 records it.
- Settled (agent, step 8, bench 2026-11-05): the triangles 501 edits pass
  (worst 3.26 / 2.91 ms), the rectangles 498 LM edit passes (3.92) and its
  DogLeg misses by 4 % (4.17), the slots 498 edits miss (6.05 / 4.66; staged
  rank 21-23 %). The slot drag is 5.0 ms (recorded), the rectangle drag 14.5
  and the bisect 11.6, none slower. An edit still misses, so A waits: the
  profile points to the dense `Coordinates` plan (`Frame::terms` 6-14 %) and
  a cheaper staged rank, both named in ARCHITECTURE.md §Frame budget.
- Human, before the A plan (not needed to start this one): A was
  estimated at about three plans (`ideas/c2-decomposition`). Whether the
  rectangle drag and the bisect stay in C2's *Accept* or move to a later
  cycle is a change to the cycle's scope.
