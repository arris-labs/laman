# Architecture

How laman is put together: the crates and the rule between them, the
sketch and its ids, what `solve` takes and returns, the drag session, where
the checker sits, and the rules that keep every target's bits the same.
Entity parametrisations and constraint equations are in
[DATA-MODEL.md](DATA-MODEL.md); the words are in
[GLOSSARY.md](../GLOSSARY.md).

## Crates and the layer rule

| Crate | Holds | Published | Depends on |
|---|---|---|---|
| `laman` | the sketch, entities, constraints and their residuals, the solver, diagnostics, drag sessions, `rigidity` | crates.io `laman` | `nalgebra`, `libm`, `thiserror`; `serde` behind the `serde` feature |
| `laman-check` | the checker: every constraint re-evaluated from solved coordinates | no (`publish = false`) until the solver's first release (`docs/BACKLOG.md`) | `laman`'s **model** only |
| `laman-debug` | text and SVG dumps, the fixture loader, `proptest` strategies, the generated corpus (`corpus`), the digest, benchmarks | no (`publish = false`) | `laman`, `laman-check`; `proptest` behind a default feature of the same name |
| `laman-py` | the PyO3 binding, `laman`, `laman.check` and `laman.debug` in Python | PyPI `laman`, never crates.io | `laman`, `laman-check`, `laman-debug` without `proptest` |

`laman-debug` is a dev-dependency, except under `laman-py`, without its
`proptest` feature (ADR-0007; `tools/check-layers.sh` refuses `proptest` in
`cargo tree -p laman-py`). `fuzz/` sits outside the
workspace and builds on nightly under `cargo fuzz`. So does `tools/oracle/`
(`publish = false`, its own `Cargo.lock`): it runs other Rust solvers out of
process on the C1 fixtures for COMPARISON.md and depends on `laman`'s model
and `laman-check` only, never on the solver (`tools/check-layers.sh` greps
`tools/oracle/src`); it is run by hand, not by the gate.

**The layer rule**: `laman-check` names only items under `laman::model`
(the sketch, its ids, entities, constraint descriptions, `Coordinates`) and
never `laman::solve`, `laman::residual` or `laman::diagnose`. Its whole
value is that it shares no code with the residuals it checks: a sign error
in a residual and the same sign error in the check would agree.
`tools/check-layers.sh`, run by the gate, greps `crates/laman-check/src`
for any other `laman::` path. Splitting `laman` into several crates waits
for a layer rule that needs it (SEED.md §7).

Inside `laman`, one module per concern, each depending only on those above
it in this list:

| Module | Holds |
|---|---|
| `model` | `Sketch`, ids, `Entity`, `Constraint` (descriptions, no maths), `Coordinates`, tags and payloads, the serde form |
| `values` | the `Values` trait a parameter's value is read through, over `Coordinates` and over `Dense`, the flat table a solve and the analysis hold (§Diagnose); it names only `residual`'s `Param` key |
| `residual` | `equations(sketch, values, constraint)` and `intrinsic(sketch, values, entity)`: each an `Equation` (value, `Unit`, a degenerate flag, and its analytic gradient as terms keyed by `(EntityId, Coordinate)`, merged and sorted, at most 8) at any coordinates; total, no `Result` (DATA-MODEL.md §Constraints) |
| `branch` | observations at coordinates (arc sweeps, circle radii, line-pair orientations) and the flips between two sets (§Branches) |
| `system` | a sketch compiled for one solve: the dense vector of free parameters, the equation list in order, the components |
| `stage` | each component cut into parts by the Dulmage–Mendelsohn decomposition of its equations: the head, the stages in dependency order, the remainder ([ADR-0009](adr/0009-stages-and-part-rank.md)) |
| `solve` | Levenberg–Marquardt and DogLeg over a `System`, warm start, priorities |
| `diagnose` | rank, DoF per entity, free directions, redundant and conflicting sets |
| `solution` | `Sketch::solve` and `apply`, `SolverOptions`, `Solution` and its `Diagnostics` (`Flip`, `Bisection` and their kin), the solution's serde form |
| `drag` | `DragSession` |
| `profile` | `profiles(sketch, values)`: loops, holes and open ends from coordinates (§Profiles); its result types live in `model::profile` |
| `rigidity` | the generic rigidity count (the 0.0.1 API); `stage` does not use it |
| `reference` | behind the non-default `reference` feature, hidden and unbound: the whole path's solve and diagnosis and `downstream`, for laman's tests (§Solve) |

## The sketch and its ids

A `Sketch` owns every parameter, entity and constraint. Storage is three
`BTreeMap`s keyed by id (parameters, entities, constraints), so iteration
is in id order everywhere and an id is stable for the sketch's life: a
removed id is never reused by the sketch itself.

- **Ids** are `u32` newtypes: `EntityId`, `ConstraintId`. The sketch mints
  the next one (one past the largest in use); a caller that persists its
  own ids passes them instead (`Sketch::point_with_id`, and so on), and a
  clash is `ModelError::DuplicateId`. Ids are the caller's handles in
  every diagnostic.
- **Typed handles** `Point`, `Line`, `Circle`, `Arc` wrap an `EntityId` and
  are `Copy`. The Rust API takes them, so a line built from a circle does
  not compile. The serde form and the Python binding carry bare ids, and
  the kind is checked when the entity or constraint is added
  (`ModelError::WrongKind`).
- **Parameters** belong to their entity (a point's `x` and `y`, a circle's
  radius) and are addressed as `(EntityId, Coordinate)`; there are no free
  floating parameters. A parameter is **free** or **fixed**; a fixed one is
  ground, never an unknown (`Sketch::point_fixed`, `Sketch::set_fixed`).
  Fixing a point through the `fix` constraint is different: that is an
  equation, which diagnostics can blame (DATA-MODEL.md §Constraints).
- **Tags and payloads**: `Sketch::tag(id, &str)` and `Sketch::payload(id,
  Payload)` attach a caller string and an opaque caller value (bytes, so
  the serde form round-trips it) to an entity or constraint. Both come
  back in every diagnostic that names the id. SEED.md §5 sketches a
  chained `.tag()` on the value a builder returns; a handle that borrows
  the sketch cannot be used while the next builder call borrows it again,
  so tags are set through the sketch.
- **Construction**: an entity may be flagged construction. It solves like
  any other; it is excluded from profiles (§Profiles) and drawn
  dashed.

**A sketch is well-formed by construction.** Every mutation validates and
returns `Result<_, ModelError>`: an unknown or removed id, the wrong kind
of entity, a non-finite coordinate or value, a dimension out of range (a
distance or radius `≤ 0`, an angle outside `(0, π)`), a duplicate id, an
entity referenced twice where the constraint needs two (a line from a point
to itself). Removing an entity that constraints use is refused
(`ModelError::InUse` naming them) unless the caller asks for the cascade
(`Sketch::remove_with_constraints`, which returns what it removed).
Loading the serde form runs the same validation. So no malformed sketch
reaches `solve`, and `solve` has no error type.

Every mutation increments `Sketch::revision`, a `u64`.

## Solve

```rust
impl Sketch {
    pub fn solve(&self, options: &SolverOptions) -> Solution;
    pub fn apply(&mut self, solution: &Solution) -> Result<(), ModelError>;
}
```

`solve` reads the sketch and never changes it. Its starting point is the
sketch's current coordinates: the drawing, or the last applied solution,
which is the **warm start**. `apply` writes a solution's coordinates back
and refuses, before changing anything, a solution not solved from this
sketch as it is now: another revision is `ModelError::Stale`, the same
revision over other contents `ForeignSolution`. A solution carries its
source's revision and a digest of what a solve reads, so equal revision
counters alone never admit one. Apply also refuses missing, unknown,
non-finite and negative-radius coordinates and a moved fixed parameter, and
bumps the revision.

A `Solution` always comes back (SEED.md §5, §9): the coordinates of every
entity (best effort when the sketch cannot be satisfied), a `Status`, and
`Diagnostics`. Status and diagnostics are defined in DATA-MODEL.md
§Diagnostics. "Require fully constrained" is
`solution.status == Status::FullyConstrained`, a check on the result.

The pipeline, each stage deterministic:

1. **Compile** the sketch into a `System`: the free parameters in (entity
   id, coordinate) order as a dense vector; the equations in (constraint
   id, equation index) order, intrinsic equations under their entity's id
   first; each equation's priority. The equations and their gradient terms
   come from `residual`; the columns of fixed parameters are dropped.
2. **Decompose** into components: free parameters connected through shared
   equations, found by union–find and numbered by their smallest parameter.
   Each component is then cut into **parts** (`stage`, ADR-0009): a maximum
   matching of its equations to its free parameters over the gradient's
   term keys, the coarse Dulmage–Mendelsohn split into an over-determined
   **head**, a square middle and an under-determined **remainder**, and
   Tarjan's strongly connected components of the middle, the **stages**,
   in dependency order. A constraint's equations, and an arc with its
   intrinsic equation, never split. A degenerate equation, which has no
   gradient where it stands, is read at a fixed generic position, and
   failing that as everything it could read: the cut is then coarser,
   never wrong. A frame cuts each component once: the solve hands its cut
   to the diagnosis, which reads it where its parameters and sources match
   and cuts again only where they do not.
3. **Solve** each component: damped least squares with analytic Jacobians,
   Levenberg–Marquardt by default and DogLeg by `SolverOptions`, dense
   `nalgebra` matrices. Hard caps on iterations and on function
   evaluations; convergence when every residual is within the equation's
   tolerance. A component that hits a cap returns its best iterate and is
   reported `NotConverged`, never an error and never a hang. A component of
   more than one part (`stage`) is solved part by part in dependency order,
   head, stages, remainder, each with the earlier parts' parameters as
   constants and the component's frozen scale, origin and acceptance;
   a satisfied part takes no step, so an edit leaves the parts it does not
   reach bit for bit. A part that held at the component's start and is
   moved only by its earlier parts starts from its first-order
   continuation, `−J_x⁺ (F + J_u Δu)` at the start: exact under a
   translation, so a stage carried far by its parents keeps its branch
   where a descent from where it stood can take the mirror root (a long
   triangle strip's rotated tail). A part's step or evaluation spends the
   share of the component's parameters it moves of the caps, a whole
   component exactly one, and no one descent takes more steps than the
   cap: hundreds of small parts fit a budget sized for one component.
   Goals are pursued in the last part only. A part that
   does not converge, or a result the diagnosis would not stage, sends the
   component back to the whole solve from its start, the budget counters
   restored: a failed component keeps the whole path's bits and best effort,
   at most twice its work.
4. **Diagnose** at the solution: the Jacobian of all equations, rank by
   column-pivoted QR with the scaling and tie order of ADR-0003, DoF, free
   directions, redundant and conflicting sets, degenerate entities and branch
   flips (DATA-MODEL.md §Diagnostics). The QR is Householder with column
   norms recomputed at every step (no downdating) and ties to the lowest
   original parameter, written by hand in index order: no library's tie
   rule or kernel choice reaches a DoF. It stops at the first remaining
   column whose part outside the chosen span is at most `RANK`. A satisfied
   component is factorized part by part over its stages (`stage`): each
   part's rows, unit over the whole row, cut to its own columns; the rank
   is the parts' sum and the free directions the last part's null basis.
   A failed or degenerate component, one whose part before the last is
   column-deficient, or one with an entry that breaks the parts'
   triangular pattern, is factorized whole. Both give the same DoF and
   direction span; the non-default `reference` feature exposes the whole
   path (`laman::reference::solve`, and `diagnose` at given coordinates;
   hidden, unbound) for the tests to compare, and `downstream`, the
   parameters a change of one constraint can move.

**Priorities**: two levels, kept in two lists. Every constraint's
equations are required, compiled into `System::equations`; a drag target
is a goal, compiled into `System::goals`, a list of its own with no
constraint source. Required equations are solved first; a
goal is solved in the null space the required equations leave, so a goal
can never push a required constraint off. Diagnostics never blame a goal
for a conflict.

A goal is `value(param) − target` on one free parameter, compiled after
every required equation into a list of its own, never into the required
equations: DoF, redundancy and conflicts read only those, so no diagnostic
can see a goal. A goal on a fixed parameter is dropped (a point with one
fixed coordinate is pursued along the other); a goal joins the component
of its parameter, and a component without one solves exactly as before.

A component with goals first solves its required equations as above, then
pursues the goals lexicographically: Levenberg–Marquardt on the reduced
problem, sequential quadratic programming on the required equations
(Nocedal & Wright, *Numerical Optimization*, §18.1). Each iteration takes
the null basis `N` of the required Jacobian (ADR-0003's QR), the projected
gradient `p = Nᵀ J_gᵀ g`, and the reduced Hessian `H = (J_g N)ᵀ(J_g N) +
Nᵀ W N`, where `W = Σ λᵢ ∇²cᵢ` is the required curvature weighted by the
least-squares multipliers `λ = −(J_rᵀ)⁺ J_gᵀ g` (read from the pivoted QR,
§Diagnose), read as a forward
difference (step √eps) of the analytic Jacobian along each null vector.
The step `N z`, `(H + μI) z = −p`, is of least norm within the null space;
`μ` is LM's damping, adapted as above, times the larger of
`max diag (J_g N)ᵀ(J_g N)` and `max |diag H|`, never a floor of 1: on a long
chain `H` is far below 1 and a floor holds every step short (the 498-rectangle
drag took 5 iterations for 3), the first stays positive where curvature cancels
`H` to zero (a point on a circle of radius 10 reaching for (0, 1) from
(10, 0)), and the second keeps the raises in reach of a large negative curvature.
The step is cut to one component
size, then corrected back onto the required equations by the required
descent, and accepted when that converges and the goal's objective does
not rise by more than the correction's own noise: near stationarity the
true decrease is below that noise, and refusing such steps stalls short of
it. A step that does not fall is accepted at most `LEVEL_STEPS` (3) times
in a row; a longer run is a stall (measured: a step that rounds away
against an origin 1e74 away leaves the objective unchanged and was
accepted until the budget), and the goal ends not converged at the last
iterate. A component without required equations takes the exact
Gauss–Newton step.

The Gauss–Newton model alone (no `W`) is not enough: it ignores the
curvature the correction follows. Measured, a point on a circle reaching
for a target inside it undershoots by the ratio of their radii every
iteration (128 iterations spent, not converged), and a two-bar linkage
reaching past its span bends and straightens in turn. Damping an
indefinite `H` instead of adapting `μ`, or a line search on the Newton
step's length, proposed steps hundreds of sizes long near a fold.

A component with goals converges when the required equations meet
acceptance and every goal is met within it (its polishing target after a
step), or the goal is **stationary** in the null space: `|p|` within that
acceptance plus `GOAL_ROUNDING = 1e-12` of `|g|`. An unreachable target
(fixture 4's y) is therefore converged, not `NotConverged`. A goal that
cannot be decreased or a spent budget is `NotConverged` with the last
iterate whose required equations held: a goal never leaves a required
residual over acceptance. Goal probes, curvature evaluations and their
corrections count against the shared iteration and evaluation caps.

Measured (`solve::tests`), under both engines:

| Case | Iterations / evaluations |
|---|---:|
| Fixture 4, each of the ten steps from the previous one | 3 / 14, no rejection |
| Point on a circle of radius 10, target (30, 40) (outside) | LM 13 / 22, DogLeg 7 / 16 |
| Same, target (0, 1) (inside) | LM 15 / 26, DogLeg 8 / 19 |
| Same from (0, −10), target (−3, 0.5) | LM 23 / 38, DogLeg 12 / 27 |
| Two-bar linkage, targets up to 5 link-lengths away, 5 000 cases | all converged, checker clean |

Stationary exits end with `|p|` from 2e-16 to 8e-13 of the component's
scale against an acceptance of 1e-12. A fixture-4 step's 3 iterations are
3 probes, each exact up to the damping (1e-3, then divided by 3) that the
next one removes; its corrections take no step.

**Least change**: among the solutions of an under-constrained component,
the solver takes the one nearest the starting point: the damped step is
the minimum-norm one, and a component with nothing to change takes no
step, so its bits are the start's bits. A solve of an already solved
sketch returns the same bits (SEED.md §8).

The numerical contract is fixed by [ADR-0002](adr/0002-regularize-structural-tangent-contacts.md)
(contact regularization) and [ADR-0003](adr/0003-scale-relative-tolerances-and-rank-order.md)
(component-relative lengths with an absolute floor, dimensionless rank and
angular thresholds, and deterministic rank/null-space ordering). The compiler and checker use the shared model
`Tolerances` policy. The compiler freezes it from the input geometry and
dimensions. The checker measures it at the coordinates it checks
(`Tolerances::at`, [ADR-0004](adr/0004-checker-tolerance-at-checked-coordinates.md)),
so a guess never loosens a verdict. An iterate's length acceptance is the
smaller of the two, so the solver never stops short of the checker. Components
connect structural free-parameter reads, including zero derivatives; ground
references alone do not join them. Fixed-only equations retain source order.
Multi-equation constraints are kept together. The legacy `LENGTH` constant
remains available for compatibility; it no longer controls checker acceptance.

A structurally shared tangent contact uses the perpendicular radius
equation, retaining membership, original sense and source provenance.
Both solve and diagnostics use this regularized system; the independent
checker measures the original geometry. This recovers geometric DoF at
C1 contacts where original Jacobian nullity overcounts it. Unsupported
singularities retain an infinitesimal interpretation (ADR-0002).

### LM engine

The internal LM engine solves scaled damped normal equations
`(JᵀJ + λI) Δ = −Jᵀf` by dense Cholesky (`nalgebra`, `libm-force`).
Products accumulate in equation order. Identity damping gives the unique
minimum-norm damped step without adding null-space motion in exact
arithmetic. Coordinates use the frozen component origin and common scale;
fixed and untouched parameters retain their bits. A non-finite transform,
factorization or trial cannot certify convergence.

**The radius bound** (ADR-0010), shared by both engines: a circle's radius
is held `≥ 0` by a projected step with an active set. A radius on its
bound that `Jᵀf` presses below has its column dropped from the model the
step is taken from; a step that would carry any radius below 0 is cut at
0, and the gain ratio reads the cut step's prediction; every trial holds a
radius at 0 rather than below it. Where no radius is active and no step
crosses 0, all of this is the unbounded iteration bit for bit.

The numerical defaults, exercised by `laman`'s `solve::tests`, are:

| Setting | Value | Purpose |
|---|---:|---|
| Initial damping | 1e-3 times max(1, largest diagonal of JᵀJ) | Regularizes deficient systems while allowing rapid convergence |
| Accepted-step damping | divide dimensionless damping by 3 | Reduces damping after strict objective decrease |
| Rejected-step damping | multiply by 10 | Shortens the next attempted step |
| Damping floor | 1e-15 | Keeps regularization representable near unit Jacobians |
| Iterations / evaluations | 128 / 256, across all components, in whole-component units (a part's step spends its share of the parameters) | Hard bounds, including rejected and non-finite trials |
| New-iterate target | 0.1 times geometric tolerance | Resolves coordinate-error accumulation in coupled geometry |

A satisfied input takes no step. A newly converged iterate must meet the
stricter target, the original tangent residuals and stored side domain.
Vector constraints use their geometric norm (including the doubled
midpoint-to-axis error for symmetry), not two separate scalar tests.
An initial evaluation costs one evaluation; each attempted step costs one
iteration and one evaluation, even if its factorization or trial is refused.
Empty components cost neither. Rejection retains the last, lowest-objective
finite iterate; zero steps above tolerance are stagnation, not convergence.

From the C1 guesses, triangle and belt each take 5 iterations / 6 evaluations,
and the slotted plate takes 6 / 7, with no rejected trials. All are checker
clean, inside the unchanged coordinate bounds, and bit-identical on a
zero-step re-solve. The coupled stopping regression needed one additional
iteration: stopping at geometric acceptance alone missed E2.y's coordinate
bound. Contradictory fixes retain their analytic midpoint and exhaust the
128-iteration cap with 125 rejected trials. Construction properties exercise
radial solves over scales 1e-6 through 1e6; extremes include nonzero motion
at 1e300 and unrepresentable differences. These are measured examples,
not a general convergence guarantee. Conflict certification and the
status are the diagnosis's (§Diagnose).

### DogLeg engine

Powell's dog leg (Madsen, Nielsen & Tingleff §3.3) runs on the same driver
as LM: component order, normalization, trials, the satisfied-input
early-out, the new-iterate target, vector acceptance and every budget rule
above are shared. Only the proposed step and its control (a trust radius
in normalized coordinates instead of a damping) differ. The step is the
Gauss–Newton step when it fits the radius. Otherwise it is the Cauchy step
cut to the radius, or the point where the leg from the Cauchy point to
Gauss–Newton leaves the region. All three lie in the row space of J, so
DogLeg keeps LM's minimum motion. Radius updates use the gain ratio
(actual over predicted decrease); a non-positive prediction counts as poor.

The Gauss–Newton leg is the minimum-norm least-squares step through a
truncated SVD (`nalgebra`'s bidiagonal SVD with a hard step bound), not
normal equations. Each normal-equations form failed one C1 shape:
- `JᵀJ` with a small regularization leaked rounding into free directions.
  A free pair under one distance moved 1e-5 off its symmetric extension.
- `JJᵀ` lost the step to cancellation on dependent equations. Two fixes of
  one point stalled 1.9e-7 short of their midpoint.

The SVD handles both kinds of deficiency. Projections and the step are
summed by hand in index order.

| Setting | Value | Purpose |
|---|---:|---|
| Initial radius | 1 (one component scale) | Radii 0.01–100 all solve the cases below; 1 is fastest on the C1 bases |
| Growth | radius ≥ 3 × accepted step when gain > 0.75 | MNT §3.3 |
| Shrink | halve when gain < 0.25, and on rejection | MNT §3.3 |
| Collapse | radius below 1e-15 stops the component | Under an order-one coordinate's rounding |
| GN cutoff | σ ≤ 1e-10 σ_max dropped | Cutoffs 1e-14–1e-6 give identical results here |
| SVD bound | 6 k² inner steps for k singular values | LAPACK xBDSQR's bound; a failed SVD is a refused trial |

From the C1 guesses, triangle, belt and slotted plate each take 4
iterations with no rejection; their redundant twins take 4 each. The
conflicting twins return finite best effort, not converged, after 54–58
iterations: rejections shrink the radius at the least-squares point until
it collapses. Contradictory fixes reach their analytic midpoint in 2
iterations. A trust region of 1e3 started near the line of two distance
centres rejects 11 overshooting Gauss–Newton steps, then solves on the
guess's branch. A stationary singular start (`Jᵀf = 0`, `JJᵀ` singular)
returns its own bits, not converged. An unsatisfiable subset with as many
equations as parameters has its least-squares point where `J` is
singular: the Gauss–Newton step is unbounded there, each step is cut to
the radius in the span of `Jᵀf` and that step, and the descent converges
linearly (about 200 iterations on `regression/staged-conflict-inconclusive`'s
four-distance subset, LM 85), so at the 128-iteration cap it ends above
`STATIONARY`. The conflict analysis therefore re-solves by LM (§Diagnose).
LM keeps its own defaults and
measurements above; both engines run every analytic case in `solve::tests`.

nalgebra's dense `DMatrix` products dispatch to `matrixmultiply` above 5×5,
which chooses FMA kernels by CPU at run time. Engine code therefore forms
products with explicit loops. The Cholesky decomposition and SVD it calls
do not use that path.

### Diagnose

The internal `diagnose` analysis reads a solve's coordinates and returns
the status, the freedom above, the redundant constraints, the conflict
sets and degeneracy ([ADR-0005](adr/0005-conflict-sets-by-certified-deletion.md)).

- **Status** comes from geometric acceptance at the coordinates, not from
  the engine's stricter polishing target. It is `Unsatisfiable` when any
  conflict set stands, `NotConverged` when some component fails without
  one, and otherwise follows the DoF.
- **Redundancy** removes satisfied constraints in reverse source order,
  with contacts and scales recompiled, and names those whose removal keeps
  the geometric DoF. A constraint it names stays removed, so of two
  duplicates the later is named. It costs no solve. In a staged
  component only a constraint with a head equation is tested: when every
  part after the head has full row rank at the coordinates, every left
  null vector of the Jacobian lives on the head's rows, so a stage or the
  remainder is never redundant (ADR-0009). A part after the head with a
  row dependent on the others (a point on a line through a point it
  coincides with: the structure hides it) sends the component to the full
  test. Each test still compares the whole DoF. The first list of head
  constraints is read on the analysis's own compilation, at the scales its
  status and freedom were judged at; after a constraint is named, on the
  recompiled sketch.
- **One staged factorization per component and frame.** The solve's
  acceptance of a staged component, the freedom and the head constraints
  each read the component's staged factorization; where they read it at
  the same coordinates it is made once and kept for the frame, matched by
  the component's parameters, sources, scale and parts and by the bits of
  every coordinate it reads. A static solve or an edit makes one per
  staged component; a drag frame two, its solve accepting the staged
  result before it pursues the goal. Each is made from the rows' sparse
  terms, not the component's dense Jacobian: a zero entry adds nothing to a
  row's maximum or sum of squares, so the blocks are the dense ones bit for
  bit. The rows are read once: the factorization tests satisfaction on the
  rows it keeps for its blocks, and the factorization keeps that answer, so
  the analysis reads it back instead of testing again at the same
  coordinates (a frame whose pursuit moved them misses the key and tests
  afresh). A component's reads are made where the component is, not on
  every lookup.
- **The QR skips exact zeros and keeps its reflectors.** `Qr::new`, the
  pivoted Householder factorization behind the rank and the null basis
  (ADR-0003), applies a reflector only to the rows where its vector is
  nonzero, skips a column whose dot with it is exactly 0, and recomputes a
  column's norm only when a reflector changed it or the row just retired was
  nonzero there. Every skipped term is an exact zero, so the pivots, R and
  every bit (up to the sign of an exact zero) are the dense method's; this is not a sparse method. The
  reflectors are kept, so `Q` applies without forming it.
- **Multipliers come from the QR.** The pursuit's least-squares multipliers
  `λ = −(Jᵀ)⁺ b` (`b = J_gᵀ g`) are read from a pivoted QR of the Jacobian's
  unit rows, with `J P = Q R`: `b` is cut to its part in the range of `Jᵀ` by
  taking out the null basis in hand, `R₁₁ᵀ y = (Pᵀ b)₁..r` is solved by
  forward substitution and `λ = −Q [y; 0]`, the minimum-norm solution the
  SVD gave. Each unit row is scaled back by its norm and mapped to its
  equation; a zero row has multiplier 0, and on dependent rows the solution
  is the minimum-norm one over the rows as scaled to unit length.
- **Coordinates are dense inside a solve.** A solve and the analysis hold
  every sketch parameter's value in a `Dense` table (a shared layout and a
  vector) read through the `Values` trait, as the `Coordinates` map is. A
  `Coordinates` is made once, where a `Solution` is, and for the conflict
  analysis, which keeps its maps. Ids are the caller's `u32`s, so a table
  indexed by raw id could be enormous: ids below a small multiple of the
  entity count index a direct table, larger ones a sorted vector searched
  by bisection. Which route an id takes never changes the value read.
- **Conflicts**: for each failed component, a deletion filter in source
  order over its constraints. Each test re-solves a copy of the sketch that
  holds only the tested constraints, from the best effort and then from the
  guesses, by the default solve: staged on either path (whose fallback is
  the whole solve from the same start, so a whole-path test gains its
  witnesses and certifies nothing the whole re-solve would not; ADR-0005,
  second amendment), and by LM under either algorithm, the caller's caps
  standing (the dog leg crawls at a singular least-squares point, §DogLeg
  engine; third amendment). It counts as satisfiable when either re-solve converges. It
  counts as unsatisfiable only when neither converges and one ends at a
  certified stationary least-squares point: `|Jᵀf| ≤ STATIONARY ·
  |(f_i |J_i|)|`, with `STATIONARY = 1e-6`, the gradient projected where a
  radius on its bound is pressed below 0 (ADR-0010). A certified local minimum of a
  satisfiable subset therefore never settles a test while the other start
  can still converge. Anything else, a budget spent before the last start
  included, is inconclusive. Every member a set
  keeps has a converged removal witness, so the set is inclusion-minimal;
  its unsatisfiability is the local certificate. Once a set is found, its
  greatest id is set aside and the rest is filtered again for a further
  set.
- **Bound**: at most 256 re-solves per analysis, each under the caller's
  caps. Running out is inconclusive.
- **Degeneracy**: entities collapsed below the length tolerance, measured
  with `Tolerances::at` like the checker, each with the equations that read
  it. Separately, the equations whose residual is undefined or non-finite.

Measured under both engines: the C1 bases and redundant twins take no
re-solve and name exactly `right-angle`, `same-belt` and `level`. The
conflicting twins take 8, 19 and 34 re-solves: every test that ends
unsatisfiable spends both starts. The belt and plate sets are the roadmap's. The triangle's is
{`base`, `right`, `apex`}, which is inclusion-minimal; the roadmap's
four-member set is the residual's support (ADR-0005 Consequences). The
C1 twins' best efforts end with stationarity 1.6e-10 to 3.2e-8, and their
subsets' re-solves with 1e-14 to 3e-9. Two contradictory fixes cost LM the
whole shared iteration budget, so a second component it never reached is
re-solved alone before it is judged. A conflict whose descent shrinks a
tangent line toward zero length certifies nothing and stays
`NotConverged`.

## Branches

A constraint system has discrete solutions: a circle tangent to a line on
either side, an angle or its supplement, an arc swept the short or the
long way. A sketch sits on one **branch**, and an edit or a drag keeps it
there (SEED.md §4, differentiator 2).

- The constraints whose equations admit more than one root carry a
  **sense** chosen when the constraint is added, from the drawing: the
  side of a tangent line, internal or external contact of circles, the
  side of a point–line distance. The equations are written signed, so the
  other root does not satisfy them (DATA-MODEL.md §Constraints).
- What no equation holds is **observed** (the internal `branch` module,
  [ADR-0006](adr/0006-branches-by-observation.md)): each arc's sweep,
  tracked from the last observation so that a semicircle's rounding never
  reads as a turn; each circle's radius; the orientation of the lines of
  each `parallel` (sign of `dot`), `perpendicular` (sign of `cross`) and
  `angle` (sign of `cos(φ − θ)`). An observation's branch is *known* only
  beyond a margin: an arc's radius and chord and a circle's radius above
  the entity's length tolerance at those coordinates (it is not
  degenerate), a line pair's quantity above `tolerance::ANGLE` with both
  lines resolved. Inside the margin it carries the last known branch, so
  a crossing spread over two solves is still caught.
- A static solve compares its solution's observations with the guesses';
  every drag step compares with the last solution's. A subject known on
  both sides whose branch differs is a **flip**, named in diagnostics with
  the quantity before and after. An arc flips when its tracked sweep
  leaves `(0, 2π)`.
- A drag step that would flip is bisected toward the last good target
  (continuation, at most `SolverOptions::max_bisections` halvings). One
  still flipping returns the solution at the farthest target that kept
  every branch, with the flip named; the session stays on its branch.

Tracking assumes a solve moves a sweep by less than half a turn. A drag
frame enforces a tighter bound: one that moves an arc's tracked sweep by
more than `tolerance::MAX_SWEEP_STEP` (an eighth of a turn) is bisected as a
flip is, naming none (ADR-0006, amendment). A static solve does not: it
reads the sweep the short way round from the guesses. Below the
length floor (ADR-0003) no branch is known and nothing flips.

## Drag sessions

```rust
let mut drag = sk.drag(point, &options)?;    // ModelError for a non-point or a fixed point
let step = drag.to([x, y]);                  // one frame: a Solution
let done = drag.finish();                    // the last Solution
sk.apply(&done)?;
```

A `DragSession` owns a clone of the sketch, placed at the last solution's
coordinates (the warm start), not a borrow: the caller's sketch stays
untouched and editable. `drag` refuses an unknown id (`UnknownId`), a
non-point (`WrongKind`) and a point fixed in both coordinates
(`ModelError::FixedPoint`); a point with one fixed coordinate drags along
the other. It solves the sketch once, which is the session's first last
solution, so `finish` before any `to` is the static solution.

At `drag` the session cuts every component into parts once (`stage`) and
keeps them: the equations a session solves never change and the cut does not
read coordinates, so each frame and each bisection reuses it, in the solve
and in the diagnosis, and a stage the dragged point does not reach is
satisfied and keeps its bits.

Each `to` compiles the target as a `Goal` pair on the dragged point's
coordinates, solves from the previous step (least change; §Priorities),
diagnoses, observes the branches after the last frame's (§Branches) and
returns a full `Solution` whose flips are that comparison. Geometry with no
equation path to the dragged point is in another component and is not
touched: its bits stay the bits it started with. A target that is not
finite is no frame: the last solution comes back with `NotConverged` and
the session is unchanged. Every solution of a session claims the sketch it
was opened on, so `apply` takes any of them on it, unedited: no frame
ends with a radius below zero (ADR-0010, regression
`drag-negative-radius`). The frame
budget a step must meet is ROADMAP.md's: ≤ 1 ms on fixture 4, ≤ 4 ms a try
at 500 entities (§Frame budget).

**Per-step diagnostics.** Every step runs the full diagnostics, the same
`analyse` a static solve runs, so a step's `redundant` is never stale;
the frame-cheap alternative, redundancy only on `finish`, is not needed.
The redundancy pass takes the rank alone (`dof_only`: no null basis, no
per-entity counts) and tests only the head's constraints (§Diagnose). Each of
fixture 4's ten steps, solve and diagnostics, takes 0.20–0.22 ms under both
algorithms (§Frame budget), inside its 1 ms. At 500 entities the first
drag step takes about 5.6–6.0 ms on rectangles, where the remainder's goal
phase is still the largest part, and about 3.4–3.8 ms on slot grids
(§Frame budget); C2's *Accept* carries 6.0 ms for the rectangle drag.

**Bisection.** A step whose solve flips a branch, or moves an arc's sweep
past `MAX_SWEEP_STEP` (§Branches), is retried
toward the last good target: the full step, then half of it, a quarter, and
so on, each solved from the last solution (never from the failed try), at
most `SolverOptions::max_bisections` halvings. The first try that keeps
every branch is the frame, `flips` empty and the dragged point stopped
short of the target; the session's reference target moves to it, and the
frame says so in `Diagnostics::bisection` (the target solved at and the
halvings; `None` for a frame at its own target). If every
try flips or jumps, the frame is the last solution unchanged with the nearest
flipping try's flips named and `bisection` the last halving tried (never solved
at; with `max_bisections` 0, `halvings` 0), and the session stays
where it was (ADR-0006): a drag cannot
cross a branch, only a static solve can. `max_bisections` defaults to 8:
the smallest step tried is 1/256 of the frame's, below what the eye reads
on a drag, and a refused frame costs at most that many extra solves,
measured at about 280 µs against 42 µs for a plain step on the two-line
fixture (`tests/fixtures/drag/bisect-stuck`, release, one machine; ≈ 30 µs
per solve, diagnostics only on the frame returned). Zero tries the target
alone.

## Profiles

`laman::profile::profiles(&Sketch, &Coordinates) -> Result<Profiles,
ProfileError>` reads any coordinates, a solution's included, and takes no
`Solution`. It reads declarations, not measurements ([ADR-0012](adr/0012-profiles-join-by-declaration.md)).

- **Join classes.** Points are joined by union-find over `coincident`
  constraints, each class represented by its smallest point id. A violated
  joining `coincident` still closes its loop; the loop names it.
- **Topology.** An edge is a non-construction line or arc, its ends being
  classes; a non-construction circle is a loop alone. Degree-1 classes are
  pruned repeatedly in id order: a component pruned away is an open chain,
  a pruned edge of a component that keeps a core is dangling. A core whose
  classes all have degree 2 is a loop, ordered from its smallest edge id;
  a core with a class of degree > 2 is branched and reported, none of its
  edges entering a profile. An edge with both ends in one class is
  self-joined. Free ends of open chains and dangling edges that lie within
  the length tolerance of each other (`Tolerances::at`) are reported as
  unjoined coincidences, never joined.
- **Geometry.** Area is Green's integral in loop order on coordinates
  translated to the loop's first join and scaled by an exact power of two,
  so it neither overflows nor loses a small loop far from the origin; an
  arc adds its segment `r²(θ − sin θ)/2`, by a series below one radian
  (measured) and directly above. The area saturates at ±`f64::MAX`.
  Containment is the winding number at the midpoint of the inner loop's
  first edge; a loop is a hole exactly when its innermost container is an outer
  boundary, and that container is its parent. Extraction is quadratic in the number of loops.
- **Why the types sit under `model`.** `Profiles`, `Profile`, `Loop`,
  `Traversal`, `Branched` and `Unjoined` live in `model::profile` so that
  the checker may name them under the layer rule.

## The checker

`laman_check::check(&Sketch, &Coordinates) -> Report`. For every
constraint and every intrinsic equation it measures what the constraint
means — a distance as a distance, tangency as the distance from centre to
line against the radius, an angle by `atan2` — and compares with the
tolerance. A `Violation` names the constraint id, its tag, its kind, the
measured value and the wanted one. It also reports degenerate entities.
It is written from DATA-MODEL.md's definitions, not from `laman::residual`
(the layer rule above), and measures differently where it can: residuals
are written for the solver's conditioning, the check for readability.

Every test that solves runs the checker on the result; a fixture that
expects violations lists them. The Python package exposes it as
`laman.check`.

## Determinism

Same input, same bits, on Linux and macOS (x86_64 and aarch64), Windows
(x86_64) and wasm32 (SEED.md §4, §9):

- `BTreeMap` and `Vec` only where order reaches a result or an id; no
  `HashMap` iteration. A value read from the dense table is the value the
  map holds: the route a lookup takes (direct slot or bisection) never
  changes it.
- Every sum over equations or parameters runs in the compiled order. No
  parallel reduction; no result depends on threads (laman starts none).
- Transcendentals through `libm`, pinned exactly; `clippy.toml` refuses the
  `std` methods. `sqrt`, `abs`, `mul_add`-free arithmetic: no FMA is
  requested, and the crate is never built with `-C target-cpu=native` for
  a digest.
- `nalgebra` with `libm-force`, as in arris; its decompositions run in a
  fixed order on these sizes.
- Floats parse and print through `serde_json` with `float_roundtrip`.

The **digest** is SHA-256 over a run: the sketch's serde text, then for a
solved run the solution's coordinates (little-endian `f64` bits in id
order), status and every diagnostic in the order the solution lists them,
then the checker's report at the solved coordinates (floats by their bits).
`laman_debug::digest::of_solution` writes it; `of_run` is the older form,
over the sketch and the report at a fixture's expected coordinates;
`of_drag_step` prefixes a drag step's index and its target's bits to the
same bytes. A profile run adds the coordinates it was extracted at (or
the solution), the extracted profiles and the profile check's report:
`of_profiles` and `of_solved_profiles`, which leave the digests above
unmoved. CI's `test` matrix runs the gate, including all 81 blessed
digests (the 50 of the C1 corpus and drag, the 31 of the C3 profile runs in
`tests/fixtures/c3/digests.txt`: each fixture at its expected coordinates
and at each algorithm's solution, and the 3 by 3 slot grid), on Linux x86_64 (`ubuntu-latest`)
and aarch64 (`ubuntu-24.04-arm`), macOS aarch64 (`macos-15`) and x86_64
(`macos-15-intel`), and Windows x86_64 (`windows-latest`). Each compares
against the same committed digests. The Linux-only `wasm` job also
compares native output with `wasm32-wasip1` under wasmtime
(`tools/digest-wasm.sh`, compared sorted: the tests run in parallel).
`laman-debug` holds the blessed digests, blessed only
after `tests/c1.rs` has checked the same solves. A change that moves a
solution's bits re-blesses them in the same commit, saying why
(`.agents/review.md`).

The **fuzzer** has four targets outside the workspace: `load_sketch` (any
bytes are a sketch or a refusal), `solve_sketch` (any loadable sketch
solves under both algorithms to a complete answer that applies back),
`drag_sketch` (any loadable sketch, any point, any targets, raw `f64` bits
included: every step a complete answer, a non-finite target `NotConverged`)
and `profiles_sketch` (finite, repeatable profiles at a sketch's and a
solution's coordinates; missing values refused, never a panic; then
`check_profiles` on the found and on mutated profiles, never a panic, and
clean on the found wherever `check` is clean).
`cargo run --release -p laman-debug --example bench` times the static C1
fixtures and each step of fixture 4's drag against the 1 ms frame budget and
names the machine.

The Rust-only `laman_debug::corpus` harness builds C2 sketches with
closed-form coordinates, deterministic displaced guesses, dimension edits
and drags. `Case` holds the static sketch and `Expected`, an `Edit` with
its tag, replacement value, constructed answers and upstream `unmoved`
entities, and an optional `DragCase`. The latter holds its own sketch
with one dimension removed, initial answers and `fixture::Drag` targets
with answers at every step. Zero counts and non-positive or non-finite
dimensions are refused with `CorpusError`; model refusals retain their ids.

The families are shared-edge rectangles (`5n+3` entities), C1-shaped slot
grids (`10rc+8`), and equilateral triangle strips (`3n+3`). Each has a
default constructor and a `_with` constructor taking dimensions. Rectangle
edits change a middle width; later rectangles shift. Slot edits change one
slot's vertical placement; a separate horizontal-placement dimension is
removed for its drag. A triangle edit changes one middle skip-edge; the
tail rotates rigidly about the preceding point by construction. Triangles
have no constructed drag. The other two families have one-DoF drags with
three targets. These generated cases do not replace the committed C1 fixtures.

`tests/c2.rs` checks static, edited and dragged coordinates, status, DoF,
empty redundant/conflict/flip diagnostics, semicircle sweeps and checker
results under both algorithms. Coordinate acceptance is `1e-9` times the
maximum x/y extent of the constructed answers; sweep acceptance is
dimensionless `1e-9`. An edit keeps the bits of every parameter
outside its reach: the construction's upstream entities and whatever the
structure puts outside the edited stage's downstream closure
(`laman::reference::downstream`). Up to 50 entities each edit and drag frame
is also diagnosed whole and held to the staged DoF, redundant and conflict
sets. Every size
runs un-ignored, each under 10 s in debug (the 500-entity rectangles,
slowest, 7 s with their drag), the 498-entity slot grid under DogLeg
included.

`tests/c2_props.rs` uses proptest's default case count and
`PROPTEST_CASES`: rectangles draw 1–6 cells, widths 10–40 and heights
5–20; slots draw 1–2 rows/columns, centre separations 20–60 and radii
3–8; triangles draw 1–8 cells and sides 10–30. Each checks its static
solve and warm-started edit against construction under both algorithms.

### Frame budget

The budgets are ROADMAP.md's: ≤ 1 ms for each C1 fixture and drag step;
≤ 4 ms for a one-dimension edit and a drag step at 500 entities, a bisected
frame judged per try, the rectangle drag accepted at 6.0 ms (ADR-0011). The
numbers below are one machine's (AMD Ryzen 9 7950X 16-Core Processor,
Linux x86_64, 32 logical CPUs, release), measured 2026-11-07.

**The C1 bench** (`bench`, median of 200 runs): the static bases and
redundant twins take 0.02–0.04 ms (triangle), 0.06–0.13 ms (belt) and
0.12–0.27 ms (slotted plate), under Levenberg–Marquardt and DogLeg alike;
each of the drag's ten steps takes 0.20–0.22 ms under both. The conflicting
twins run the conflict analysis and are timed, not budgeted: 0.9–1.0 ms
(triangle), 12 ms (belt), 42–43 ms (plate).

**The C2 bench** (`cargo run --release -p laman-debug --example bench_c2`)
times the generated families (§Determinism) at about 100, 250 and 500
entities, diagnostics included. Static solves start from displaced guesses
and are reported, not budgeted; edits start from the initial solved
coordinates after replacing one dimension; drags time the first target in a
newly opened session. Construction, the initial edit solve and session
opening are outside the timed operation; the flushed worker timing marker is
inside it. The 498-entity slot grid has two more cells, each a bisected
frame (`laman_debug::corpus`; the worker fails if the frame is not bisected
so): `bisect`, its drag with the middle slot's left arc let loose of its
tangent lines and the arc's start pulled a quarter turn about its centre, so
the first frame moves the sweep past `MAX_SWEEP_STEP` and is halved twice
(3 tries); and `bisect-deep`, a frame refused at every halving
(`max_bisections` + 1 = 9 tries), the most a frame costs. A bisected
frame's time is divided by its tries, the unit of the budget (ADR-0011).

Each position runs in a separate process with a 5 s wall limit, setup
included. A completed first sample above 100 ms is not repeated; otherwise
five samples are taken, which bounds the 52 workers to 260 s plus process
overhead, including operations that do not return. A cell is over budget
when its worst sample is. An interrupted timed operation prints a lower
bound, a setup that exhausts the limit gives no verdict, and the benchmark
exits zero on misses and timeouts.

One run, median / worst in ms, five samples; a bisect cell per try:

| Family | Actual entities | Algorithm | Static | Edit | First drag step |
|---|---:|---|---:|---:|---:|
| rectangles | 98 | LM | 0.622 / 0.855 | 0.527 / 0.565 | 0.549 / 0.595 |
| rectangles | 98 | DogLeg | 0.543 / 0.661 | 0.490 / 0.521 | 0.551 / 0.593 |
| rectangles | 248 | LM | 1.595 / 1.778 | 1.351 / 1.711 | 1.800 / 1.858 |
| rectangles | 248 | DogLeg | 1.494 / 1.626 | 1.367 / 1.466 | 1.800 / 1.830 |
| rectangles | 498 | LM | 3.587 / 3.800 | 2.904 / 2.913 | 5.614 / 5.759 |
| rectangles | 498 | DogLeg | 3.239 / 3.506 | 2.970 / 4.155 | 5.718 / 5.988 |
| slots | 98 | LM | 0.926 / 1.214 | 0.674 / 0.705 | 0.724 / 0.817 |
| slots | 98 | DogLeg | 1.032 / 1.277 | 0.674 / 0.715 | 0.754 / 0.840 |
| slots | 248 | LM | 2.458 / 2.617 | 1.750 / 1.765 | 1.837 / 1.930 |
| slots | 248 | DogLeg | 2.448 / 2.614 | 1.734 / 1.986 | 1.854 / 1.887 |
| slots | 498 | LM | 5.249 / 5.779 | 3.592 / 3.743 | 3.706 / 3.774 |
| slots | 498 | DogLeg | 4.676 / 4.973 | 3.299 / 4.393 | 3.420 / 3.443 |
| triangles | 99 | LM | 0.455 / 0.550 | 0.373 / 0.445 | N/A |
| triangles | 99 | DogLeg | 0.443 / 0.525 | 0.381 / 0.416 | N/A |
| triangles | 249 | LM | 1.084 / 1.360 | 0.973 / 0.987 | N/A |
| triangles | 249 | DogLeg | 1.146 / 1.256 | 1.023 / 1.040 | N/A |
| triangles | 501 | LM | 2.174 / 2.480 | 1.947 / 1.967 | N/A |
| triangles | 501 | DogLeg | 2.313 / 2.458 | 2.001 / 2.297 | N/A |

| Bisected frame (slots 498) | LM | DogLeg |
|---|---:|---:|
| `bisect`, per try of 3 | 2.602 / 2.802 | 2.508 / 2.539 |
| `bisect-deep`, per try of 9 | 3.809 / 3.957 | 3.803 / 3.838 |

Triangles have no constructed drag (N/A). Every 98- to 249-entity cell is
inside 4 ms. At 500 entities the edits' medians are 1.9–3.6 ms; a single
sample on the edge can cross 4 ms by noise (here the DogLeg edits, 4.2 and
4.4), which C2's *Accept* allows up to 4.4 ms. The slot drag is 3.4–3.8 ms,
a bisected frame 2.5–4.0 ms a try, and the rectangle drag 5.6–6.0 ms, inside
its 6.0. The 500-entity static solves of the slot grid (4.7–5.8 ms) are
reported, not budgeted.

**Where the rectangle drag's time goes** (instrumented in a scratch copy,
LM, medians over 12 samples, of a 5.0 ms frame): the pursuit 2.3 ms; the
head's and the stages' descents 0.4 and cutting the parts 0.15;
`stageable`, the first factorization and before the pursuit, 0.56;
`Tolerances::new` per `to` 0.42 (finding the components 0.13, framing them
0.22); the analysis 1.0, of it `freedom_on` 0.68 (a second factorization
0.5, the per-entity ranks 0.13), the satisfaction loop 0.18, redundancy
0.08, degeneracy 0.04; the branch observation 0.06; placing and cloning the
solution 0.03. `freedom_on` factorizes again because the pursuit moves the
goal's coordinates: the Jacobian at the final point is another matrix, and
its R and null basis are what the freedom reports, so reusing the earlier
factorization would change bits. Nothing outside the pursuit is removed
without changing a bit or a contract, which is where the stop rule ends the
work on that cell (ADR-0011). The smaller sources left in the other cells,
and the savings weighed and not taken, are in `docs/BACKLOG.md`.

Rigid bodies in the remainder, a cache of the diagnosis and a split of the
remainder out of the drag driver were each weighed against the profile and
are not built: the edits have no remainder, a diagnosis cache leaves the
solve's share and the bisected frame's tries (which carry no diagnostics),
and the pursuit's own cost fell by other means (ADR-0011; rigid bodies
under `docs/BACKLOG.md` §Rejected).

## wasm32

`laman` and `laman-check` build for `wasm32-unknown-unknown` with no
features and no glue: no file system, clock, environment or threads. The
gate builds it on every commit; CI runs the digest corpus on
`wasm32-wasip1`. JS bindings are not in v1 (SEED.md §9). `laman-py` is
empty on wasm32.

## Serde

The `serde` feature (on by default) gives `Sketch` and `Solution` their
serde forms; the checker's `Report` has none (`laman-debug`'s dumps render
it). The forms, their fields and their versioning are in DATA-MODEL.md §The
serde form.

## The Python binding

`laman-py` is a 1:1 layer, abi3 for Python ≥ 3.10, typed stubs held by
`mypy.stubtest`, tests held by `mypy --strict`. The binding computes
nothing.

- `laman.Sketch` has the Rust builder, solve, apply and drag methods under
  the same names; built by hand or loaded with `Sketch.from_json`. Entities
  are frozen, hashable handles (`Point`, `Line`, `Circle`, `Arc`) with
  `.id`; a loaded sketch hands them out with `point_handle` and its kin.
  Constraint ids are plain `int`. Keyword arguments carry what Rust spells
  as separate methods (`sk.point(0, 0, fixed=True, tag="origin")`); `Sense`
  is an `enum.Enum`.
- `Sketch.solve(options) -> Solution`, `apply(solution)`,
  `drag(point, options) -> DragSession` with `to(x, y)` and `finish()`; a
  call on a finished session raises `ValueError` (Rust moves the session).
  `SolverOptions`, `Algorithm` and `Status` mirror Rust's; `Solution` and
  every diagnostic are frozen classes with Rust's fields (tags `str | None`,
  payloads `bytes | None`; Rust enums with data are flattened to optional
  fields or strings).
- `laman.check.check(sketch, coordinates) -> Report`.
- `laman.debug`: `text`, `svg`, `solution_text` and the digests, as hex
  strings. Tooling, outside the stability promise (ADR-0007).
- The corpus (`tests/fixtures/c1/`, with `digests.txt`) is run by pytest
  under both algorithms; `hypothesis` builds Henneberg frameworks and solves
  them back. CI's `python` job runs `tools/py-test.sh` (pytest,
  `mypy.stubtest` and strict mypy) once, on Ubuntu with Python 3.10: the
  source build. The five platforms are covered by the wheels below. The
  script resolves venv executables through `uv run`, including Windows'
  `Scripts/` layout.
- `wheels.yml` builds abi3-py310 wheels on every pull request whose paths
  can change the package or its tests (`crates/**`, `tests/fixtures/**`,
  `tools/py-*`, `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, the workflow itself), on dispatch, and as
  the release workflow's build; a push to `main` does not run it. Linux x86_64 and aarch64 build
  natively inside manylinux2014 containers (glibc ≥ 2.17), macOS arm64
  and x86_64 use their native runners, and Windows uses x86_64. Separate
  clean runners install each artefact on Python 3.10 and run the same
  corpus, all 50 digests, stubtest and strict mypy, and the Linux x86_64
  runner repeats it on Python 3.13, the newest interpreter; `auditwheel show`
  checks both Linux wheels against `manylinux_2_17`. The sdist is built
  and installed in isolation on a separate Linux runner, with the same
  tests. `tools/py-package-test.sh` asserts imports come from the fresh
  venv, so the checkout cannot hide a missing file in a distribution.
- The release gate is the tag's own `ci.yml` run: `ci.yml` also triggers on
  `v*` tags, and `release.yml` does not call it. `publish-crate` refuses
  unless that run succeeded on the tagged commit (ADR-0008).

## Errors

| Type | When | Carries |
|---|---|---|
| `ModelError` | a mutation, a load or an `apply` that would make the sketch malformed or land a solution on a sketch it was not solved for | the ids involved and what was wrong |
| `RigidityError` | `rigidity` given a bad bar | the bar index and point |

In Python every `ModelError` variant is a subclass of `laman.ModelError`
(a `ValueError`), the ids as attributes, also reachable as
`ModelError.<Variant>`; Rust's `Entity` and `Constraint` variants are
`EntityError` and `ConstraintError`. Bad JSON text is a plain `ValueError`.

No other error type: solving, dragging and checking always return a
value.

## Open questions

- Closed by ADR-0006: branches are observed beyond the degeneracy and
  angle tolerances, carried inside them, and a drag keeps its branch.
- Closed by ADR-0005: a conflict set is inclusion-minimal, found by a
  bounded deletion filter with local certificates, never a minimum. The
  256-re-solve bound is not yet measured at 500 entities: the conflict
  analysis's cost is in `docs/BACKLOG.md`.
