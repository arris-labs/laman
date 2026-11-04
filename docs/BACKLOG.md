# Backlog

One line per raw idea. Picking one up means `/idea` (needs thinking) or
`/plan` (obvious); the line is removed then — as is a line a roadmap cycle
has committed to, which now lives in `docs/ROADMAP.md` instead. Rejected
ideas keep one line below with the reason, so the same idea is not
re-brainstormed. A *measured* defect is not a raw idea: it goes under
[Findings](#findings) below, in the fixed shape.

- Run SolveSpace and PlaneGCS through `tools/oracle/` for the Python column of COMPARISON.md (GPL/LGPL Python bindings; out of the Rust-field comparison)
- C2 corpus: share the edit replacement of `examples/bench_c2.rs` and `tests/support/c2.rs` as an exhaustive `Edit::apply`, and keep the inspect dumps out of the timed tests
- Ask a consumer whether dimension edits need `Sketch::set_dimension` beyond remove/re-add with the same constraint id; M5 uses the existing API
- Validate C2's interpretation of nested constraints with a consumer: triangle strips cover deep dependencies and slot grids cover profiles inside a profile; dimension references or other nested shapes may need additional corpus families
- `tools/oracle/`: the three adapters repeat the run preamble, `TRANSLATION_TOLERANCE`, the translation-proof flow and the `a_wrong_expectation_is_an_error_not_numbers` test; share them. The adapters also drop laman's `sense` (the other solvers infer the side from the guesses), so a mirrored-branch agreement is possible
- Python `id=` on a constraint builder mints, removes and re-adds, so it burns an id and can raise `IdsExhausted` where Rust would accept; give the Rust builder id-taking variants
- `Sketch.from_json`/`to_json` go through `laman_debug::json`, which is tooling; move the JSON form into `laman` (ADR-0007 justifies the dependency only for the checker, dumps and digest)
- Python tests: `DEFAULT_TOLERANCE` copies Rust's by hand and is absolute, not the sketch's size; `MIN_CROSS` in the hypothesis property is not scale-relative and may flake under the `ci` profile
- The collapse-name mapping is written twice in `laman-py` (`solve.rs`, `check.rs`); no Python test produces a non-empty `flips` or `ill_conditioned`, or the `Origin::Arc` path
- `IncompleteSolution` and `FixedMoved` are unreachable from Python; `Rigidity.is_rigid` re-derives `dof == 0` instead of calling Rust's
- The conflict analysis costs 1–52 ms on the conflicting twins (the slotted plate's is the slowest; timed by the bench, not budgeted); profile it before C2's incremental solve
- Property tests set an explicit `ProptestConfig` case count, so `PROPTEST_CASES` cannot raise it; let the environment win
- Step 4 (DogLeg) of the m2 plan has no commit of its own: it landed inside the step 5 commit (e303c7e)
- `release.yml`: refuse a tag whose commit is not on `main`; pin `crates-io-auth-action` and `pypi-publish` to commit SHAs
- Wheels beyond the five platforms (musllinux, Windows aarch64, PyPy, free-threaded), and a CI job holding the MSRV (1.85)
- `laman-check` on crates.io: ARCHITECTURE.md plans to publish it with the solver's first release (Python users get it as `laman.check`); reserve the name then
- Trademark search for "LAMAN" in software classes (WIPO, EUIPO, USPTO) before the solver's first public release (SEED.md §2); the human's
- The GitHub repository `arris-labs/laman` (the crates' `repository` URL and the README's SEED link point there); the human's
- `/retro`'s session digest: port arris's `session-digest` script when the first plan has sessions to mine
- Digest of the sketch hashes its JSON text, so a `serde_json` float-printing change would move every blessed digest; hash the f64 bits instead (M2, when `Solution` joins the digest)
- Checker gaps: it does not compare fixed parameters with the sketch's values; `laman-debug`'s runner fills missing expected coordinates with the guesses; `tools/check-layers.sh` is evaded by `use laman as l` or a renamed dependency (a `cargo tree` check would close it)
- Malformed file shapes (an untagged tangent or span form matching no variant) arrive as a serde error without the id; a span with both `line` and `a`/`b` silently takes `line`
- Fuzz target: also call `to_form` and `check(&sk, &sk.coordinates())`; the serde property only mints dense ids, so `to_form`'s dependency-first reordering is covered by one hand test
- The checker's `ANGLE` is absolute; short lines far from the origin can fail it at rounding level: input to the M2 tolerance ADR
- Degenerate-point tests for `parallel`, `perpendicular`, `angle`, `equal_length`, `symmetric` and tangent at coincident centres (only `point_on_line`, `distance` and an arc end on its centre have one); the FD strategies never draw an exactly degenerate configuration
- A static solve tracks an arc's sweep from the guesses, so a solve that moves the sweep more than half a turn from them reads it the short way round and can name a flip that did not happen, or miss one (ADR-0006, Consequences)
- `laman-debug`'s `configuration` strategy and its `Mode` use words GLOSSARY.md lists under _Avoid_ for **Branch**; rename one side. `tests/residual.rs` also has dead `covered()` scaffolding and picks arcs by `name.contains("Arc")`

## Findings

A defect a run measured, one block each:

```
### <slug>
- What: …
- Where: crate, file, function
- Seen: run / commit / seed / date
- Reproduce: the command that replays it
- Measured: the numbers
- State: raw | measured | regression/<slug> | fixed
```

### extreme-point-on-line
- What: a solve called satisfied that the checker cannot measure: `point_on_line` is NaN at 1e303 magnitudes (products overflow)
- Where: `laman-check` (and/or the solver's stopping scale), `point_on_line`
- Seen: property `any_well_formed_sketch_solves_to_a_complete_answer`, M2 step 8
- Reproduce: `cargo test -p laman-debug --test solve_regression -- --ignored extreme_point_on_line`
- Measured: status UnderConstrained, DoF 5, checker violation measured NaN
- State: regression/extreme-point-on-line

### dogleg-conflict-inconclusive
- What: a contradictory distance framework: Levenberg-Marquardt proves the conflict, DogLeg ends `NotConverged`
- Where: `laman::diagnose` (conflict analysis) under `Algorithm::DogLeg`
- Seen: property `an_extra_distance_is_redundant_when_true_and_a_conflict_when_false`, M2 step 8
- Reproduce: `cargo test -p laman-debug --test solve_regression -- --ignored dogleg`
- Measured: 6 points, 8 distances; LM `Unsatisfiable`, DogLeg `NotConverged`. The same holds for DogLeg on `false-conflict-rigid-framework` and `false-conflict-excludes-contradiction`, whose LM halves are fixed (`*_dogleg`, ignored)
- State: fixed (plans/m7-drag-conflicts step 5, with `*_dogleg`: every filter test re-solved by Levenberg–Marquardt, ADR-0005 third amendment; the cause is `staged-conflict-inconclusive`'s); regression/dogleg-conflict-inconclusive

### framework-conflict-inconclusive
- What: the same family, both algorithms `NotConverged` with no conflict named
- Where: `laman::diagnose`
- Seen: same property, M2 step 8
- Reproduce: `cargo test -p laman-debug --test solve_regression -- --ignored framework_conflict_inconclusive`
- Measured: rigid distance framework plus one stretched distance. The whole path (`laman::reference::solve`) shows it under LM too where the staged analysis proves the set (property seed `58eab082…`, kept in `solved.proptest-regressions`)
- State: regression/framework-conflict-inconclusive

### negative-radius-solve
- What: a solve returns a circle radius below zero (-7.6e90) that `Sketch::apply` refuses; no flip is named
- Where: `laman::solve` / `laman::solution` (the radius parameter is not bounded below)
- Seen: property `any_point_of_any_sketch_drags_to_any_targets`, M3 step 7
- Reproduce: `cargo test -p laman-debug --test solve_regression -- --ignored negative_radius_solve`
- Measured: externally tangent circles of radius 0.001 and 3e91 with centres 764.76 apart; both algorithms
- State: fixed (plans/m7-drag-conflicts step 1: the radius bound, ADR-0010); regression/negative-radius-solve

### drag-negative-radius
- What: a drag step returns a radius of -2.55e-19 (a circle tangent to a line through its own centre, so r is forced to 0), which `apply` refuses, so `DragSession::finish` does not apply
- Where: `laman::drag`, `laman::solution` (same cause as `negative-radius-solve`)
- Seen: same property, M3 step 7
- Reproduce: `cargo test -p laman-debug --test solve_regression -- --ignored drag_negative_radius`
- Measured: DogLeg ends at -2.55e-19, Levenberg-Marquardt at +3.7e-14
- State: fixed (plans/m7-drag-conflicts step 1: the radius bound, ADR-0010); regression/drag-negative-radius

### satisfied-status-degenerate-tangent
- What: a static solve reports `UnderConstrained` where the checker finds the tangency violated (NaN measure) and the circle and line degenerate
- Where: `laman::diagnose` (status at degenerate coordinates), same family as `drag-negative-radius`
- Seen: property `any_well_formed_sketch_solves_to_a_complete_answer` at 20000 cases, M6 step 2
- Reproduce: `cargo test -p laman-debug --test solve_regression -- --ignored satisfied_status_degenerate_tangent`
- Measured: a line of length 1.04e-209 through the centre of a circle of radius 0.001, tangent; Levenberg-Marquardt ends at r = 3.7e-14
- State: regression/satisfied-status-degenerate-tangent

### upstream-edit-drift
- What: an edit to a downstream distance changes the previously solved coordinate bits of its upstream point
- Where: `laman::solve`, whole-component edit re-solve under both algorithms
- Seen: M5 step 4 shared C2 runner, baseline `87cc742`, 2026-11-04
- Reproduce: `cargo test -p laman-debug --test solve_regression upstream_edit_drift`
- Measured: three points, two distances, second edited 20 → 22; upstream x moves by 248 ULP (LM) or 1 ULP (DogLeg); FullyConstrained, DoF 0 and checker clean before/after
- State: fixed (plans/m6-stages step 4: the staged solve leaves a satisfied stage's bits); regression/upstream-edit-drift

### whole-false-certificate
- What: the whole path names a conflict set that holds together: its conflict filter drops a true distance (`spared`) on a certified stationary point of a satisfiable subset (ADR-0005's certificate is local); the staged path solves that subset and keeps `spared`, so staged and whole conflict sets differ (the plan's "staged = reference on conflict sets" fails here)
- Where: `laman::diagnose::analysis` (`test`, `certified`) on `Path::Whole`: `laman::reference::solve`, and the fallback of a failed component
- Seen: property `an_extra_distance_is_redundant_when_true_and_a_conflict_when_false`, seed `aa67e2f4…`, plans/m6-stages step 7, 2026-11-04; also red at 3495a4b (step 5)
- Reproduce: `cargo test -p laman-debug --test solve_regression -- --ignored whole_false_certificate`
- Measured: LM whole names {1, 3–9, `stretched`}, staged {`spared`, 1, 3–9, `stretched`}; a checker-clean witness of everything but `spared` is in the fixture; DogLeg ends `NotConverged` on both paths
- State: fixed (plans/m7-drag-conflicts step 4: every filter test re-solved by the default staged solve, ADR-0005 second amendment); regression/whole-false-certificate

### staged-conflict-inconclusive
- What: under DogLeg the staged conflict analysis ends `NotConverged` with no set where the whole-component path (`laman::reference::solve`) proves one
- Where: `laman::diagnose::analysis`, the deletion filter's re-solves of subsets by the staged solve, `Algorithm::DogLeg`
- Seen: property `an_extra_distance_is_redundant_when_true_and_a_conflict_when_false`, seed `4ca64a2b…` (kept in `solved.proptest-regressions`), M6 retirement, 2026-11-04
- Reproduce: `PROPTEST_CASES=1 cargo test -p laman-debug --test solved an_extra` with a strict staged = whole comparison
- Measured: 6 construction steps, `false_by` 2.03; whole DogLeg names {5, 11, 12}, staged DogLeg names nothing; the property exempts an inconclusive side from the staged = whole comparison until this is shrunk to a fixture and fixed (M7's conflict work)
- Cause: under DogLeg every filter test of the subset {leg-a, leg-b, span} with constraint 10 (as many equations as parameters, inconsistent: its least-squares point is where the Jacobian is singular) ended at the 128-iteration cap 2–5× above `STATIONARY` (ratios 2.2e-6, 4.6e-6; the dog leg needs about 200 iterations there, LM 85). Which path concluded was decided by the budget share an earlier, satisfied component left (2 iterations staged, 4 whole), not by staging
- State: fixed (plans/m7-drag-conflicts step 5: every filter test re-solved by Levenberg–Marquardt, ADR-0005 third amendment); regression/staged-conflict-inconclusive (shrunk to 7 points, 9 distances; the set is the triangle inequality {leg-a, leg-b, span})

## Rejected

- The working name `clevis`: taken on crates.io and PyPI (SEED.md §2)
