# Plan: m8-dense-rank

- Started: 2026-11-05
- Milestone: M8 (cycle C2, docs/ROADMAP.md), its fourth plan
- Idea: none. Verbatim from the human: "on the dense Coordinates slice
  (Frame::terms, 6–14 %) and a cheaper staged rank (10–24 %, the slot edit's
  blocker)."

## Goal
A frame evaluates a component's rows for the staged rank once and tests its
satisfaction once, and the solve and the diagnosis read coordinates from a
dense table instead of a `BTreeMap`. The six 500-entity edits (rectangles
498, slots 498, triangles 501, LM and DogLeg) each have a worst sample
≤ 4 ms in `bench_c2`. The slot drag and the bisect are measured and no
slower. One path, no public type, the same bits everywhere.

## Non-goals
- **Closing M8.** The rectangle drag (14.5 ms, pursuit 60 %) and the bisect
  (11.6 ms) stay over 4 ms: neither source here reaches the pursuit. Whether
  they stay in C2's *Accept* is the human's (*Open questions*).
- Rigid bodies in the remainder (A), a diagnosis cache (S2), a driver split.
- `Qr` itself: `new` is at most 5 of the slot edit's 22 points under
  `stageable`, and its order of operations is what the digests pin.
- `Tolerances::build` (8–15 %), unless step 9 finds the edits need it.
- The public `Coordinates` alias, `Solution::coordinates`, `residual::*`,
  `Tolerances::at`; the conflict analysis's cost; a timing test in the gate.

## Design deltas
No public type or signature change, no ADR, no glossary term: each step
reads the same numbers in the same order, held by equality and the digests.

- **One row pass.** `factorize` calls `satisfied`, which reads
  `system.rows(component, component, values, 1.0)`, then makes the same call
  itself. `sparse_satisfied` splits into the read and a test over its rows,
  and `factorize` keeps the rows for its blocks.
- **Satisfaction once.** `analyse_under` tests each component again at the
  coordinates `stageable` tested. `StagedRanks`' `Entry` keeps the answer
  under its existing key (every coordinate the component reads, by bits);
  a new `StagedRanks::satisfied` reads it or computes it. A frame whose
  pursuit moved the coordinates misses the key and computes, as now.
- **Reads once.** `component_reads` runs on every `StagedRanks::read`, a hit
  included (about 4 of its 6 points are hits). It moves to a
  `Component::reads: Arc<[Param]>`, made where the component is.
- **`Values`** (new, `crates/laman/src/values.rs`, `pub(crate)`): a trait
  with `get(Param) -> Option<f64>`, implemented for `Coordinates`, so the
  read paths become generic with no behaviour change. Public functions
  (`residual::equations`, `intrinsic`) keep their `&Coordinates` signatures
  over `pub(crate)` generic bodies.
- **`Dense`** (same file): a shared `Layout` and a `Vec<f64>`. The layout
  holds every sketch parameter's slot; a clone is a `memcpy`. **Ids are the
  caller's `u32`s**: `from_form` takes any, and removal leaves holes, so a
  table indexed by raw id could allocate 96 GB from a small file. A direct
  table covers ids below a named `DIRECT_SPAN` (a multiple of the entity
  count, measured by step 6), and ids past it go through a sorted vector
  searched by binary search. Which route an id takes never changes the value
  read. A key the sketch lacks reads `None`, as the map does.
- **The boundary.** `Outcome::coordinates` becomes `Dense`, and a
  `Coordinates` is made once, where a `Solution` is. `diagnose::merged`
  goes on the frame's path, every sketch parameter being in the table. The
  conflict analysis keeps its maps, converting at its entry.
- **Thresholds** (the earlier plans'): removed at ≥ 5 % of a 500-entity
  edit or the slot drag; otherwise struck through with the number.

## Steps
Grades are tiers (`.agents/tiers.md`): **[1]** routine, **[2]** careful,
**[3]** unproven. The rank steps come first: they are the slot edit's
blocker and the largest measured share, and the dense steps are then judged
on what is left.

- [x] Step 1 **[1]**: Make the staged-rank proptest bite before `factorize`
  changes: add triangles, and assert the components are staged, not
  vacuously whole (the `if let Ok` / `built > 0` of `diagnose/tests.rs`).
- [x] Step 2 **[2]**: One row pass, as in the deltas. Test: `StagedRanks`
  bit-equal to `three_pass` on the C1 fixtures and every family; a failed or
  degenerate component still goes whole; digests unchanged.
- [x] Step 3 **[2]**: Satisfaction once. Test: a `cfg(test)` count of one
  test per component in an edit, and a drag whose pursuit moved the
  coordinates still computes afresh; statuses and digests unchanged.
- [x] Step 4 **[1]**: Reads once. Test: `Component::reads` equals a fresh
  `component_reads` on the C1 fixtures and every family.
- [x] Step 5 **[1]**: Run the six edits and the slot drag through `bench_c2`
  and record them under *Profile*, with `Frame::terms`' and the map search's
  shares now. Commit: `docs(plan)`.
- [ ] Step 6 **[2]**: `Values` and `Dense`, the read paths generic
  (`Reader`, `Contact::evaluate`, `Frame::terms`, `Splitter`, `System::rows`),
  callers still on maps. `DIRECT_SPAN` measured here. Test: a proptest that
  `Dense` reads what the map reads, ids at 0, `u32::MAX`, holes after a
  removal and non-finite values included; a file with one huge id allocates
  by its entity count; `Frame::terms` equal on both (closes the backlog gap).
- [ ] Step 7 **[2]**: `Outcome::coordinates` dense through the solve
  (`descend`, `continued`, `pursue`, a descent's in-place trial), a
  `Coordinates` made at the boundary. Test: the solve suites, the locality
  test by bits, digests unchanged.
- [ ] Step 8 **[2]**: The diagnosis, drag and branch observation read
  `Dense`, `merged` off the frame's path. Test: the staged-against-whole
  properties at `PROPTEST_CASES=1024`; digests unchanged, native and
  wasm32-wasip1.
- [ ] Step 9 **[2]**: Re-profile by the method under *Profile*. Remove any
  source left ≥ 5 % that is not the solve's arithmetic, with its own equality
  test, or split it off as 9a, `Tolerances::build` first if an edit misses.
- [ ] Step 10 **[1]**: Run `bench_c2` in full; replace the table in
  ARCHITECTURE.md §Frame budget with the attribution and A's ceiling
  restated. A cell of *Acceptance* that misses is recorded with what the
  profile names next. Commit: `docs`.

## Acceptance
- `cargo run --release -p laman-debug --example bench_c2` on the reference
  machine (AMD Ryzen 9 7950X): the rectangles 498, slots 498 and triangles
  501 edits, LM and DogLeg, each with a **worst** sample ≤ 4 ms of five.
- Recorded, not asserted: the slots 498 first drag step (5.0 ms) and its
  distance to 4 ms. The rectangle drag (14.5–14.6 ms) and the bisect
  (11.6 ms) are no slower, their distance to 4 ms and the pursuit's share
  restated.
- The C1 bench keeps its 1 ms budget; no 98/99 or 248/249 cell gets slower.
- Bits: the C1 corpus's 50 digests unchanged, natively and on wasm32-wasip1.
  The M6 locality test and the staged-against-whole properties green at
  `PROPTEST_CASES=1024`. No fixture expectation changes.
- `tools/gate.sh` and `tools/py-test.sh` green.

## Docs to update on completion
- `docs/ARCHITECTURE.md` §Frame budget: the table, the attribution, A's
  ceiling. §Solve and §Diagnose: one row pass, satisfaction read from the
  frame's factorization, the dense table and its boundary. §Determinism: a
  read's route never changes its value.
- `docs/ROADMAP.md` §C2: the status line and the M8 line.
- `docs/BACKLOG.md`: drop the dense-and-rank line (keep A), the proptest
  line, and `Frame::terms` from the equality-test line.
- `AGENTS.md` current state.
- `CHANGELOG.md` under Unreleased: the 500-entity times, if a consumer sees
  them. No API change.

## Profile
Planning, 2026-11-05, AMD Ryzen 9 7950X, at a4dc89d. Method: `perf record
-F 4000 --call-graph fp` of `bench_c2 --worker F 2 ALGO OP --samples 300`,
built with `RUSTFLAGS="-C force-frame-pointers=yes"` and
`CARGO_PROFILE_RELEASE_DEBUG=true` in a separate target dir; stacks folded
from `perf script`. Inclusive percent of all samples, setup included; shares
overlap. LM cells; the slot DogLeg edit is within a point of LM.

| inclusive % | rect edit | slots edit | tri edit | slots drag | bisect |
|---|---:|---:|---:|---:|---:|
| `stageable` | 13.8 | 22.4 | 12.2 | 20.8 | 24.1 |
| of it `factorize` | 9.2 | 19.0 | 10.1 | 17.5 | 20.3 |
| of it `satisfied` | 3.1 | 6.5 | 3.2 | 6.2 | 7.1 |
| of it `new` (`Qr::new`, at most) | 4.4 | 5.0 | 2.8 | 4.8 | 5.4 |
| `satisfied`, whole frame | 6.3 | 13.0 | 6.9 | 14.9 | 12.2 |
| `component_reads`, whole frame | 7.3 | 6.3 | 2.0 | 6.2 | 4.4 |
| `Frame::terms` | 13.0 | 5.4 | 11.5 | 5.3 | 12.2 |
| `Tolerances::build` | 7.7 | 11.4 | 9.7 | 15.0 | 9.0 |

The staged rank is mostly evaluation, not factorization. On the slot edit
`system.rows` runs 10.4 points under `satisfied` and 5.5 under `factorize`,
the same call on the same arguments, and `satisfied` runs twice a frame.

Reach, estimated, on the slot edit: one row pass and one satisfaction about
-13 points, reads once -4 to -6, dense -5 (`Frame::terms`, and the map
searches the first two leave). That is about -23 %: 4.87 → 3.7 ms median and
4.60 → 3.5 (DogLeg). The rectangles and triangles gain most from dense.

After steps 2-4 (b090ff2), 2026-11-07, the same machine, `bench_c2` in full,
median / worst of five, ms:

| cell | before (planning) | now |
|---|---|---|
| rectangles 498 edit LM / DogLeg | worst 4.2-6.1 range | 3.48 / 3.67, 3.49 / 3.51 |
| slots 498 edit LM | 4.87 / 6.05 | 3.87 / 4.24 |
| slots 498 edit DogLeg | 4.60 / 4.66 | 3.78 / 3.79 |
| triangles 501 edit LM / DogLeg | (inside 4) | 2.54 / 2.75, 2.53 / 2.61 |
| slots 498 drag LM / DogLeg | 5.0 | 4.34 / 5.04, 4.63 / 4.65 |
| rectangles 498 drag LM / DogLeg | 14.5 | 14.21 / 14.29, 14.87 / 15.21 |
| slots 498 bisect LM | 11.6 | 10.43 / 10.44 |

Only the slots LM edit's worst (4.24) misses 4 ms. The shares of
`Frame::terms` and the map search are **not measured**: `perf` is refused on
this machine (`kernel.perf_event_paranoid` is 4, even for user-only
`cpu-clock:u`), and the setting is the human's. Step 9 needs it lowered, or
a profile from the human.

## Open questions
- ⚠ OPEN (agent, step 10): the slots LM edit's worst sample was 6.05 ms
  against a 4.87 median, and DogLeg's 4.66 against 4.60. At -23 % that worst
  is 4.7 if the outlier is real. Five samples are the acceptance, so step 10
  reports what they give; a repeated outlier goes to step 9's profile, and
  the bar is not moved.
- ⚠ OPEN (agent, step 6): `DIRECT_SPAN`, measured.
- ⚠ OPEN (human, before the next M8 plan; not needed to start this one):
  whether the rectangle drag and the bisect stay in C2's *Accept* or move to
  a later cycle, since A was estimated at about three plans and its ceiling
  alone misses 4 ms on all three cells.
