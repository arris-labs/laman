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
- `cargo doc --document-private-items` with warnings denied fails in a checkout (found during m8 review; the gate's public doc stage passes): find the item and either fix it or decide private docs are not held
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
- Frame-budget sources left at 5 % or more after the dense table (none is the solve's arithmetic; the edits are inside 4 ms without them): the cut (`Decompositions::of`, 13–15 % of an edit, a diagnosis cache is S2); `Tolerances::build` (15–18 %); the per-part `split` and `Scope::measured` (6–11 %); the parts built and `remeasured` in the triangles (8.7 %); `freedom_on` factorizing again a drag frame whose pursuit moved the coordinates (0.68 ms of the rectangle drag's 5.0; kept, since the final Jacobian is another matrix and reusing the earlier one would change bits)
- Move `stageable` after the pursuit in a drag frame: saves one factorization (about 0.5 ms of the rectangle drag) but changes which path a staged result takes in an edge case and the work wasted when it fails
- Cache `Tolerances` per `to` in a drag frame (components 0.13 ms, frames 0.22): saves under 0.15 ms and touches a model module
- Fuzz target: also call `to_form` and `check(&sk, &sk.coordinates())`; the serde property only mints dense ids, so `to_form`'s dependency-first reordering is covered by one hand test
- The checker's `ANGLE` is absolute; short lines far from the origin can fail it at rounding level: input to the M2 tolerance ADR
- Degenerate-point tests for `parallel`, `perpendicular`, `angle`, `equal_length`, `symmetric` and tangent at coincident centres (only `point_on_line`, `distance` and an arc end on its centre have one); the FD strategies never draw an exactly degenerate configuration
- A static solve tracks an arc's sweep from the guesses, so a solve that moves the sweep more than half a turn from them reads it the short way round and can name a flip that did not happen, or miss one (ADR-0006, Consequences)
- `laman-debug`'s `configuration` strategy and its `Mode` use words GLOSSARY.md lists under _Avoid_ for **Branch**; rename one side. `tests/residual.rs` also has dead `covered()` scaffolding and picks arcs by `name.contains("Arc")`
- `augment`'s buffer reuse (M8 frame floor step 6) has no equality test of their own, only the stage tests against `reference_rank` and the splitter property
- Review leftovers from m8-dense-rank: the extremes test in `diagnose/tests.rs` asserts one staged factorization per scale over the families, not per case (check 1e±150 and 1e300 per case, keep the sum for 1e-300 only); `diagnose_on` (`solution.rs`) overlays the caller's map on the sketch's, so a partial or stale map no longer comes back unchanged, which `reference::diagnose`'s doc promises; the `DIRECT_FACTOR` comment in `values.rs` counts entities, not parameterised ids (a rectangle chain's ids interleave with its lines'); `values.rs` casts `slot as u32` and `id * KINDS` unchecked; `Values::to_map` repeats `Dense::to_coordinates`; the huge-id test builds a sketch, not a file
- Review leftovers from m8-pursuit: `diagnose::multipliers` factorizes the unit rows again beside `null_space`'s QR (one factorization per pursuit iteration would be cheaper); no test asserts the 498 rectangle drag's pursuit iterations; the zero-skipping QR proptest never draws `-0.0` (the skip may leave a `-0.0` the dense method turns to `+0.0`); `null_space` and `far_goal_stationary` tests use unnamed `1e-10`/`1e-9`; the `b -= (v·b) v` projection is written three times; `refused_drag`'s `FAR` works only between 3000 and 10000 radii
- Close-cycle drift left in code: `bench_c2.rs:1,25` still say "C2 baseline" and 48 workers / 240 s (it is 52 / 260 s); about 33 comments in `crates/laman/src`, `crates/laman/Cargo.toml` and `crates/laman-debug/examples` cite deleted `plans/<slug>`; `values` and `residual` import each other through a type alias (`values.rs:11`, `residual.rs:30`), against ARCHITECTURE.md's module-table rule; ARCHITECTURE.md §Drag sessions' bisection cost on `bisect-stuck` (280 µs against 42 µs) is the C1 figure, not re-measured; the 498-entity rectangle DogLeg edit touched 4.155 ms in one sample (median 2.97)

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

### framework-conflict-inconclusive
- What: the same family, both algorithms `NotConverged` with no conflict named
- Where: `laman::diagnose`
- Seen: same property, M2 step 8
- Reproduce: `cargo test -p laman-debug --test solve_regression -- --ignored framework_conflict_inconclusive`
- Measured: rigid distance framework plus one stretched distance. The whole path (`laman::reference::solve`) shows it under LM too where the staged analysis proves the set (property seed `58eab082…`, kept in `solved.proptest-regressions`)
- Also at `PROPTEST_CASES=20000` (plans/m7-drag-conflicts review): (a) a five-distance case, regression/framework-conflict-inconclusive-circuit (two fixed points, two free points each tied to both, the fifth distance between them stretched by 1.00002): the staged solve ends `NotConverged` under LM, about 1 stretched solve in 6000 of the circuit property (`solved.rs` counts and bounds it); (b) `an_extra_distance_is_redundant_when_true_and_a_conflict_when_false` (staged = whole check, `support/staged.rs:60`) about 1 case in 10000: staged `[]`, whole names {7, 12, 14}; input `steps = [(0,62.362974614242034), (-69.0403764012459,0; nudge (0,0.09744215601998288); ties [1]), (0,-43.29504348460387), (-7.86902799208324,-54.77203232424581; nudge (0.031051381421328888,0)), (26.879996666022468,94.77420170446021; nudge (0,-0.030821528787309888)), (39.890714266668866,0; nudge (0,-0.03248348545513495); ties [35,3]), (48.67845169294814,85.52985789506866; nudge (-0.06595306681022901,-0.06009572363438356); ties [30,51])]`, `pick = (8, 23)`, `false_by = 2.5944183626117723`. Not shrunk; its seed is not kept in `solved.proptest-regressions` (it would fail the gate every run)
- State: regression/framework-conflict-inconclusive

### symmetric-subnormal-axis
- What: a satisfied `symmetric` about a line whose extent is subnormal (7.7e-309): the checker measures NaN
- Where: `laman-check` (the symmetric measure) and the solve's status at a collapsed axis
- Seen: property `any_well_formed_sketch_solves_to_a_complete_answer` at `PROPTEST_CASES=20000`, about 1 case in 16000 (plans/m7-drag-conflicts review)
- Reproduce: `cargo test -p laman-debug --test solve_regression -- --ignored symmetric_subnormal_axis`
- Measured: status `UnderConstrained` (as expected), checker violation `Symmetric` measured NaN, the axis degenerate `ZeroLengthLine`; same family as `extreme-point-on-line`
- State: regression/symmetric-subnormal-axis

### satisfied-status-degenerate-tangent
- What: a static solve reports `UnderConstrained` where the checker finds the tangency violated (NaN measure) and the circle and line degenerate
- Where: `laman::diagnose` (status at degenerate coordinates), same family as `drag-negative-radius`
- Seen: property `any_well_formed_sketch_solves_to_a_complete_answer` at 20000 cases, M6 step 2
- Reproduce: `cargo test -p laman-debug --test solve_regression -- --ignored satisfied_status_degenerate_tangent`
- Measured: a line of length 1.04e-209 through the centre of a circle of radius 0.001, tangent; Levenberg-Marquardt ends at r = 3.7e-14
- State: regression/satisfied-status-degenerate-tangent

### far-goal-stationary
- What: a goal at 1e300 is called stationary at once and the frame ends converged where it started: `norm(&g)` overflows to infinity, so `|p| <= acceptance + GOAL_ROUNDING * |g|` passes
- Where: `laman`, `solve.rs`, `pursue`
- Seen: plans/m8-pursuit step 1, 2026-11-07, a hand test at f64's extremes
- Reproduce: `cargo test -p laman-debug --test solve_regression -- --ignored far_goal_stationary`
- Measured: point on a radius-10 circle, target (-1e300, 3e299): ends (10, 0), both algorithms; 1e30 and 1e150 end nearest the target
- State: regression/far-goal-stationary

## Rejected

- The working name `clevis`: taken on crates.io and PyPI (SEED.md §2)
- Rigid bodies in the remainder, for the rectangle drag and the slot bisect: they remove the same factorizations a pursuit plan removes more cheaply, cannot go below the same 3.4–3.9 ms floor, and C2's stop rule no longer calls for them (ADR-0011)
