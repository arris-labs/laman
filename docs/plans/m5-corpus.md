# Plan: m5-corpus

- Started: 2026-11-04
- Milestone: M5 (cycle C2, docs/ROADMAP.md §C2)
- Idea: docs/ideas/c2-decomposition.md (its M5 part absorbed; Q1 C, Q2 S1, Q3 the split, settled)

Under ~150 lines and ~10 steps; larger work is milestone plans.

## Goal

C2's acceptance has sketches to run on and a bench to judge them, before
any solver change. `laman_debug::corpus` builds three families of sketches
by construction at any size, 100 to 500 entities included, each with its
closed-form expectations, a one-dimension edit and (two families) a drag.
`tests/c2.rs` holds every number the corpus can state today; the bench
`bench_c2` measures today's solver on it (static solve, edit re-solve, drag
step, both algorithms) against the 4 ms budget, and those baseline numbers
are in ARCHITECTURE.md. The C1 corpus and its 50 digests do not move.

## Non-goals

- The decomposition: stages and their diagnostics (M6, ADR-0009).
- Drag robustness, radius ≥ 0, conflict sets against the exact minimum (M7).
- Rigid bodies in the remainder or a solver cache (M8, only if M6 misses).
- A public edit API (`Sketch::set_dimension`): an edit is
  `remove_constraint` + `add_constraint_with_id` with the same id, value
  changed; whether a consumer needs more is a backlog line.
- Committed digests or JSON of the generated sketches: they would move in
  M6 anyway; M6 blesses them.
- Python: the corpus is Rust-only this cycle.

## Design deltas

- **`laman_debug::corpus`** (new, laman-debug only, no `laman` change): a
  family is a constructor from its dimensions to `Case { sketch, expected:
  Expected, edit: Edit, drag: Option<DragCase> }`, reusing `fixture::Expected`
  and `fixture::Drag`. `DragCase` holds a separate sketch with the
  dimension removed, its initial expectation (DoF 1), and the `Drag`;
  this is necessary because slot edits and drags remove different dimensions.
  `CorpusError` names refused counts/dimensions or wraps a model error.
  Each family has a default constructor and a `_with` variant taking
  positive finite dimensions for the property. The constructor computes the coordinates from the
  dimensions in closed form, so an edited or dragged expectation is the
  same constructor at the new dimension: never laman's output
  (`.agents/rules/solver.md` §Testing). Guesses are the constructed
  coordinates displaced by a deterministic offset (a closed formula in the
  entity's index, under a tenth of the smallest dimension), so every guess
  sits on the construction's branch. No randomness.
- **`Edit`**: the constraint's tag, its new value, the expected coordinates
  after, and `unmoved`: the entities upstream of the edit in the
  construction, whose exact **bits** are the M6 regression guarantee across the re-solve (the
  roadmap's "geometry outside the edited clusters"). The re-solve starts
  from the solved coordinates (the sketcher's case).
- **The three families** (ROADMAP.md §C2's "chains of profiles, patterns of
  slots, nested constraints"):
  - `rect_chain(n)`: `n` rectangles in a row, each sharing its left edge
    with the last one's right; ground at the first bottom-left corner;
    `horizontal`/`vertical` sides, one `distance` per width, one height.
    5 entities per rectangle plus the first edge (3 entities). Edit: width `j` (rectangles after `j` shift).
    Drag: width `j` dropped (DoF 1), the top-right of `j` pulled right.
  - `slot_grid(rows, cols)`: a grounded plate holding `rows × cols` copies
    of C1's slot, each placed by two `point_line_distance` from the plate's
    bottom and left edges. 10 entities per slot plus the plate (8 entities).
    `slot_grid_with(rows, cols, length, radius)` varies centre separation and
    radius; pitches are `2 length + 4 radius` and `6 radius`. Edit: one slot's height.
    Drag: one slot's horizontal placement dropped (DoF 1), its centre pulled.
  - `triangle_strip(n)`: the nested family, a deep dependency chain:
    points `p₀` (ground), `p₁` on a horizontal from it, each next point by
    two `distance`s to its two predecessors, lines along every edge (3n + 3 entities for n triangles).
    `triangle_strip_with(n, side)` varies the equilateral side length. Edit:
    one middle skip-edge length; the tail rotates rigidly about its preceding
    point, computed from circle intersection and one rotation by construction. No
    drag (a dropped distance makes a mechanism whose closed form the plan
    does not need).
- A term needs no glossary entry: *fixture*, *by construction*, *edit*
  already mean this.

## Steps

Complexity grades; a step's grade is its tier (`.agents/tiers.md`):
**[1]** routine — the design says exactly what to write; **[2]** careful —
a numeric or geometric case to get right within a given design; **[3]**
unproven — an algorithm whose robustness or bound is established here.

- [x] Step 1 **[2]** — `corpus` module, `Case`/`Edit`, and `rect_chain` with
  its guesses, edit and drag; a test at `n = 3` solves from the guesses
  under both algorithms, checker green, coordinates and DoF as built.
  Dumped to SVG and looked at (`inspect`).
- [x] Step 2 **[2]** — `slot_grid` the same way, tested at 2 × 2: each
  arc's sweep π, no flip, the C1 slot's coordinates per cell.
- [x] Step 3 **[2]** — `triangle_strip` the same way, tested at `n = 6`,
  including the edit's rigid tail by construction.
- [x] Step 4 **[2]** — `tests/c2.rs` runs each family's case, edit and drag
  through one helper: expected coordinates, status, DoF, empty redundant
  and conflicts, no flip, checker green, `unmoved` geometrically unchanged, every drag
  step's coordinates; small sizes in the gate.
- [ ] Step 5 **[2]** — the same tests at 100, 250 and 500 entities per family;
  each size whose debug-build run exceeds 10 s (measured here) is
  `#[ignore = "C2 M6: whole-component solve"]` with its time in the
  reason, lifted by M6. Release runs of the ignored ones recorded in the
  commit body (pass or the first failing number).
- [ ] Step 6 **[2]** — property: each family at random small sizes and
  random dimensions within its construction's ranges (`proptest`) solves
  back to its construction, checker green, the edit's `unmoved` geometrically unchanged.
- [ ] Step 7 **[1]** — example `bench_c2`: per family and size (100, 250,
  500), per algorithm, the median and worst of the static solve, the edit
  re-solve and a drag step; prints over/under the 4 ms budget and the
  machine; exits zero (the budget is M6's to meet). A case slower than a
  set limit on its first run is printed `over` without repeats.
- [ ] Step 8 **[1]** — run `bench_c2` in release on the reference machine;
  the table into ARCHITECTURE.md §Frame budget as C2's baseline.

## Acceptance

- `cargo test -p laman-debug --test c2`: every non-ignored case green under
  both algorithms, checker clean, DoF exact (0 static and edited, 1 dragged),
  redundant and conflicts empty, no flip, `unmoved` within the unchanged coordinate tolerance; every
  ignored case named with its measured time, and run once in release with
  the outcome in step 5's commit.
- The corpus property at its default case count, green.
- `cargo run --release -p laman-debug --example bench_c2` prints the full
  table (3 families × 3 sizes × 3 operations × 2 algorithms) and the
  machine; the numbers are in ARCHITECTURE.md.
- The C1 corpus, its 50 digests and `bench` unchanged (no `laman` change).

## Docs to update on completion

- `docs/ARCHITECTURE.md` §Frame budget — the C2 baseline table (step 8);
  §Determinism or the harness list — `laman_debug::corpus` named.
- `docs/ROADMAP.md` §C2 — M5 done; the families and sizes named in *In*.
- `crates/laman-debug/src/lib.rs` — `corpus` in the module list.
- `docs/ideas/c2-decomposition.md` — the M5 milestone line marks it done.
- `AGENTS.md` current state — M5 done; next `/plan` M6.

## Open questions

- **Settled (human, 2026-11-04):** strict upstream bit preservation is
  deferred to M6, recorded in `BACKLOG.md` and the unchanged ignored
  `regression/upstream-edit-drift` tests for both algorithms. The human
  authorized "yes, mark in backlog and continue". M5 retains its original
  coordinate tolerance, checker, status, DoF, diagnostics and drag assertions;
  its ordinary tests and property check upstream geometry within that
  tolerance. This explicitly changes M5 acceptance, without accepting the
  drift as correct behavior.

- ⚠ OPEN: the debug-build limit for ignoring a large case (10 s here) and
  the bench's first-run limit — the agent, at steps 5 and 7, recording the
  measured times.
- ⚠ OPEN: whether "nested constraints" means more than a deep dependency
  chain (dimensions referencing dimensions, profiles inside profiles) — the
  agent reads the triangle strip as the deep chain, the slot grid as
  profiles inside a profile; the human may name a consumer shape that
  replaces either.
