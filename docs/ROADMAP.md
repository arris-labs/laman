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
  conflict set **by tag**, the expected flips (usually none), and the
  blessed digest;
- for a drag, `drag.json`: the dragged point, the targets in order, and the
  expected coordinates after chosen steps.

Expected values come from a closed form or from how the sketch was built,
never from laman's output (`.agents/rules/solver.md`). The digest is the
exception: it is a regression guard, blessed from the first green run, and
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

The risk this cycle retires is the diagnostics: that rank-based DoF,
redundant and conflicting sets come out right and named, deterministically,
on sketches whose answers are known by hand.

### M0 — the model and the harness

*Goal: everything needed to judge a solve exists before the solver.*
*Status: landed. The senses taken from the drawing moved here from M1: the
builder's signature must not change when the residuals arrive.*

- In: the `model` module (sketch, ids, handles, entities, constraint
  descriptions, tags, payloads, revision, validation and `ModelError`);
  the serde form with versioning; `laman-check` with every constraint of
  DATA-MODEL.md measured; `laman-debug` with the text dump, the SVG dump,
  the digest and the fixture loader; `tools/check-layers.sh` in the gate;
  the four acceptance fixtures below committed with their `expected.json`;
  the fixture tests written and `#[ignore]`d until the solver exists; CI's
  wasm32-wasip1 job running the digest corpus under wasmtime; the fuzz
  crate (one target: load any bytes as a sketch, never panic).
- Out: residuals, the solver, diagnostics.
- Accept: **the checker passes every fixture's expected coordinates** and
  fails each twin's (the fixtures are proven right before anything solves
  them); the serde form of every fixture round-trips byte for byte; the
  fuzz target runs 10 minutes without a panic; the SVG dump of each fixture
  is looked at once (`inspect`).

### M1 — constraints

*Goal: every cycle-1 constraint has residuals and an analytic Jacobian
that agree with finite differences.*
*Status: landed.*

- In: `laman::residual` for every row of DATA-MODEL.md §Constraints and
  the arc's intrinsic equation;
  the finite-difference check (an eighth-order central stencil, step
  `5e-3` of the equation's local scale, agreement within `1e-6` of the
  gradient's size; sound to a line `1e-6` of the sketch's size anywhere in
  it, `laman_debug::fd`) over `proptest` strategies of random
  non-degenerate and near-degenerate configurations.
- Out: the solve.
- Accept: the finite-difference property at 1 000 cases per constraint,
  green; each residual is zero at its fixture's expected coordinates.

### M2 — the solver and its diagnostics (the risk milestone)

*Goal: the three static fixtures and their twins solve from their guesses
with the right status, DoF and named constraints.*

- In: compile, components, Levenberg–Marquardt and DogLeg with caps,
  warm start, least change; rank, geometric DoF for regular configurations
  and C1 contacts (ADR-0002), per-entity DoF, free directions,
  redundant and conflicting sets, degenerate entities; `Sketch::apply`.
  The ADRs closing **tolerances** (absolute or scale-relative) and the
  **conflict set** (exact or bounded).
- Out: drag, priorities beyond `Required`.
- Accept: fixtures 1–3 and their twins below, every number as listed; a
  re-solve of each solution returns the same bits; property: random sketches
  built by construction from a known solution solve back to a solution the
  checker passes, with the DoF of their construction.

*Status: landed. Both algorithms solve the bases and redundant twins within
the budget and name the conflicting twins' sets exactly; the 20 solved
digests agree on native and wasm32-wasip1. Known gaps, all findings in
`docs/BACKLOG.md`: DogLeg ends `NotConverged` on conflicts Levenberg-Marquardt
proves, and the checker overflows past about 1e154.*

### M3 — the drag session

*Goal: one drag across a profile keeps its branch and leaves the rest
alone.*

- In: `DragSession`, `Goal` priority, the branch observations and flips,
  bisection on a flip; the ADR closing **branch preservation**.
- Out: incremental decomposition (C2).
- Accept: fixture 4 below; every step within the cycle-1 frame budget.

*Status: landed. Fixture 4 passes in full through `DragSession` under both
algorithms (checker green, DoF 3, no flips, `Q`'s bits kept); its ten step
digests agree on native and wasm32-wasip1; every step is under 1 ms on the
reference machine (median; thin margin). ADR-0006 closes branch
preservation. Known gaps, all findings in `docs/BACKLOG.md`: a frame can
end a radius just below zero (`apply` refuses it), arc tracking assumes
under half a turn per frame, and bisected frames are not budgeted.*

### M4 — Python and the cycle's close

*Goal: the same corpus from Python, the same bits on every target.*

- In: `laman-py` over the whole API (sketch, solve, drag, check, dumps),
  stubs under `mypy.stubtest` and `mypy --strict`; pytest running every
  fixture; `hypothesis` building random sketches by construction; CI's test
  job on macOS and Windows; the wheel matrix and the release workflow
  (`docs/BACKLOG.md`).
- Out: anything C2.
- Accept: the C1 acceptance below, in full, from `cargo test` and pytest;
  the comparison (COMPARISON.md) re-read against the code;
  `sketch-solver`, `acs-solver` and `ezpz` run out of process
  (`tools/oracle/`) on the four fixtures and their twins, with their
  status, DoF and conflict sets (not coordinates) recorded in
  COMPARISON.md.

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

*Goal: laman can sit under an interactive sketcher (SEED.md §6).*

- In: subsystem decomposition into rigid clusters, with the (2,3) pebble
  game (`laman::rigidity`) as the combinatorial pre-pass, so a local edit
  costs a local solve; incremental edits that re-solve only the clusters
  they reach; generated sketches of 100 to 500 entities (chains of
  profiles, patterns of slots, nested constraints) as fixtures by
  construction; the first consumer's requirements written as laman
  fixtures (the consumer integrates on its own schedule).
- Accept: on 500-entity sketches, a drag step and a one-dimension edit each
  re-solve in ≤ 4 ms (a quarter of a 60 Hz frame) on the reference machine;
  geometry outside the edited clusters keeps its bits; no flip across the
  generated drags; conflict sets measured against the exact minimum where
  the conflict-set ADR says they are approximate.

---

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
