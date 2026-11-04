# Laman glossary

The words this project uses for its own concepts, one meaning each. A term
here is used as written in code, docs, plans and commits; the words under
_Avoid_ are the synonyms it replaces. Definitions say what a thing **is**;
how it works lives in the design doc named beside it. Standard geometry
and numerical vocabulary (Jacobian, rank, Levenberg–Marquardt) means what
the literature means.

## The model

**Sketch**:
The arena that owns every parameter, entity and constraint of one 2D
drawing, and the unit a solve runs on (SEED.md §5).
_Avoid_: system, document, model (a model is the consumer's)

**Parameter**:
One real unknown of a sketch (a coordinate, a radius), free or fixed.
_Avoid_: variable, unknown

**Entity**:
A geometric object of a sketch built from parameters: a point, line
segment, circle or arc.
_Avoid_: primitive, element, shape

**Handle**:
A typed, frozen reference to one entity of a sketch by its id: Rust's
`Point`, `Line`, `Circle`, `Arc` and the Python classes of those names. It
carries no coordinates and is checked against the sketch at every use, so
a handle of another sketch or of a removed entity is refused with a typed
error; a loaded sketch hands them out by id (`Sketch.point_handle`, ...).
_Avoid_: pointer, object, proxy

**Ground**:
A fixed parameter, or a point with both coordinates fixed: not an unknown,
never blamed (DATA-MODEL.md §Entities). The `fix` constraint is not
ground: it is an equation.
_Avoid_: anchor, locked

**Construction**:
A flag on an entity: it solves like any other and is left out of profiles
(ARCHITECTURE.md §The sketch and its ids).
_Avoid_: helper geometry, reference geometry

**Constraint**:
A relation the solve must make true between entities, written as one or
more residual equations. A dimension is a constraint with a value.
_Avoid_: rule, relation

**Intrinsic equation**:
An equation an entity carries by its definition, not a constraint the
caller added: an arc's end on its start's circle (ADR-0001).
_Avoid_: internal constraint, implicit constraint

**Sense**:
The root of a multi-root constraint chosen from the drawing when it is
added and stored with it: the side of a tangent, internal or external
contact (ARCHITECTURE.md §Branches).
_Avoid_: orientation (an observed orientation is not stored), flag

**Dimension**:
A constraint carrying a caller's value: distance, point–line distance,
radius, angle.
_Avoid_: measurement, driving constraint

**Tag**:
An optional caller string on an entity or constraint, returned in every
diagnostic that names it.
_Avoid_: label, name

**Payload**:
An opaque caller value on an entity or constraint that laman stores and
returns, never reads.
_Avoid_: user data, metadata

## Solving

**Solution**:
What `solve` always returns: coordinates (best effort when the sketch is
unsatisfiable), a status and diagnostics. Never an error.
_Avoid_: result (an `Err` is a result)

**Origin**:
Where a diagnosed equation comes from: a constraint, or an arc's own
equation. Named by id, kind and tag.

**Status**:
The verdict on a sketch: fully constrained, under-constrained,
unsatisfiable or not converged. Redundancy is a diagnostic, not a separate
status. A status is never an error (DATA-MODEL.md §Diagnostics).

**Malformed**:
A sketch description a builder or loader refuses with a typed error:
a dangling id, a constraint on the wrong kind of entity, a non-finite
input. A constructed sketch is well-formed; `solve` returns a Solution.
_Avoid_: invalid sketch (an unsatisfiable sketch is well-formed)

**DoF**:
Degrees of freedom left in a sketch or an entity: geometric freedom at
regular configurations and supported C1 contacts, an infinitesimal estimate
at other singularities (ADR-0002).
_Avoid_: freedom count, mobility

**Geometric freedom**:
The dimension of locally possible finite motions satisfying the geometry.
At a singular contact it can be smaller than infinitesimal freedom.

**Infinitesimal freedom**:
The nullity of the original constraint Jacobian, which describes motions
allowed to first order only.

**Contact regularization**:
Replacing a structurally recognized tangent equation by perpendicular
radius contact, retaining membership, sense and source provenance
(ADR-0002). It recovers geometric freedom for C1 contacts.

**Component scale**:
The translation-independent extent of one structural component, including
its radii and dimensions; sets its length tolerance (ADR-0003). The solver
freezes it from its input. The checker measures it at the coordinates it
checks, never at the guesses (ADR-0004).

**Free direction**:
A motion of the sketch the constraints allow at the solution, a null-space
vector of the Jacobian mapped to the entities it moves.

**Degenerate**:
An entity whose geometry collapsed at the solution: a zero-length line, a
zero radius, an arc whose ends meet. A diagnostic, never an error.

**Redundant**:
A constraint the others already imply at the solution: removing it frees
no DoF. Of two duplicates the later is named (ADR-0005). A warning, never
a conflict.
_Avoid_: duplicate, over-constraint

**Conflicting**:
A set of constraints that cannot hold together; the conflict set is an
inclusion-minimal one named to blame: every member has a removal witness,
and its unsatisfiability is a local certificate (ADR-0005).
_Avoid_: inconsistent, failed

**Removal witness**:
A converged re-solve of a conflict set with one member removed: the proof
that the member is needed in the set.

**Conflict certificate**:
A least-squares point, outside tolerance, at which the weighted residual
is a left null vector of the Jacobian to `STATIONARY`, its gradient
projected where a radius is held at 0 (ADR-0010). No first-order motion
the radius bound allows reduces the residual there. It is local evidence of a conflict,
never a proof that no solution exists anywhere (ADR-0005).

**Radius bound**:
A circle's radius is never below 0, held inside both engines by a
projected step: a radius on the bound that descent presses below takes no
step, and a step that would cross 0 is cut there (ADR-0010).
_Avoid_: radius clamp

**Fundamental circuit**:
In a rigid distance framework with one extra distance, the extra edge and
every edge whose removal leaves the rest independent: the smallest
over-braced set holding the extra edge. Generically the only minimal
conflict set when that distance is stretched a little (Laman 1970; pebble
game, Jacobs and Hendrickson 1997). A test oracle computed from the graph
alone (`laman-debug` test support), never a solver output.

**Component**:
The free parameters connected through shared equations, solved as one
problem (ARCHITECTURE.md §Solve).
_Avoid_: subsystem (until it means a rigid cluster), island

**Stage**:
One of a component's square parts: a strongly connected set of required
equations with the free parameters they fix, from the Dulmage–Mendelsohn
decomposition of the compiled system. Stages solve in dependency order,
so an edit never moves a stage before it (ideas/c2-decomposition).
_Avoid_: block, cluster

**Head**:
A component's over-determined part, solved first: more equations than the
parameters they fix, so every redundancy and conflict lives here. It
reads nothing outside itself.

**Remainder**:
A component's under-determined part, solved after its stages with least
change: where every freedom, and every drag, lives.

**Rigid cluster**:
A part of a sketch that is rigid by itself, as a pebble game
(`laman::rigidity`) would find it, solved as one rigid body. Not built:
C2 decomposes into stages, and rigid clusters join only if a remainder
misses the frame budget (ideas/c2-decomposition, M8).
_Avoid_: block, group

**Generic rigidity**:
Rigidity of a bar framework for points in general position, decided by
Laman's condition alone (`laman::rigidity`).

**Warm start**:
A solve that starts from the previous solution's parameters.

**Drag session**:
A sequence of solves that moves a target point while the rest of the
sketch follows, by priority, with least change.
_Avoid_: drag solve, interactive solve

**Goal**:
The priority of a drag target's equations: solved lexicographically after
the required ones, only in the null space they leave (least squares there,
of least motion), so it never pushes a required constraint off; met, or
stationary when unreachable; never seen by a diagnostic or blamed in a
conflict.
_Avoid_: soft constraint, weight

**Observation**:
What no equation holds, recorded per subject at a solution: an arc's
tracked sweep, a circle's radius, a line pair's orientation under
`parallel`, `perpendicular` or `angle`. Its branch is known only beyond
a margin, and carried inside it (ADR-0006).
_Avoid_: orientation flag, state

**Flip**:
A change of branch between solves: an observation known on both sides
whose branch differs from the last solution's (the guesses', for a
static solve). Named in diagnostics with the quantity before and after.

**Bisection**:
Halving a drag step that flips, or moves an arc's tracked sweep past
`MAX_SWEEP_STEP` (a sweep jump), toward the last good target, at most
`max_bisections` times, so the step keeps its branch (ADR-0006).
_Avoid_: backtracking, line search

**Branch**:
Which of a constraint system's discrete solutions a sketch sits on (which
side of a line a tangent circle is on). Kept across edits and drags.
_Avoid_: configuration, mode

## Verification

**Checker**:
`laman-check`: re-evaluates every constraint from solved coordinates with
code that shares nothing with the solver's residuals.
_Avoid_: validator, verifier

**Fixture**:
A sketch with its expected values (coordinates, DoF, conflict sets),
known analytically or by construction, committed under `tests/fixtures/`.
_Avoid_: test case, sample

**Digest**:
A hash of a solution's bits, compared between native and wasm32 to hold
determinism.
_Avoid_: checksum, fingerprint

**Equation**:
One residual `f = 0` that a constraint or an entity contributes, with its
unit and its analytic gradient over the parameters it reads
(`laman::residual`, DATA-MODEL.md §Constraints).
_Avoid_: row, residual (as a noun for the whole thing)

**Local scale**:
The shortest length among the lines, radii and point pairs an equation
reads: the length its gradient varies over, which sets the
finite-difference step (`laman_debug::fd`).
_Avoid_: characteristic length, step size

**Oracle**:
Another solver run out of process under `tools/oracle/`: SolveSpace or
PlaneGCS when analytic expectations run out, and the comparison run of the
Rust field (`sketch-solver`, `acs-solver`, `ezpz`) on the C1 fixtures;
compared on status, DoF and conflict sets, not coordinates.
_Avoid_: reference solver, ground truth

**Gate**:
The checks every commit passes: `tools/gate.sh`, run by the pre-commit
hook and by CI.
_Avoid_: CI (CI runs the gate and more)

**Frame budget**:
The time an interactive solve must fit in, stated per sketch size in the
roadmap.
