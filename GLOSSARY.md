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

**Constraint**:
A relation the solve must make true between entities, written as one or
more residual equations. A dimension is a constraint with a value.
_Avoid_: rule, relation

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

**Status**:
The verdict on a sketch: fully constrained, under-constrained,
over-constrained but consistent, or unsatisfiable. A status, never an
error (SEED.md §9).

**Malformed**:
A sketch `solve` refuses with `Err`: a dangling id, a constraint on the
wrong kind of entity, a non-finite input. The only errors.
_Avoid_: invalid sketch (an unsatisfiable sketch is well-formed)

**DoF**:
Degrees of freedom left in a sketch or an entity, counted from the rank of
the constraint Jacobian at the solution.
_Avoid_: freedom count, mobility

**Redundant**:
A constraint the others already imply at the solution: removing it loses
no DoF. A warning, never a conflict.
_Avoid_: duplicate, over-constraint

**Conflicting**:
A set of constraints that cannot hold together; the conflict set is the
minimal one named to blame.
_Avoid_: inconsistent, failed

**Generic rigidity**:
Rigidity of a bar framework for points in general position, decided by
Laman's condition alone (`laman::rigidity`).

**Warm start**:
A solve that starts from the previous solution's parameters.

**Drag session**:
A sequence of solves that moves a target point while the rest of the
sketch follows, by priority, with least change.
_Avoid_: drag solve, interactive solve

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

**Oracle**:
Another solver (SolveSpace, PlaneGCS) run out of process when analytic
expectations run out; compared on DoF and conflict sets, not coordinates.
_Avoid_: reference solver, ground truth

**Gate**:
The checks every commit passes: `tools/gate.sh`, run by the pre-commit
hook and by CI.
_Avoid_: CI (CI runs the gate and more)

**Frame budget**:
The time an interactive solve must fit in, stated per sketch size in the
roadmap.
