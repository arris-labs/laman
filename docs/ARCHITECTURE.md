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
| `laman-debug` | text and SVG dumps, the fixture loader, `proptest` strategies, the digest, benchmarks | no (`publish = false`) | `laman`, `laman-check` |
| `laman-py` | the PyO3 binding, `laman` and `laman.check` in Python | PyPI `laman`, never crates.io | `laman`, `laman-check` |

`laman-debug` is only ever a dev-dependency. `fuzz/` sits outside the
workspace and builds on nightly under `cargo fuzz`.

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
| `system` | a sketch compiled for one solve: the dense vector of free parameters, the equation list in order, the components |
| `solve` | Levenberg–Marquardt and DogLeg over a `System`, warm start, priorities |
| `diagnose` | rank, DoF per entity, free directions, redundant and conflicting sets, branches |
| `drag` | `DragSession` |
| `rigidity` | the generic rigidity count (the 0.0.1 API), later the combinatorial pre-pass of cycle 2's decomposition |

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
and refuses one made from another revision (`ModelError::Stale`), so a
solution cannot land on a sketch it was not solved for.

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
   Cycle 2 refines this into rigid clusters (SEED.md §6).
3. **Solve** each component: damped least squares with analytic Jacobians,
   Levenberg–Marquardt by default and DogLeg by `SolverOptions`, dense
   `nalgebra` matrices. Hard caps on iterations and on function
   evaluations; convergence when every residual is within the equation's
   tolerance. A component that hits a cap returns its best iterate and is
   reported `NotConverged`, never an error and never a hang.
4. **Diagnose** at the solution: the Jacobian of all equations, rank by
   column-pivoted QR with the scaling and tie order of ADR-0003, DoF, free
   directions, redundant and conflicting sets, degenerate entities and branch
   flips (DATA-MODEL.md §Diagnostics).

**Priorities**: every equation has one. Constraints are `Required` (the
default); a drag target is `Goal`. Required equations are solved first; a
goal is solved in the null space the required equations leave, so a goal
can never push a required constraint off. Diagnostics never blame a goal
for a conflict.

**Least change**: among the solutions of an under-constrained component,
the solver takes the one nearest the starting point: the damped step is
the minimum-norm one, and a component with nothing to change takes no
step, so its bits are the start's bits. A solve of an already solved
sketch returns the same bits (SEED.md §8).

The M2 numerical contract is fixed by [ADR-0002](adr/0002-regularize-structural-tangent-contacts.md)
(contact regularization) and [ADR-0003](adr/0003-scale-relative-tolerances-and-rank-order.md)
(component-relative lengths with an absolute floor, dimensionless rank and
angular thresholds, and deterministic rank/null-space ordering). The compiler and checker use the shared model
`Tolerances` policy, frozen from input geometry and dimensions. Components
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
- What no equation holds is **observed**: each arc's sweep, the
  orientation (sign of the cross product) of parallel lines and of each
  angle's lines. The solution records them; the next solve and every drag
  step compare against the last solution, and a change is a **flip**,
  named in diagnostics.
- A drag step that would flip is bisected toward the last good target
  (continuation, at most `SolverOptions::max_bisections` halvings); one
  still flipping is returned with the flip named, so the caller decides.

`⚠ OPEN:` how branch preservation is defined and tested beyond these
observations (SEED.md §10.8). Closed by an ADR in M3.

## Drag sessions

```rust
let mut drag = sk.drag(point, &options)?;    // ModelError for a non-point or a fixed point
let step = drag.to([x, y]);                  // one frame: a Solution
let done = drag.finish();                    // the last Solution
sk.apply(&done)?;
```

A `DragSession` owns its compiled `System` (it does not borrow the sketch)
and the last solution. Each `to` adds the target as a `Goal` pair of
equations on the dragged point, solves from the previous step (warm
start, least change), checks branches, and returns a full `Solution`.
Geometry with no equation path to the dragged point is in another
component and is not touched: its bits stay the bits it started with. The
frame budget a step must meet is ROADMAP.md's.

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

Same input, same bits, on Linux, macOS, Windows (x86_64 and aarch64) and
wasm32 (SEED.md §4, §9):

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

The **digest** is SHA-256 over a solution's coordinates (little-endian
`f64` bits in compiled order), status and diagnostics. Until `Solution`
exists it covers the sketch's serde text and the checker's report at a
fixture's expected coordinates (floats in the report by their bits). CI computes the
digests of the fixture corpus on Linux natively and on `wasm32-wasip1`
under wasmtime, and fails if any differ; `laman-debug` holds the blessed
digests. A change that moves a solution's bits re-blesses them in the same
commit, saying why (`.agents/review.md`).

## wasm32

`laman` and `laman-check` build for `wasm32-unknown-unknown` with no
features and no glue: no file system, clock, environment or threads. The
gate builds it on every commit; CI runs the digest corpus on
`wasm32-wasip1`. JS bindings are not in v1 (SEED.md §9). `laman-py` is
empty on wasm32.

## Serde

The `serde` feature (on by default) gives `Sketch` its serde form,
`Solution` and `Report` theirs. The form, its fields and its versioning
are in DATA-MODEL.md §The serde form.

## The Python binding

`laman-py` is a 1:1 layer, abi3 for Python ≥ 3.10, typed stubs held by
`mypy.stubtest`. A `laman.Sketch` has the Rust builder methods, ids are
small handle objects with `.id`, keyword arguments carry what Rust spells
as separate methods (`sk.point(0, 0, fixed=True, tag="origin")`).
`ModelError` variants are exception classes under `laman.ModelError`
(itself a `ValueError`); statuses are a Python `enum`. The binding computes
nothing.

## Errors

| Type | When | Carries |
|---|---|---|
| `ModelError` | a mutation or a load that would make the sketch malformed | the ids involved and what was wrong |
| `RigidityError` | `rigidity` given a bad bar | the bar index and point |

No other error type: solving, dragging and checking always return a
value.

## Open questions

- `⚠ OPEN:` branch preservation's definition and test (§Branches). ADR in
  M3.
- `⚠ OPEN:` whether the minimal conflict set is exact or a bounded
  approximation on large sketches (DATA-MODEL.md §Diagnostics). ADR in M2,
  measured on cycle 2's sizes.
