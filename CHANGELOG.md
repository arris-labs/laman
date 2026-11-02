# Changelog

What a user of the `laman` crate or the `laman` Python package can do,
release by release. Written in the commit that makes the change, under
`Unreleased` (`.agents/rules/git.md` §The changelog).

## Unreleased

- `laman::solution`: `Sketch::solve(&SolverOptions)` returns a `Solution`
  (coordinates, `Status`, `Diagnostics`) with Levenberg-Marquardt or
  DogLeg under hard caps; under-constrained, over-constrained and
  unconverged sketches are statuses, never errors. Diagnostics give the
  degrees of freedom (total, per entity, free directions), redundant
  constraints, conflict sets and degenerate entities, naming your ids,
  kinds and tags. `Sketch::apply(&Solution)` writes the answer back and
  refuses a solution solved from another revision or other contents
  (`ModelError::Stale`, `ForeignSolution`), an incomplete or non-finite
  one, or one that moves a fixed parameter (`IncompleteSolution`,
  `FixedMoved`). Solutions save to and load from JSON.
  `ModelError` gains these four variants (it is non-exhaustive).

- `laman::drag`: `Sketch::drag(point, &SolverOptions)` opens a
  `DragSession` on a point; `to([x, y])` returns one frame as a full
  `Solution`, solved from the previous frame with the target held as a
  goal that never pushes a constraint off (a target the sketch makes
  unreachable ends at the nearest reachable place); `finish()` returns the
  last solution, which `Sketch::apply` takes on the unedited sketch. A
  target that is not finite returns the last frame as `NotConverged`.
  Dragging a point fixed in both coordinates is refused with the new
  `ModelError::FixedPoint`; a point with one fixed coordinate drags along
  the other. A frame whose solve would flip an arc, a circle or a line
  pair is halved toward the last target (`SolverOptions::max_bisections`,
  default 8) and the first halving that keeps every branch is the frame; if
  none does, the last solution comes back with the flip named and the
  session stays on its branch.

- `Diagnostics::flips`: a solve names each arc, circle and `parallel`,
  `perpendicular` or `angle` constraint that ended on a different branch
  than the sketch was drawn on (an arc turned the other way round, a
  circle's radius changed sign, a line pair swapped orientation), with the
  observed quantity before and after (`Flip`, `FlipSubject`). Empty when
  every branch is kept; absent from a solution's JSON then.

- Checking uses translation-independent, component-relative length tolerances,
  with an absolute floor, measured at the coordinates being checked: neither
  unrelated geometry nor the sketch's starting guesses can loosen acceptance.
  Rust callers can inspect the policy through `model::tolerance::Tolerances`
  (`new` at the sketch's coordinates, `at` at any others).

- `laman::model`: build a sketch of points, lines, circles and arcs with
  ids, tags, fixed parameters, construction flags and opaque payloads, and
  add all 18 constraints between them. A malformed request (an unknown id,
  the wrong kind of entity, a repeated entity, a non-finite or out-of-range
  value, removing an entity a constraint uses) is refused with a
  `ModelError` naming your ids. Tangent and circle-contact constraints take
  their side from the drawing.
- `laman::residual`: evaluate each constraint's equations, and an arc's own
  equation, at any coordinates: the value, its unit (length or
  dimensionless) and its gradient over the parameters it reads. A
  coordinate you do not supply is read from the sketch; an undefined point
  (a zero-length line) gives a finite value flagged degenerate, never NaN.
- A sketch saves to and loads from JSON (feature `serde`, on by default),
  versioned; a malformed file is refused with an error naming the id.

## 0.0.1 — 2026-11-01

- `laman::rigidity(points, bars)` (Python: `laman.rigidity`): the generic
  rigidity of a bar framework in the plane, by Laman's theorem. Returns the
  remaining internal degrees of freedom and which bars are independent and
  which redundant, by index. A bar naming a point out of range, or the same
  point twice, is refused with a `RigidityError` naming the bar.
