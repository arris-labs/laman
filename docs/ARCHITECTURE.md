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
| `laman-debug` | text and SVG dumps, the fixture loader, `proptest` strategies, the digest, benchmarks | no (`publish = false`) | `laman`, `laman-check`; `proptest` behind a default feature of the same name |
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
| `residual` | `equations(sketch, values, constraint)` and `intrinsic(sketch, values, entity)`: each an `Equation` (value, `Unit`, a degenerate flag, and its analytic gradient as terms keyed by `(EntityId, Coordinate)`, merged and sorted, at most 8) at any coordinates; total, no `Result` (DATA-MODEL.md §Constraints) |
| `branch` | observations at coordinates (arc sweeps, circle radii, line-pair orientations) and the flips between two sets (§Branches) |
| `system` | a sketch compiled for one solve: the dense vector of free parameters, the equation list in order, the components |
| `stage` | each component cut into parts by the Dulmage–Mendelsohn decomposition of its equations: the head, the stages in dependency order, the remainder ([ADR-0009](adr/0009-stages-and-part-rank.md)) |
| `solve` | Levenberg–Marquardt and DogLeg over a `System`, warm start, priorities |
| `diagnose` | rank, DoF per entity, free directions, redundant and conflicting sets |
| `drag` | `DragSession` |
| `rigidity` | the generic rigidity count (the 0.0.1 API); `stage` does not use it |

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
  any other; it is excluded from closed profiles (a later cycle) and drawn
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
   never wrong.
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
least-squares multipliers `λ = −(J_rᵀ)⁺ J_gᵀ g`, read as a forward
difference (step √eps) of the analytic Jacobian along each null vector.
The step `N z`, `(H + μI) z = −p`, is of least norm within the null space;
`μ` is LM's damping, adapted as above; the step is cut to one component
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
| Fixture 4, each of the ten steps from the previous one | 4 / 13, no rejection |
| Point on a circle of radius 10, target (30, 40) (outside) | LM 13 / 22, DogLeg 7 / 16 |
| Same, target (0, 1) (inside) | LM 15 / 26, DogLeg 8 / 19 |
| Same from (0, −10), target (−3, 0.5) | LM 23 / 38, DogLeg 12 / 27 |
| Two-bar linkage, targets up to 5 link-lengths away, 5 000 cases | all converged, checker clean |

Stationary exits end with `|p|` from 2e-16 to 8e-13 of the component's
scale against an acceptance of 1e-12. A fixture-4 step's 4 iterations are
4 probes, each exact up to the damping (1e-3, then divided by 3) that the
next one removes; its corrections take no step.

**Least change**: among the solutions of an under-constrained component,
the solver takes the one nearest the starting point: the damped step is
the minimum-norm one, and a component with nothing to change takes no
step, so its bits are the start's bits. A solve of an already solved
sketch returns the same bits (SEED.md §8).

The M2 numerical contract is fixed by [ADR-0002](adr/0002-regularize-structural-tangent-contacts.md)
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
  result before it pursues the goal.
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
  |(f_i |J_i|)|`, with `STATIONARY = 1e-6`. A certified local minimum of a
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
budget a step must meet is ROADMAP.md's: ≤ 1 ms, measured by the bench on
fixture 4 (§Frame budget).

**Per-step diagnostics.** Every step runs the full diagnostics, the same
`analyse` a static solve runs, so a step's `redundant` is never stale;
the frame-cheap alternative, redundancy only on `finish`, is not needed.
The redundancy pass takes the rank alone (`dof_only`: no null basis, no
per-entity counts) and tests only the head's constraints (§Diagnose). Each of
its ten steps, solve and diagnostics, takes 0.39–0.40 ms under both
algorithms (§Frame budget), inside C1's 1 ms. At 500 entities the first
drag step takes 12–17 ms after M8's steps 2–5, over C2's 4 ms; the
per-part extent measurement and the diagnosis's recompile are gone, and what
remains is the solve of the sketch's rows (§Frame budget).

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
  `HashMap` iteration.
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
same bytes. CI's `test` matrix runs the gate, including all 50 blessed
digests of the fixture corpus and drag, on Linux x86_64 (`ubuntu-latest`)
and aarch64 (`ubuntu-24.04-arm`), macOS aarch64 (`macos-15`) and x86_64
(`macos-15-intel`), and Windows x86_64 (`windows-latest`). Each compares
against the same committed digests. The Linux-only `wasm` job also
compares native output with `wasm32-wasip1` under wasmtime
(`tools/digest-wasm.sh`, compared sorted: the tests run in parallel).
`laman-debug` holds the blessed digests, blessed only
after `tests/c1.rs` has checked the same solves. A change that moves a
solution's bits re-blesses them in the same commit, saying why
(`.agents/review.md`).

The **fuzzer** has three targets outside the workspace: `load_sketch` (any
bytes are a sketch or a refusal), `solve_sketch` (any loadable sketch
solves under both algorithms to a complete answer that applies back) and
`drag_sketch` (any loadable sketch, any point, any targets, raw `f64` bits
included: every step a complete answer, a non-finite target `NotConverged`).
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

Measured by the bench (release, median of 200 runs, AMD Ryzen 9 7950X,
re-run after M6, 2026-11-04): the static bases and redundant twins take
0.05–0.07 ms (triangle), 0.15–0.22 ms (belt) and 0.33–0.46 ms (slotted
plate), under Levenberg–Marquardt and DogLeg alike; each of the drag's ten
steps takes 0.39–0.40 ms under both. The conflicting twins run the conflict
analysis and are timed, not budgeted: 0.8–1.0 ms (triangle), 10–12 ms
(belt), 40–44 ms (plate). C1 keeps its 1 ms budget.

C2 baseline (M5, 2026-11-04, before stages), measured with
`cargo run --release -p laman-debug --example bench_c2` on AMD Ryzen 9
7950X 16-Core Processor, Linux x86_64 (32 logical CPUs), rustc 1.99.0.
The solver then solved whole components. The budget is 4 ms per operation,
including diagnostics. Static solves start from displaced guesses; edits
start from the initial solved coordinates after replacing one dimension;
drags measure the first target in a newly opened session. Construction,
initial edit solve and session opening are outside the timed operation.
The flushed worker timing marker is inside it.

Each applicable position runs in a separate process with a 5 s wall limit,
including setup. A completed first sample above 100 ms is not repeated;
otherwise up to five samples are taken. This bounds the 48 workers to
240 s plus process overhead, including operations that do not return.
The benchmark exits zero on budget misses and timeouts.

Cells give **median / worst in ms (sample count)**. A single observation
has equal median and worst; it is not a repeated-run estimate. **T ≥ x**
is an interrupted timed operation with a conservative lower bound in ms,
no completed median or worst. **S** means setup exhausted the 5 s wall
limit before timing began: no operation timing verdict. **N/A** means the
triangle family has no constructed drag. All completed samples and timed
lower bounds are **over** 4 ms; setup timeouts remain unmeasured.

| Family | Actual entities | Algorithm | Static | Edit | First drag step |
|---|---:|---|---:|---:|---:|
| rectangles | 98 | LM | 22.324 / 22.515 (5) | 21.712 / 22.112 (5) | 20.855 / 20.893 (5) |
| rectangles | 98 | DogLeg | 19.939 / 20.150 (5) | 19.415 / 19.791 (5) | 20.757 / 21.033 (5) |
| rectangles | 248 | LM | 497.297 / 497.297 (1) | 496.296 / 496.296 (1) | 508.140 / 508.140 (1) |
| rectangles | 248 | DogLeg | 498.801 / 498.801 (1) | 492.604 / 492.604 (1) | 507.539 / 507.539 (1) |
| rectangles | 498 | LM | T ≥ 4998.579 | S | S |
| rectangles | 498 | DogLeg | T ≥ 4998.585 | S | S |
| slots | 98 | LM | 111.531 / 111.531 (1) | 111.910 / 111.910 (1) | 118.767 / 118.767 (1) |
| slots | 98 | DogLeg | 118.863 / 118.863 (1) | 111.508 / 111.508 (1) | 118.227 / 118.227 (1) |
| slots | 248 | LM | 4271.589 / 4271.589 (1) | T ≥ 753.735 | T ≥ 789.130 |
| slots | 248 | DogLeg | 4336.484 / 4336.484 (1) | T ≥ 719.925 | T ≥ 786.688 |
| slots | 498 | LM | T ≥ 4997.838 | S | S |
| slots | 498 | DogLeg | T ≥ 4997.885 | S | S |
| triangles | 99 | LM | 18.649 / 19.163 (5) | 18.943 / 19.793 (5) | N/A |
| triangles | 99 | DogLeg | 19.499 / 20.618 (5) | 19.552 / 19.726 (5) | N/A |
| triangles | 249 | LM | 525.653 / 525.653 (1) | 527.927 / 527.927 (1) | N/A |
| triangles | 249 | DogLeg | 526.255 / 526.255 (1) | 532.407 / 532.407 (1) | N/A |
| triangles | 501 | LM | T ≥ 4998.992 | S | N/A |
| triangles | 501 | DogLeg | T ≥ 4999.145 | S | N/A |

C2 after M8 steps 2–5 (`plans/m8-frame-budget`, 2026-11-04), same command,
machine and cell conventions (median / worst in ms, five samples; every
position finishes inside its limit; the verdict is on the median, the
acceptance on the worst sample). Load average at the run: 5.1 (1 min), 12.2
(5 min), 14.2 (15 min), from other processes on the machine; a second run
at 3.7 (1 min) put every 500-entity cell within 0.7 ms of these. **Under 4 ms:
the 98- and 99-entity cells, the 249-entity triangle cells and the
248-entity rectangle cells (rectangles 248 DogLeg drag's worst sample is
5.04). Over: the 248-entity slot cells (4.2–5.4) and every 500-entity cell:
edit 6.4–11.3, first drag step 12.1–17.4, bisected frame 29.5–30.0. The
500-entity acceptance (edit and first drag step, each ≤ 4 ms) is not met.**

| Family | Actual entities | Algorithm | Static | Edit | First drag step |
|---|---:|---|---:|---:|---:|
| rectangles | 98 | LM | 1.104 / 1.948 | 0.924 / 0.967 | 0.929 / 0.969 |
| rectangles | 98 | DogLeg | 0.986 / 1.136 | 0.888 / 0.945 | 0.953 / 1.003 |
| rectangles | 248 | LM | 3.862 / 4.038 | 2.945 / 3.225 | 3.926 / 4.423 |
| rectangles | 248 | DogLeg | 3.184 / 3.642 | 2.930 / 3.103 | 3.891 / 5.040 |
| rectangles | 498 | LM | 9.860 / 10.456 | 7.322 / 7.389 | 17.357 / 17.658 |
| rectangles | 498 | DogLeg | 8.093 / 8.340 | 7.159 / 7.364 | 17.265 / 17.382 |
| slots | 98 | LM | 1.750 / 1.904 | 1.434 / 1.535 | 1.319 / 1.657 |
| slots | 98 | DogLeg | 1.813 / 2.092 | 1.382 / 1.463 | 1.307 / 1.430 |
| slots | 248 | LM | 5.348 / 6.131 | 4.172 / 4.326 | 4.420 / 4.610 |
| slots | 248 | DogLeg | 5.369 / 5.528 | 4.350 / 4.374 | 4.423 / 4.693 |
| slots | 498 | LM | 15.327 / 15.745 | 11.277 / 11.773 | 12.072 / 13.612 |
| slots | 498 | DogLeg | 13.737 / 14.432 | 11.382 / 11.724 | 12.231 / 12.531 |
| triangles | 99 | LM | 0.827 / 0.935 | 0.721 / 0.860 | N/A |
| triangles | 99 | DogLeg | 0.797 / 0.924 | 0.740 / 0.779 | N/A |
| triangles | 249 | LM | 2.868 / 3.015 | 2.343 / 2.644 | N/A |
| triangles | 249 | DogLeg | 2.679 / 2.786 | 2.430 / 2.466 | N/A |
| triangles | 501 | LM | 6.887 / 7.875 | 6.351 / 6.450 | N/A |
| triangles | 501 | DogLeg | 7.198 / 7.455 | 6.379 / 6.404 | N/A |

The bisected frame is the slot grid's `bisect` cell (M7, below): 29.99 /
30.81 ms LM and 29.51 / 30.05 ms DogLeg, against 12.1 / 12.2 ms for the plain
first drag step.

Against M6's table, the 500-entity edits went 1.6–2.0× faster (rectangles
11.8 → 7.3 ms, slots 22.5 → 11.3, triangles 11.7 → 6.4) and the first drag
step 1.2–1.7× (rectangles 20.1 → 17.4, slots 19.9 → 12.1); no cell got
slower, and the 248-entity slot drag went 7.1 → 4.4. The 4 ms target is
still missed at 500 entities by 1.6× (triangles edit) to 2.8× (slots edit),
the drag step by 4.3× on rectangles (17.4 ms) and the bisected frame by 7.5×.

**Profile and the choice (M8 step 5; self time, `perf report
--no-children`, the 500-entity cells, LM and DogLeg).** Steps 2–4 removed the
costs step 1 measured: the staged rank computed three times per frame
(`staged_checked`, 30–55 % of a frame), the per-part extent measure
(`scale_at`, 12–24 %) and the tolerances made six or seven times per frame
(`Tolerances::at`, 15–28 %). Step 5 removed the one source it found above
10 %, the per-part split of the frame (12 % of the slot edit). What is left has no
single source above 9 %. Most of it is the solver's per-iteration work:
`unit_row` 7–8 %, `evaluate_within` 6 %, `entity_params` 3–5 %, `BTreeMap`
lookups about 6 %, `malloc`/`free` about 8 %. Two more are the staged solve's
`Splitter::split` (the joins of each part's frame, 8.5 % on the rectangle
edit; it is called only from the solve) and the diagnosis's cut of the
placed sketch into parts, `stage::cut` (6 %). Of the three options the plan named, the choice is:

- **Rigid bodies in the remainder (A)**, the next M8 plan. It shrinks the rows
  and parameters every solve touches, the per-iteration work above, and so
  reaches all four cell kinds: edit, first drag step, the bisect's tries and
  the static solve.
- A **diagnosis cache (S2)** is not chosen. It reaches only the diagnosis's
  own share, the cut and the recompile (`stage::cut` is 6 % on the rectangle
  edit; the split is solve cost and a cache of the diagnosis leaves it), and
  not the bisected frame at all:
  its tries carry no diagnostics, so the solve is most of that cell (30 ms
  against the 12 ms plain drag it extends).
- A **driver split of the remainder** is not chosen. It moves the solve of
  the rest out of a drag frame only; the edits (6–11 ms) and each bisect try
  of the edited stage keep the full per-iteration cost.

The gain of A is not measured; its plan's own profile measures it before any
cut. (`docs/plans/m8-frame-budget.md`, Open questions: settled by the agent.)

The 98- and 99-entity cells are 11–15 times faster than the M5 baseline and
under 4 ms; the 498-entity cells went from not finishing in 5 s to 6–30 ms.
After M6, the 500-entity cells were attributed to the per-part extent
measure, the diagnosis's recompile and re-cut of the placed sketch, and the
repeated linear scans; M8's step-1 profile confirmed the first and the
rank and tolerance repeats, and steps 2–5 removed them. The table above is
the state now.

**A bisected frame (M7, 2026-11-04)**, same command and machine: the 498-entity
slot grid's `bisect` cell is its drag with the middle slot's left arc let
loose of its tangent lines and its start pulled a quarter turn about its
centre, so the first frame moves the arc's sweep past `MAX_SWEEP_STEP` and is
bisected (the worker fails if it is not). Median / worst, five samples:
LM 42.3 / 42.7 ms, DogLeg 40.5 / 42.7 ms, against 21.5 / 22.2 and
20.8 / 21.0 for the plain first drag step: the extra tries roughly double
the frame. Over 4 ms like every 500-entity cell; after M8 steps 2–5 it is
29.5–30.0 ms (the table above), still over.

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
  bounded deletion filter with local certificates, never a minimum. Cycle 2
  measures the 256-re-solve bound again on its sizes.
