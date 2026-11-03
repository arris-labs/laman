# tools/oracle

Other Rust constraint solvers, run out of process on laman's ten C1 sketches
(the four fixtures and their six twins under `tests/fixtures/c1/`), compared
on status, DoF and conflict sets, never coordinates (`docs/plans/m4-oracle.md`,
SEED.md §7). Its own Cargo workspace root like `fuzz/`: not in the gate, not in
CI, built over the network and run by hand. It depends on `laman` for the
model and on `laman-check`, never on laman's solver (`tools/check-layers.sh`).

```sh
cargo build --manifest-path tools/oracle/Cargo.toml
tools/oracle/target/debug/laman-oracle hello <sketch-solver|acs-solver|ezpz>
tools/oracle/run.sh [solver ...]   # every fixture, one process each, under ORACLE_TIMEOUT (60 s)
```

`run.sh` writes `results/<solver>/<fixture>.json` (the report of `src/report.rs`);
a panic, signal or hang in a solver becomes a `crashed` or `timeout` report.
`cargo test --manifest-path tools/oracle/Cargo.toml` holds the report type.

## The solvers, pinned

| Solver | Version | Licence | Entry point used |
|---|---|---|---|
| `sketch-solver` (NeoSCAD) | `=0.1.0` | MIT OR Apache-2.0 | `Sketch::new`, `add`, `solve` -> `Solution` |
| `acs-solver` | `=0.1.19` | MIT | `acs::sketch_solve::solve_sketch_json` (the JSON contract of USAGE.md) |
| `ezpz` | `=0.2.29` | MIT | `solve_analysis(&[ConstraintRequest], guesses, Config)` |

## What each API exposes

`hello` solves `o` fixed at the origin and `a` at distance 5 from it, guessed at
(3, 1); then the same with a second `distance` of 5 (redundant) and of 6
(conflicting). Observed:

| | status | DoF | redundant set | conflict set |
|---|---|---|---|---|
| `sketch-solver` | `Solved` / `NotConverged` | `dof` (1) and `free` coordinates | `redundant`: the later constraint, with the `with` it depends on | `conflicts`: the later constraint only (the sources it contradicts are in `with`) |
| `acs-solver` | `"converged"` / `"failed"` | `dof` (1) | `redundant`: `["k2"]`, latest first | `conflicting`: `["k1","k2"]`, both sides |
| `ezpz` | `is_satisfied()`; `Err(FailureOutcome)` for a solve that errors | none: only `underconstrained()` variable ids (`[2, 3]`), so DoF is `null` | none | none: `unsatisfied()` constraint indices (`[2, 3]`, the two distances) |

Each solver reports ids in its own terms; the adapters map them back to
laman's tags (an untagged constraint by `#id`). A solver that names no set
leaves that field `null` in its report, never an empty set.

## The expressibility matrix

The kinds the ten C1 sketches use (`sketch.json` of each), against each
solver's API. "direct" is one primitive of the solver; "built" is made from
the solver's primitives with the same equation count, so DoF still compares
(the open question in the plan: built only where the equivalence is exact);
"unsupported" is reported as such, never approximated.

| laman kind | `sketch-solver` | `acs-solver` | `ezpz` |
|---|---|---|---|
| point; fixed point (`fixed: [x, y]`) | direct; `Fix { at: None }` (2 eq) | direct; `"fixed": true` | direct; two `Fixed(id, value)` |
| line; construction line | direct; construction is a flag (`set_construction`) | direct (`isConstruction` passes through); a construction line constrains like any line | direct (`DatumLineSegment`); no flag, nothing to set |
| circle (centre + radius) | direct (radius is an unknown) | direct (`radius` is a variable) | direct (`DatumCircle`, radius a `DatumDistance`) |
| arc as centre + endpoints | direct; adds one equation (`\|s-c\| = \|e-c\|`), as laman's | direct but parametrised: radius and two angles are the variables, endpoints derived: 5 DoF free, laman's 6 coordinates - 1 equation = 5, same count | direct (`DatumCircularArc`) with the `Arc` constraint (the same one equation) |
| `horizontal`, `vertical` (line or two points) | direct (`Pair`) | direct | direct (on a `DatumLineSegment`; two points via a segment) |
| `distance` (point-point) | direct (`Along::Direct`) | direct | direct (`Distance`) |
| `radius` (circle) | direct (`Radius`) | direct | direct (`CircleRadius`) |
| `point_on_circle` | direct (`On`) | direct (`on`) | built: `DistanceVar(point, centre, radius)`, one equation, exact |
| `tangent` line-circle, `sense` +-1 | direct (`Tangent`; the side comes from the drawing, `sense` is not passed) | direct (`tangent`; segment form, span form for an arc) | direct (`LineTangentToCircle`, `LineSide`) for a circle |
| `tangent` line-arc (slotted plate) | direct (`Tangent`) | direct (`tangent` line-arc: at a shared endpoint, perpendicular to the radius) | built: a `DatumCircle` whose centre is the arc's centre and whose radius is a new `DatumDistance` tied by `DistanceVar(start, centre, r)`: one new unknown, one new equation, exact |
| `perpendicular` | direct | direct | direct (`LinesAtAngle(Perpendicular)`) |
| `equal_length` | direct (`Equal`) | direct (`equal`) | direct (`LinesEqualLength`) |
| `equal_radius` (circles, arcs) | direct (`Equal`) | direct (`equal`) | direct for circles (`ScalarEqual` of the radii); for arcs through the built circles above |
| `midpoint` (point, line) | direct (`Midpoint`, 2 eq) | direct (`midpoint`) | direct (`Midpoint`) |
| `symmetric` about a line | direct (`Symmetric`, `about` a line) | direct (`mirror`, axis = the line's extension) | direct (`Symmetric`) |
| `point_line_distance` | direct (`Distance`, point-line, on the drawn side) | direct (`distance`, `extension: true`: the perpendicular distance to the infinite line) | direct (`PointLineDistance`, perpendicular) |
| `angle` between two lines | direct (`Angle`, signed counter-clockwise degrees) | direct (`angle`, radians; `reverseA`/`reverseB` choose the rays) | direct (`LinesAtAngle(Other)`) |

No kind of the C1 sketches is `unsupported` by any of the three, so no
approximation beyond the two "built" rows is needed. Whether each translation
means the same thing is not argued here: step 3 onward checks the solver's own
residuals at the fixture's expected coordinates, and a translation that fails
that reports `error`, never numbers.

## What the reports can name

| | status | DoF | redundant | conflicts |
|---|---|---|---|---|
| `sketch-solver` | `Solved` -> solved/under; `NotConverged` -> conflicting or unsolved (ambiguous without a conflict set) | yes | yes, with the supporting `with` | yes, the later constraint plus its `with` |
| `acs-solver` | `converged`/`failed` the same way | yes | yes (minimal, latest first) | yes (every constraint in the dependency) |
| `ezpz` | satisfied or not only | no | no | no: the unsatisfied constraints, not a minimal set |

`ezpz` stays a row with that finding: it compares on satisfaction and on which
variables it calls underconstrained, nothing more.

## Findings that shape the adapters

- `ezpz` has no `point_on_circle`, and a circle-arc tangent needs a circle:
  both are built, as above, with the same equation count.
- `ezpz` takes one guess per variable in `Vec<(Id, f64)>` and infers a
  `LineSide`/`CircleSide` of `Undefined` from the guesses, as laman does from
  the drawing; the fixtures' guesses are on the expected branch.
- `acs-solver`'s `conflicting` lists both sides of a dependency, laman's
  `conflicts` names a minimal set; the comparison treats a superset as a
  difference to record, not as wrong (plan, *Comparison*).
- `acs-solver` accepts angles in radians, `sketch-solver` in degrees; laman's
  `angle` is radians.

## Adapters

- `src/sketch_solver.rs` (step 3): every C1 kind translates directly; a point
  fixed in both coordinates becomes `Fix`. `translation_residual` is the
  solver's own largest residual with every point and circle guessed at the
  fixture's expected coordinates and no iteration allowed, and the solver may
  move no coordinate off them. `laman-check` says which constraints are
  violated there (none, but a conflicting twin's extra one); those are left out
  of the proof, listed in `translation_unchecked`, and the solver's unmet set
  must equal exactly them. A fixture the checker finds violated that is not
  expected `Unsatisfiable` is an `error`, not numbers.
- `sketch-solver` names a conflict by the later constraint plus the earlier
  ones it depends on (`with`); the report's set is both together. An arc's own
  equation is named `arc:<tag or #id>`.
- `src/acs.rs` (step 4): every C1 kind translates to a JSON request
  (`e<id>` entities, `k<id>` constraints; a `fix` would be two). Lines are
  infinite where laman's are (`extension: true` for `point_on_line`,
  `point_line_distance` and line-circle `tangent`), `angle` is the
  one-root form between rays. An arc's radius and two angles are computed from
  the guesses. The proof is as `sketch-solver`'s with `maxIterations: 0`
  (acs-solver still takes one step in a component not already within 1e-10, so
  the check is status `converged`, the reported initial and final error, and
  that no coordinate moved), and each violated constraint put back alone must
  make the solve `failed`. `conflicting` is one flat list, reported as one set.
