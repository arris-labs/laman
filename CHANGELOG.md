# Changelog

What a user of the `laman` crate or the `laman` Python package can do,
release by release. Written in the commit that makes the change, under
`Unreleased` (`.agents/rules/git.md` §The changelog).

## Unreleased

- Large sketches solve far faster: each connected part of a sketch is cut
  into stages solved in dependency order, so a 100-entity sketch solves,
  diagnostics included, in about 0.5–1 ms and a 500-entity edit in 2–4 ms
  (a first drag step in about 4 ms on slots and about 6 ms on a rectangle
  chain; a bisected drag frame 2.5–4 ms a try), where they took tens of
  milliseconds and seconds. Dragging a long chain is about twice as fast or
  better.
- An edit keeps the exact bits of every coordinate it cannot reach: a
  re-solve after changing one dimension moves only that dimension's stage
  and what depends on it.
- Solutions may differ from 0.1.0's in their last bits (the same answers
  within tolerance, reached by other steps); DoF, the span of the free
  directions and redundant constraints are unchanged. A conflict set can
  now keep a constraint 0.1.0 wrongly left out of it, where the rest held
  together. On some contradictory sketches the conflict analysis now names
  a set where 0.1.0's was inconclusive (`NotConverged`), under DogLeg too:
  its analysis finds a conflict set wherever Levenberg-Marquardt's does.
- `SolverOptions::max_iterations` and `max_evaluations` count a sketch
  solved by stages in whole-solve units: a stage's step spends its share
  of the parameters, so the defaults bound the same work. A staged solve
  that fails is solved again whole, so a failing sketch can spend up to
  twice the caps.

- A circle's radius is never negative: a solve or a drag frame that would
  need one ends with the radius at 0 (a conflict, or under-constrained),
  so every solution applies back to its sketch.
- A conflict set no longer includes a false member or misses a true one
  where a subset of the constraints held together at a stationary point
  that was not a solution: each test of the analysis is solved again by the
  staged solve, falling back to the whole one, so both paths agree.

- A drag frame that would move an arc's sweep by more than an eighth of a
  turn is bisected toward the last target, as a flip is, without naming a
  flip: a frame that carried an arc's end past half a turn used to be read
  the short way round and land where many small frames would have been
  refused. A static solve is unchanged. The bound is the new constant
  `laman::model::tolerance::MAX_SWEEP_STEP`.

### Breaking

- `Diagnostics` gains a field, `bisection: Option<Bisection>`, set on a drag
  frame that was halved toward its target (`Bisection { reached, halvings }`)
  and `None` otherwise (a frame refused because every try flipped or jumped carries it too, with `halvings` 0 when `max_bisections` is 0). `Diagnostics` is not `#[non_exhaustive]`, so a
  struct literal needs `bisection: None`; the Python `Diagnostics` has a
  `bisection` attribute (`laman.Bisection` or `None`) and `Solution` JSON
  gains an optional `diagnostics.bisection`.

## 0.1.0 — 2026-11-03

- Python: abi3 wheels for Linux x86_64 and aarch64 (glibc ≥ 2.17),
  macOS arm64 and x86_64, and Windows x86_64; Python 3.10 and newer.
  A source distribution is available for other build environments.

- Python: `laman.Sketch` is built by hand or loaded from JSON, solved,
  applied and dragged (`DragSession`), with `Solution` and every diagnostic
  readable, `laman.check` for the independent check and `laman.debug` for
  text and SVG dumps and digests (tooling). Refusals are `ModelError`
  subclasses (a `ValueError`) naming your ids. Typed stubs ship with the
  package.

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
  last solution, which `Sketch::apply` takes on the unedited sketch (except,
  for now, when a degenerate sketch ends a frame with a radius rounded just
  below zero, which `apply` refuses as out of range). A
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
