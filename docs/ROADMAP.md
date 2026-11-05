# Roadmap

Every milestone ends with a fixture run that prints numbers, and each
retires the riskiest remaining unknown first. A milestone's "out" list is
as binding as its "in" list. One section per cycle; a finished cycle
compresses to its status line (`/close-cycle`), and the next is appended
below.

Spine: **C1** M0 → M1 → M2 → M3 → M4 (the vertical slice), then **C2**
(the sketcher gate), then the later cycles one at a time.

---

## Fixtures

The unit of acceptance: a directory `tests/fixtures/<area>/<slug>/` with

- `sketch.json`: the sketch in its serde form (DATA-MODEL.md §The serde
  form), its coordinates being the **guesses** the solve starts from;
- `expected.json`: status, DoF, every free point's expected coordinates
  with the comparison tolerance, the redundant constraints and each
  conflict set **by tag**, and the expected flips (usually none);
- for a drag, `drag.json`: the dragged point, the targets in order, and the
  expected coordinates after chosen steps.

Expected values come from a closed form or from how the sketch was built,
never from laman's output (`.agents/rules/solver.md`). The digest is the
exception: it lives apart, one `<name> <hex>` per line in
`tests/fixtures/c1/digests.txt` (with the conflicting twins' best-effort
violations in `violations.json`), read by `determinism.rs` and by pytest. It
is a regression guard, blessed from the first green run, and
its acceptance is equality across targets. Coordinates compare within
`1e-9` of the sketch's size unless a fixture says otherwise. The test for
a fixture loads it, solves, runs the checker, compares, and compares the
digest; the same fixture runs from pytest in M4.

---

## C1 — the vertical slice

*Goal: one path through every crate before any crate is broad. The SSS
triangle, the belt, the symmetric slotted plate, and one drag across that
plate: each solved, checker green, with exact DoF; the over-constrained
twins naming the right constraints; the digests identical natively and on
wasm32; the same fixtures solving from Python (SEED.md §6).*

**Status: done 2026-11-03.** Retired the diagnostics risk: rank-based DoF,
redundant and conflicting sets come out right, named and deterministic on
sketches whose answers are known by hand. ADRs 0001–0007. Known gaps are
findings in `docs/BACKLOG.md`.

- In: the `model` module and its versioned serde form; `laman-check`;
  `laman-debug` (dumps, digest, fixture loader, strategies, bench);
  `tools/check-layers.sh`; the fuzz targets; `laman::residual` for every
  constraint and the arc's intrinsic equation, held by the
  finite-difference check (`laman_debug::fd`); compile, components,
  Levenberg–Marquardt and DogLeg with caps, warm start, least change; rank,
  geometric DoF (ADR-0002), per-entity DoF, free directions, redundant and
  conflicting sets, degenerate entities; `Sketch::apply`; `DragSession`
  with `Goal` priority, branch observations, flips and bisection;
  `laman-py` over the whole API with stubs; CI on five OS/arch runners and
  wasm32-wasip1; the wheel matrix and the release workflow; the oracle run
  (`tools/oracle/`).
- Out: incremental decomposition and priorities beyond `Required` and
  `Goal` (C2).
- Accept: the fixtures below and their twins, every number as listed, from
  `cargo test` and pytest under both algorithms, checker green; the
  finite-difference property at 1 000 cases per constraint, agreement
  within `1e-6` of the gradient's size; a re-solve of each solution
  returns the same bits; random sketches built by construction solve back
  with the DoF of their construction; the checker passes every fixture's
  expected coordinates and fails each twin's; every fixture's serde form
  round-trips byte for byte; the fuzz targets run 10 minutes without a
  panic; the frame budget below; every digest equal natively, on the five
  CI runners and on wasm32-wasip1; `sketch-solver`, `acs-solver` and
  `ezpz` run out of process on the ten C1 sketches, their status, DoF and
  conflict sets recorded in COMPARISON.md.

### C1 acceptance

Four fixtures and their twins, under `tests/fixtures/c1/`. Ground points are
`point_fixed`; every other coordinate is free and starts at the guess.

**1. SSS triangle** (`sss-triangle`, SEED.md §5's sketch)

| Item | Value |
|---|---|
| Entities | `o` (0, 0) ground, tag `origin`; `a` guess (40, 5); `b` guess (10, 30); lines `base` = o→a, `left` = o→b, `right` = a→b |
| Constraints | `horizontal(base)`; `distance(o, a) = 50` tag `base`; `distance(o, b) = 30` tag `left`; `distance(a, b) = 40` tag `right` |
| Expect | `a` = (50, 0), `b` = (18, 24); status `FullyConstrained`; DoF 0; per-entity DoF all 0; nothing redundant or conflicting |
| Redundant twin | + `perpendicular(left, right)` tag `right-angle` (the 30–40–50 triangle is right-angled at `b`): same coordinates and status; redundant = [`right-angle`] |
| Conflicting twin | + `angle(base, left) = 60°` tag `apex` (the triangle's is atan2(24, 18) = 53.130°): status `Unsatisfiable`; one conflict set = {`base`, `right`, `apex`}, inclusion-minimal (`horizontal` is not in it: every member is invariant under rotation about `o`, `horizontal` is not; neither is `left`: with `base` and `apex` fixing `b` on the 60° line through `o`, `a` lies 50·sin 60° = 43.3 from it, more than the 40 `right` allows, whatever `left` says) |

**2. The belt** (`belt`): two circles and both external tangents.

| Item | Value |
|---|---|
| Entities | `c1` (0, 0) ground; circle `C1` centre `c1`, radius guess 18; `c2` guess (95, 3); circle `C2` centre `c2`, radius guess 12; `t1` guess (0, 22), `t2` guess (100, 12), line `top` = t1→t2; `t3` guess (0, −22), `t4` guess (100, −12), line `bottom` = t3→t4 |
| Constraints | `radius(C1) = 20` tag `r1`; `radius(C2) = 10` tag `r2`; `horizontal(c1, c2)`; `distance(c1, c2) = 100` tag `span`; for `top`: `point_on_circle(t1, C1)`, `point_on_circle(t2, C2)`, `tangent(top, C1)`, `tangent(top, C2)`, tags `top-*`; the same four for `bottom` |
| Expect | with `s` = (20 − 10)/100 = 0.1 and `k` = √(1 − s²) = 0.99498743710662: `c2` = (100, 0); `t1` = (20s, 20k) = (2, 19.8997487421324); `t2` = (100 + 10s, 10k) = (101, 9.9498743710662); `t3`, `t4` their mirrors in `y`; status `FullyConstrained`; DoF 0 (12 free parameters; original Jacobian rank 8 at contact, regularized rank 12, ADR-0002) |
| Redundant twin | + `equal_length(top, bottom)` tag `same-belt` (both are √9900 = 99.498743710662 by symmetry): redundant = [`same-belt`] |
| Conflicting twin | + `distance(t1, t2) = 100` tag `top-length`: status `Unsatisfiable`; conflict set = {`r1`, `r2`, `span`, the four `top-*`, `top-length`} (the tangent length is √(span² − (r1 − r2)²), whatever the rotation, and `bottom`'s parameters enter no member) |

**3. The symmetric slotted plate** (`slotted-plate`)

| Item | Value |
|---|---|
| Plate | `P0` (0, 0) ground; `P1` guess (115, 2), `P2` (118, 63), `P3` (−1, 58); lines `bottom` P0→P1, `right` P1→P2, `top` P2→P3, `left` P3→P0; `horizontal(bottom)`, `vertical(right)`, `horizontal(top)`, `vertical(left)`, `distance(P0, P1) = 120` tag `width`, `distance(P0, P3) = 60` tag `height` |
| Centreline | `Mb` guess (55, 1), `Mt` (62, 59); construction line `axis` Mb→Mt; `midpoint(Mb, bottom)`, `midpoint(Mt, top)` |
| Slot | `K1` guess (38, 31), `S1` (39, 36), `E1` (41, 24): arc `A1` (centre K1, S1 → E1, the left end); `K2` guess (83, 29), `S2` (82, 24), `E2` (79, 36): arc `A2` (centre K2, S2 → E2, the right end); lines `upper` E2→S1, `lower` E1→S2; `tangent` of each line to each arc (4); `equal_radius(A1, A2)` tag `same-ends`; `radius(A1) = 5` tag `slot-r`; `symmetric(K1, K2, axis)` tag `centred`; `distance(K1, K2) = 40` tag `slot-length`; `point_line_distance(K1, bottom, 30)` tag `slot-height` |
| Expect | `P1` (120, 0), `P2` (120, 60), `P3` (0, 60); `Mb` (60, 0), `Mt` (60, 60); `K1` (40, 30), `S1` (40, 35), `E1` (40, 25); `K2` (80, 30), `S2` (80, 25), `E2` (80, 35); each arc's sweep π; status `FullyConstrained`; DoF 0 (22 free parameters: 6 + 4 + 2 intrinsic + 4 + 6 equations; original Jacobian rank 18, regularized rank 22, ADR-0002) |
| Redundant twin | + `horizontal(K1, K2)` tag `level`: redundant = [`level`] (`centred` already holds K1K2 perpendicular to the vertical axis) |
| Conflicting twin | + `radius(A2) = 6` tag `slot-r2`: conflict set = {`same-ends`, `slot-r`, `slot-r2`} |

**4. The drag** (`slotted-plate-drag`): fixture 3 without `width`, plus a
stray free point `Q` (200, 200) that no constraint names.

| Item | Value |
|---|---|
| Before | DoF 3 (the plate's width; `Q`'s two); status `UnderConstrained`; coordinates as fixture 3 |
| Drag | `P2` toward (120 + 3k, 60 + 3k) for k = 1 … 10 |
| After step k | `P1` (120 + 3k, 0), `P2` (120 + 3k, 60) (the target's y is not reachable: `height` and `top` hold it); `Mb`, `Mt` at x = 60 + 1.5k; `K1` at (40 + 1.5k, 30), `K2` at (80 + 1.5k, 30), the slot's other points with them |
| After step 10 | `P1` (150, 0), `P2` (150, 60), `K1` (55, 30), `K2` (95, 30) |
| Always | `P3` stays (0, 60) within tolerance; `Q` keeps its bits; checker green; no flip (each arc's sweep stays π, every tangent on its side); DoF 3 |

**Across the four**: the digest of every fixture and twin, and of each drag
step, is identical on x86_64-linux natively and on wasm32-wasip1; pytest
reproduces every number above and the same digests.

**Frame budget (C1)**: on the reference machine (named, with its CPU, in
the benchmark output), release build: each static fixture solves from its
guesses in ≤ 1 ms (the satisfiable and redundant ones: a conflicting twin
also runs the conflict analysis, which is timed, not budgeted), and each drag
step in ≤ 1 ms.

---

## C2 — the sketcher gate

*Goal: laman can sit under an interactive sketcher (SEED.md §6): edits and
drags at 100 to 500 entities solve inside a frame, keep their branch and
leave unrelated geometry where it was.*

**Status: M5, M6 and M7 done; M8's three plans done, and the frame budget is still missed.** M5: the generated
corpus, construction properties and bounded release benchmark. M6: every
component solved and diagnosed by stages (ADR-0009); an edit keeps every
bit outside its stage's downstream closure (`upstream-edit-drift`
lifted); staged DoF, per-entity DoF, direction span and redundant sets
equal the whole path's, and conflict sets wherever the whole path's are
unsatisfiable. The 4 ms acceptance is **not met** at 500 entities: edit
medians 10.8–22.5 ms, first drag step 20 ms, both algorithms; 98/99
entities 1.1–2.8 ms (ARCHITECTURE.md §Frame budget, with the two measured
cost sources). M7: no radius below 0 in a solve or drag (ADR-0010), conflict
sets equal on the staged and whole paths and the fundamental circuit where
the stretch is small (ADR-0005 amendments), a frame that jumps an arc's
sweep bisected and marked (`Diagnostics::bisection`, ADR-0006). M8's first
plan (staged rank and tolerances once per frame, the extent split): the
500-entity edits are 6–11 ms, the first drag step 12–17 ms, the bisected
frame 29–30 ms, still over 4 ms; the profile in ARCHITECTURE.md §Frame budget
chose a frame floor first (`plans/m8-frame-floor`: sparse staged rank,
satisfaction without a Jacobian, the splitter in O(parts), set-up once a
frame): the 500-entity edits are now 4.4–5.8 ms medians (worst 4.6–6.4),
the first drag step 6.0 (slots) and 15–16 ms (rectangles), the bisected
frame 15 ms, all still over 4 ms. Rigid bodies in the remainder reach only
the drag and bisect cells (the edits have no remainder), by a measured
ceiling of about 5.8, 4.8 and 9.4 ms; they do not close M8 alone. M8's third
plan (`plans/m8-setup`: one cut a frame shared by the solve and the
diagnosis, parts read off the cut, the cut's evaluation once per source, a
descent trial in place) took the edits to 2.9–6.1 ms worst: the triangles
(3.3, 2.9) and the rectangles LM edit (3.9) are inside 4 ms; the rectangles
DogLeg edit (4.2) and the slot edits (6.0, 4.7) are not. The first drag
step is 14.5 ms (rectangles) and 5.0 (slots), the bisected frame 11.6.

- **M7 — drag robustness and conflicts**: done (status line).
- **M8 — the frame budget at 500 entities**: profile the 500-entity edit
  and drag step first, then remove what it confirms (the per-part extent
  measurement, the diagnosis's recompile and re-cut of the placed sketch,
  repeated linear scans), one path, no public type. Accept: every
  500-entity bench cell, edit and first drag step, ≤ 4 ms (the cycle's
  *Accept*). They still miss after three plans: the set-up is done (edits 2.9–6.1 ms
  worst, three of six inside 4 ms; first drag step 5.0 and 14.5 ms, bisected
  frame 11.6 ms). The slot edit is held by the staged rank (21–23 %), which
  is not set-up. Next, from the profile: the coordinates as a dense slice
  (`Frame::terms`, 6–14 %) and a cheaper staged rank; rigid bodies in the
  remainder (option A, ARCHITECTURE.md §Frame budget) reach only the drag
  and bisect cells and cannot close M8 alone.

The risk this cycle retires is locality: that a decomposition into stages
makes a local edit cost a local solve, without changing a diagnostic or a
bit outside the stages it reaches.

- In: subsystem decomposition into stages (Dulmage–Mendelsohn on the
  compiled equations, ADR-0009; no pebble game); incremental
  edits that re-solve only the stages they reach; generated sketches of
  about 100 to 500 entities as fixtures by construction: shared-edge
  rectangle chains (98/248/498), slot grids (98/248/498), and nested
  triangle strips (99/249/501). M5 supplies these families and the bench;
  M6 supplies the decomposition. Drag robustness: a frame that
  moves an arc's tracked sweep more than a named fraction of a turn is
  bisected; a frame solved at a bisected target is marked in its
  `Solution`; a bisected frame is held to the frame budget; the drag
  property's construction gets an observed subject, so it can fail on a
  flip. A radius never ends below zero (findings `negative-radius-solve`,
  `drag-negative-radius`; their fixtures lifted).
- Out: a named consumer's fixtures (none is named yet; they join when one
  is, and integrate on its own schedule); the conflict analysis's cost and
  the inconclusive-conflict findings (`docs/BACKLOG.md`); the Python and
  release-workflow backlog; new constraint kinds.
- Accept: on 500-entity sketches, a drag step and a one-dimension edit each
  re-solve in ≤ 4 ms (a quarter of a 60 Hz frame) on the reference
  machine, bisected drag frames included; geometry outside the edited
  stages' downstream closure keeps its bits; DoF, redundant sets, and
  conflict sets where the whole-sketch solve's are unsatisfiable, equal
  to those of a whole-sketch solve; no flip across the generated drags; no radius
  below zero from any solve or drag the properties draw; conflict sets
  measured against the exact minimum where ADR-0005 says they are
  approximate; the C1 corpus and its digests unchanged, or re-blessed with
  the reason.

## Later cycles, one line each, unordered

- **Profiles**: closed loops and holes from the solved, non-construction
  curves.
- **Edits**: trim, extend, offset and mirror.
- **Ellipses** and their arcs.
- **B-splines**.
- **External references**: projected geometry as ground.
- **3D constraints**: workplanes and 3D points, to be argued first (SEED.md
  §4 non-goals).

Each earns its own cycle with an acceptance corpus; none is scheduled.

## What not to spend agent time on

Rendering or UI; snapping and inference (the sketcher's); solid modelling;
file formats beyond the serde form; speed past the frame budgets;
constraint kinds no consumer reaches (SEED.md §4, §8).
