# Roadmap

Every milestone ends with a fixture run that prints numbers, and each
retires the riskiest remaining unknown first. A milestone's "out" list is
as binding as its "in" list. One section per cycle; a finished cycle
compresses to its status line (`/close-cycle`), and the next is appended
below.

Spine: **C1** M0 → M1 → M2 → M3 → M4 (the vertical slice), then **C2**
(the sketcher gate), **C3** (profiles), then the later cycles one at a
time.

---

## Fixtures

The unit of acceptance: a directory `tests/fixtures/<area>/<slug>/` with

- `sketch.json`: the sketch in its serde form (DATA-MODEL.md §The serde
  form), its coordinates being the **guesses** the solve starts from;
- `expected.json`: status, DoF, every free point's expected coordinates
  with the comparison tolerance, the redundant constraints and each
  conflict set **by tag**, and the expected flips (usually none); and, for
  a C3 fixture, `profiles`: every list of the extraction by id, each loop's
  edges in order with `reversed` and its joins, and each area (its
  tolerance where it is not the default, relative `1e-9`);
- for a drag, `drag.json`: the dragged point, the targets in order, and the
  expected coordinates after chosen steps.

Expected values come from a closed form or from how the sketch was built,
never from laman's output (`.agents/rules/solver.md`). The digest is the
exception: it lives apart, one `<name> <hex>` per line in
`tests/fixtures/c1/digests.txt` (with the conflicting twins' best-effort
violations in `violations.json`) and `tests/fixtures/c3/digests.txt`, read
by `determinism.rs` and by pytest. It
is a regression guard, blessed from the first green run, and
its acceptance is equality across targets. Coordinates compare within
`1e-9` of the sketch's size unless a fixture says otherwise. The test for
a fixture loads it, solves, runs the checker, compares, and compares the
digest; the C1 and C3 fixtures run from pytest too.

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

**Status: done 2026-11-07.** Retired the locality risk: a local edit costs a
local solve, without changing a diagnostic or a bit outside the stages it
reaches. ADRs 0009 (stages), 0010 (a radius is bounded below by zero) and
0011 (the budget per try, and the stop at the rectangle drag); ADR-0005 and
ADR-0006 amended. The frame budget's numbers are in ARCHITECTURE.md §Frame
budget.

- In: subsystem decomposition into stages (Dulmage–Mendelsohn on the
  compiled equations, ADR-0009; no pebble game); incremental edits that
  re-solve only the stages they reach; generated sketches of about 100 to
  500 entities as fixtures by construction: shared-edge rectangle chains
  (98/248/498), slot grids (98/248/498), and nested triangle strips
  (99/249/501). Drag robustness: a frame that moves an arc's tracked sweep
  more than a named fraction of a turn is bisected; a frame solved at a
  bisected target is marked in its `Solution`; a bisected frame is held to
  the frame budget; the drag property's construction gets an observed
  subject, so it can fail on a flip. A radius never ends below zero.
- Out: a named consumer's fixtures (none is named yet; they join when one
  is, and integrate on its own schedule); the conflict analysis's cost and
  the inconclusive-conflict findings (`docs/BACKLOG.md`); the Python and
  release-workflow backlog; new constraint kinds; rigid bodies in the
  remainder (rejected, ADR-0011).
- Accept: on 500-entity sketches, a drag step and a one-dimension edit each
  re-solve in ≤ 4 ms (a quarter of a 60 Hz frame) on the reference
  machine (a single sample of three runs may touch 4.4 ms, accepted as
  noise), the rectangle drag excepted at its measured 6.0 ms, and every
  try of a bisected drag frame ≤ 4 ms (a frame's time divided by its tries,
  ADR-0011); geometry outside the edited stages' downstream closure keeps
  its bits; DoF, redundant sets, and conflict sets where the whole-sketch
  solve's are unsatisfiable, equal to those of a whole-sketch solve; no
  flip across the generated drags; no radius below zero from any solve or
  drag the properties draw; conflict sets measured against the exact
  minimum where ADR-0005 says they are approximate; the C1 corpus and its
  digests unchanged, or re-blessed with the reason.

## C3 — profiles

*Goal: a sketcher can ask a solved sketch for its closed loops and holes —
ordered, oriented and nested — and extrude or fill them, from the same
solved coordinates and without a new solve.*

**Status: done 2026-11-08.** Retired the topology risk: loops, orientation
and nesting come out right and the same bits on every target, and the
degenerate cases (an open chain, a dangling edge, a loop touching itself)
are reported with the caller's ids, never errors or panics. ADRs 0012
(joins are declared), 0013 (the check measures by sectors) and 0014 (the
extraction budget). The numbers are in ARCHITECTURE.md §Frame budget.

- In: profile extraction from the solved, non-construction lines, arcs and
  circles: edges joined at shared points, loops as ordered, directed edge
  lists, outer boundaries counter-clockwise and holes clockwise, nesting by
  containment, signed area; open chains, dangling edges and self-touching
  loops named in the result with the caller's ids; the checker's own
  independent measure of area, orientation and containment (`laman-check`,
  model types only, ADR-0013); `laman-py` over the new API with stubs; the
  digest extended over profiles. Joins are declared, by a shared point id or
  a `coincident` chain, never measured (ADR-0012).
- Out: trim, extend, split at intersections and any change to the sketch
  (the Edits cycle); curves that cross without a shared point; ellipses and
  splines; filling, meshing, booleans; a named consumer's fixtures.
- Accept: fixtures by construction, every number from a closed form: the
  rectangle (area 120 × 60 = 7200, outer counter-clockwise); the C1 slotted
  plate (one outer, one hole, net area 7200 − (40 × 10 + 25π) = 6721.46);
  a plate with an island inside a hole; two disjoint loops; a circle alone;
  an open chain and a dangling edge, each reported by id; the generated
  slot grids give their known loop and hole counts; the checker agrees on
  every fixture and fails each broken twin; every digest equal natively
  and on wasm32-wasip1; pytest reproduces every number; extraction at 500
  entities inside 1.5 ms (the stop rule of ADR-0011, ADR-0014).

## C4 — shared edges and crossings

*Goal: a sketch drawn the way people draw it, with two rectangles sharing
a wall or a window split into panes, gives one profile per enclosed area.
Where curves cross without a shared point, laman says where they cross and
how. Both come from the solved coordinates, with no edit and no new solve.*

The risk this cycle retires is read-only topology on solved geometry. A
branched core has to split into its profiles, with edges at a join ordered
by direction and then by curvature where two edges leave tangent. A crossing
has to be found, classified and ordered the same way on every target. A
tangent touch is a double root, so it is only as accurate as the solve that
produced it, and it must be told apart from a crossing. Every later edit
(split, trim, extend) is built on these three. Retiring them before any edit
changes the sketch keeps a geometry bug from being mistaken for an edit bug.

- In: profiles of a branched core, joins still declared (ADR-0012 stands).
  At each join the edges are ordered by tangent direction, then by
  curvature; every bounded profile is reported, and so is an edge that
  bounds a profile on both sides. Intersections of lines, arcs and circles
  that meet without a shared point: each pair named by its entities, the
  points ordered along each edge, each one classified as a crossing, a
  touch or an overlap (collinear lines, or concentric equal circles or
  arcs), and an arc's crossings bounded by its sweep. A tangent double root
  solved to the coordinate tolerance (regression/`coincident-contact-drift`
  lifted), because a touch cannot be classified at √residual. The checker's
  independent measure of each intersection and of the branched profiles.
  `laman-py` over the new API, with stubs. The digest extended over
  intersections.
- Out: any change to the sketch (C5). Profiles that use a crossing nobody
  declared: a crossing joins nothing until C5's split declares it
  (ADR-0012). Ellipses and splines. Filling, meshing, booleans. A named
  consumer's fixtures.
- Accept: fixtures by construction, every number from a closed form. Two
  rectangles sharing an edge: two profiles, 60 × 60 each. A window of four
  panes: a degree-4 join, four profiles. A slot whose arc leaves a join
  tangent to a line: ordered by curvature. A figure eight touching itself
  at one declared join: two profiles. A slot grid with shared walls: its
  known profile count. Intersections: two crossing lines; a line through a
  circle; two circles meeting at two points; a tangent touch, reported as a
  touch; a collinear overlap; and an arc whose circle crosses a line outside
  its sweep, which is no intersection. The same intersections at magnitudes
  near 1e±300. The coincident-joined slotted plate at the shared-point
  plate's accuracy. The checker agrees on every fixture and fails each
  broken twin. Every digest is equal natively and on wasm32-wasip1. Pytest
  reproduces every number. Extraction and intersections at 500 entities
  stay inside ADR-0014's stop rule.

*Proposed at the close of C3 and revised before planning: edits move to
C5, which needs these intersections and branched profiles; mirror and
offset wait for a consumer. Theme and in/out split are the human's call.*

## Later cycles, one line each, unordered

- **Edits** (C5 when C4 closes): split at a C4 intersection, which
  declares the join; trim and extend to one; re-point a shared point. An
  ADR decides what each constraint on a split, trimmed or removed piece
  becomes (moves, splits, drops, or is refused by id), and the result
  matches a hand-built sketch in DoF, profiles and bits.
- **Mirror and offset**, once a consumer asks. Mirror is copies plus
  `symmetric` and needs no new kind. A live offset needs `concentric` and a
  curve-to-curve distance, which are new kinds (DATA-MODEL.md §Not yet).
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
