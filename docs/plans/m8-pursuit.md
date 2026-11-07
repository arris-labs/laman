# Plan: m8-pursuit

- Started: 2026-11-07
- Milestone: M8 (cycle C2, docs/ROADMAP.md)
- Idea: docs/ideas/c2-accept-drag-cells.md (absorbed; option B, Q1–Q5 settled)

## Goal

The rectangle drag's pursuit costs a fraction of its 10 ms: fewer iterations,
factorizations that skip exact zeros, and multipliers read from the
factorization instead of an SVD. The rectangle drag at 498 entities meets 4 ms
worst, or C2's *Accept* carries its measured number and C2 closes on it (the
stop rule: **this is the last plan on this cell**). The bisect clause reads
≤ 4 ms per try, the bench holds it at a frame of many halvings, and ADR-0011
gives the reasons. Every edit and slot drag stays inside 4 ms.

## Non-goals

- Rigid bodies in the remainder (rejected, `docs/BACKLOG.md`).
- A sparse factorization or storage (SEED.md §7: a later, argued change). Z
  below keeps the dense storage and the bits.
- Lowering `max_bisections`, or tries sharing their work (ADR-0006 stands; the
  restated clause needs neither).
- Static solves (reported, not budgeted) and any public type or signature.
- A second floor plan: step 5 takes what one profile confirms, then stops.

## Design deltas

Measured for the idea (274df0f, a throwaway copy, LM, 498 rectangles,
in-process medians about 1.5 ms under the bench): frame 13.4 ms; pursuit 10.2,
of it 5 iterations of SVD 6.4 and dense QR 3.2, the probes' descents 0.2;
outside it 3.2. Remainder: 100 parameters, 99 rows, null space 1. Later steps
and pointer-rate steps cost the same as the first.

**D1 — Iterations (`pursue`, `solve.rs`).** `μ` is the damping times
`max(1, max diag H)`. On a long chain `H` is 0.05, so the floor of 1 holds the
first step short: 5 iterations where a linear problem wants 2 or 3. Removing
the floor gave 3 iterations and failed twice: it broke
`unreachable_goal_converges_stationary_at_the_nearest_point`, and took the
bisect cell from 2 halvings to 4 (7 to 10.7 ms). Step 1 finds why and a rule
that keeps the gain without either. Candidates: a first step undamped when
the model is trusted; the floor kept only where curvature is a large part of
`H`. The rule and any constant: measured by step 1.

*Settled by step 1:* `μ` is the damping times `max(max diag G, max |diag H|)`,
`G = (J_g N)ᵀ(J_g N)`; no constant. The test failed because `H` is exactly 0
there (`G` 1 and `W` −1 cancel at the circle's far side), so a damping read
against `H` alone is 0 and raises do nothing. `G` is positive past the
stationary test, and `|H|` keeps the raises in reach of a far target's large
negative curvature. Measured: `|H|` alone gave the bisect 4 halvings, `G` alone
2, and the test now pins 2; why `|H|` alone gave 4 was not traced.

**D2 — Zero-skipping QR (`Qr::new`, `diagnose.rs`).** The reflector touches only
the rows where `v` is nonzero and skips a column whose dot is exactly 0; a
column's norm is recomputed only if the reflector changed it or the row just
retired was nonzero there. Skipped terms are exact zeros, so the pivots, R and
every bit stay (ADR-0003): the prototype passed 490 tests with digests equal,
null space 3.2 → 1.65 ms. Not a sparse method.

**D3 — Multipliers from the factorization (`Pursuit::curvature`).** `Qr` keeps
its reflectors. With `J P = Q R`, take `b' = b − N Nᵀ b` (the part of `J_gᵀ g`
in the range of `Jᵀ`, `N` the basis in hand), solve `R₁₁ᵀ y = (Pᵀ b')₁..r`, and
`λ = −Q [y; 0]`: the same minimum-norm `λ` as the SVD's, since it lies in the
range of `J`. Two traps: `component_qr` factors **unit** rows and drops zero
ones, so `λ` is rescaled by each row's norm and mapped back to its equation;
and the prototype (an unpivoted QR of `Jᵀ`, full column rank only, 7.3 ms on
the bench with D2) proved the speed, not this. `gauss_newton` stays for the
goal step of a component without required equations and everywhere else it is
used.

**D4 — Bench (`bench_c2.rs`).** The verdict moves to the worst sample, as the
*Accept* is stated (the backlog's line). A bisected frame also prints its tries
and its time per try. A new `bisect-deep` cell: a frame refused at every
halving, so `max_bisections` + 1 tries, by a target past a flip on the arc let
loose in `bisecting_drag`. That function moves to `laman_debug::corpus` so a
test can hold the cell's halvings.

*Settled by step 4:* no target makes the loosened slot flip (it turns any quarter
or half turn with room to spare; a scan of turns from 60 to 300 degrees and of
rays gave only sweep jumps). `bisect-deep` is a target a few thousand radii away
(`corpus::refused_drag`): even the step halved 8 times moves the sweep past
`MAX_SWEEP_STEP`, so every try is refused by a jump and the session stays put.
Measured at 498 entities, worst of 5: bisect 7.3 ms over 3 tries (2.4 a try),
bisect-deep 34.8 ms over 9 (3.9 a try), both algorithms alike.

**D5 — The *Accept* (ROADMAP.md §C2).** "bisected drag frames included" becomes
every try of a bisected frame ≤ 4 ms, a frame's time divided by its tries. The
rectangle drag keeps 4 ms unless step 6 applies the stop rule. ADR-0011:
the budget per try (a frame's cost is its tries, up to 9, so no per-frame number
holds; the frames after one take 2.4 ms; it weakens what M7 wrote, on
purpose), the stop rule, and why rigid bodies lost.

No public type or signature changes. `GLOSSARY.md`'s **Try** and **Pursuit** are
in. Digests: D2 cannot move them; D1 and D3 may, and a move is re-blessed under
`fixtures:` with the reason (C2's *Accept* allows it).

## Steps

Complexity grades; a step's grade is its tier, the model that executes it
(`.agents/tiers.md`): **[1]** routine — the design says exactly what to write
and the tests are mechanical; **[2]** careful — a numeric or geometric case to
get right within a given design; **[3]** unproven — an algorithm whose
robustness, convergence or bound has to be established here.

- [x] Step 1 **[3]** — D1: move `bisecting_drag` to the corpus and pin the
  cell at 2 halvings under both algorithms; explain the test failure and the
  extra halvings of the floorless damping; land a rule that takes the 498
  rectangle drag to at most 3 pursuit iterations with neither regression. If
  no rule holds, keep the floor, commit the pin, and record what was tried here.
- [x] Step 2 **[2]** — D2: zero-skipping `Qr::new`. A test keeps the dense
  version as a reference and compares R, `perm`, rank and basis **by bits** on
  random matrices with structural zeros, rank-deficient ones included.
- [x] Step 3 **[2]** — D3: `λ` from the kept factorization. Tests against
  `gauss_newton` within a named tolerance: a curved linkage, a chain, rows of
  mixed norm, a zero row, and redundant rows (a whole-path component).
- [x] Step 4 **[2]** — D4: the verdict on the worst sample, tries and time per
  try printed, the `bisect-deep` cell, failing if its frame is not refused at
  every halving.
- [ ] Step 5 **[2]** — Re-profile the rectangle drag by instrumentation in a
  scratch copy, record the split here, and remove what it confirms outside the
  pursuit. First suspect: `freedom_on` factorizing again after the pursuit
  (1 ms of `conclude`). One pass, no second profile.
- [ ] Step 6 **[2]** — Three full `bench_c2` runs. Record each 500-entity cell's
  worst across runs here, and apply the stop rule to the rectangle drag.
- [ ] Step 7 **[1]** — D5: ADR-0011 and ROADMAP.md's *Accept* and M8 lines, with
  step 6's numbers.

## Acceptance

- `cargo run --release -p laman-debug --example bench_c2`, three runs,
  worst across them, both algorithms:
  - the six 500-entity edits and the slot drag ≤ 4 ms;
  - the rectangle drag ≤ 4 ms, or under 7.5 ms with its number in the *Accept*
    (7.5 is what D2 and the prototype of D3 reached; above it the plan failed);
  - the slot bisect and `bisect-deep` ≤ 4 ms per try.
- The bisect cell stays at 2 halvings under both algorithms.
- `tools/gate.sh` and `tools/py-test.sh` green; every test in `tests/c2.rs`,
  `c2_props.rs` and the drag properties green at `PROPTEST_CASES=2000`; C1
  digests unchanged or re-blessed with the reason, on every target (CI).
- C1's 1 ms budgets hold.

## Docs to update on completion

- `docs/ARCHITECTURE.md` §Frame budget — the new table and the pursuit's split;
  drop *A's reach*. §Priorities or §Drag sessions — the damping rule if D1 landed.
  §Diagnose — the QR's skipped zeros and kept reflectors.
- `docs/ROADMAP.md` §C2 — status line.
- `AGENTS.md` current state — M8 done; C2 ready for `/close-cycle` if every cell
  is settled.
- `CHANGELOG.md` `Unreleased` — long chains drag about twice as fast or better.
- `docs/BACKLOG.md` — drop the worst-sample line (D4) and the `freedom_on` part of
  the frame-budget line if step 5 took it.

## Open questions

- ⚠ OPEN: the form of a stopped cell's number in the *Accept*. Agent, by step 6;
  default: the worst across the three runs, rounded up to the next 0.5 ms.
