# Backlog

One line per raw idea. Picking one up means `/idea` (needs thinking) or
`/plan` (obvious); the line is removed then — as is a line a roadmap cycle
has committed to, which now lives in `docs/ROADMAP.md` instead. Rejected
ideas keep one line below with the reason, so the same idea is not
re-brainstormed. A *measured* defect is not a raw idea: it goes under
[Findings](#findings) below, in the fixed shape.

- The conflict analysis costs 1–52 ms on the conflicting twins (the slotted plate's is the slowest; timed by the bench, not budgeted); profile it before C2's incremental solve
- Property tests set an explicit `ProptestConfig` case count, so `PROPTEST_CASES` cannot raise it; let the environment win
- Step 4 (DogLeg) of the m2 plan has no commit of its own: it landed inside the step 5 commit (e303c7e)
- Release automation: a `release.yml` that publishes the crate and multi-platform abi3 wheels (Linux, macOS, Windows; manylinux older than 2_34) from a pushed `v*` tag, each behind an environment reviewer, and a `/release` skill, before the first 0.1.0. 0.0.1 went up by hand with one `manylinux_2_34_x86_64` wheel and the sdist
- `laman-check` on crates.io: ARCHITECTURE.md plans to publish it with the solver's first release (Python users get it as `laman.check`); reserve the name then
- Trademark search for "LAMAN" in software classes (WIPO, EUIPO, USPTO) before the solver's first public release (SEED.md §2); the human's
- The GitHub repository `arris-labs/laman` (the crates' `repository` URL and the README's SEED link point there); the human's
- `/retro`'s session digest: port arris's `session-digest` script when the first plan has sessions to mine
- Determinism on more than one OS: CI's test job on macOS and Windows beside Linux, once the digest exists
- Digest of the sketch hashes its JSON text, so a `serde_json` float-printing change would move every blessed digest; hash the f64 bits instead (M2, when `Solution` joins the digest)
- Checker gaps: it does not compare fixed parameters with the sketch's values; `laman-debug`'s runner fills missing expected coordinates with the guesses; `tools/check-layers.sh` is evaded by `use laman as l` or a renamed dependency (a `cargo tree` check would close it)
- Malformed file shapes (an untagged tangent or span form matching no variant) arrive as a serde error without the id; a span with both `line` and `a`/`b` silently takes `line`
- Fuzz target: also call `to_form` and `check(&sk, &sk.coordinates())`; the serde property only mints dense ids, so `to_form`'s dependency-first reordering is covered by one hand test
- The checker's `ANGLE` is absolute; short lines far from the origin can fail it at rounding level: input to the M2 tolerance ADR
- Degenerate-point tests for `parallel`, `perpendicular`, `angle`, `equal_length`, `symmetric` and tangent at coincident centres (only `point_on_line`, `distance` and an arc end on its centre have one); the FD strategies never draw an exactly degenerate configuration
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
- Reproduce: `cargo test -p laman-debug --test solve_regression -- --ignored dogleg_conflict_inconclusive`
- Measured: 6 points, 8 distances; LM `Unsatisfiable`, DogLeg `NotConverged`
- State: regression/dogleg-conflict-inconclusive

### framework-conflict-inconclusive
- What: the same family, both algorithms `NotConverged` with no conflict named
- Where: `laman::diagnose`
- Seen: same property, M2 step 8
- Reproduce: `cargo test -p laman-debug --test solve_regression -- --ignored framework_conflict_inconclusive`
- Measured: rigid distance framework plus one stretched distance
- State: regression/framework-conflict-inconclusive

### negative-radius-solve
- What: a solve returns a circle radius below zero (-7.6e90) that `Sketch::apply` refuses; no flip is named
- Where: `laman::solve` / `laman::solution` (the radius parameter is not bounded below)
- Seen: property `any_point_of_any_sketch_drags_to_any_targets`, M3 step 7
- Reproduce: `cargo test -p laman-debug --test solve_regression -- --ignored negative_radius_solve`
- Measured: externally tangent circles of radius 0.001 and 3e91 with centres 764.76 apart; both algorithms
- State: regression/negative-radius-solve

### drag-negative-radius
- What: a drag step returns a radius of -2.55e-19 (a circle tangent to a line through its own centre, so r is forced to 0), which `apply` refuses, so `DragSession::finish` does not apply
- Where: `laman::drag`, `laman::solution` (same cause as `negative-radius-solve`)
- Seen: same property, M3 step 7
- Reproduce: `cargo test -p laman-debug --test solve_regression -- --ignored drag_negative_radius`
- Measured: DogLeg ends at -2.55e-19, Levenberg-Marquardt at +3.7e-14
- State: regression/drag-negative-radius

## Rejected

- The working name `clevis`: taken on crates.io and PyPI (SEED.md §2)
