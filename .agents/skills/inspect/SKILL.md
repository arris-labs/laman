---
name: inspect
description: See a sketch without a GUI — dump it as text and SVG and look — and turn a failure into a regression fixture. Use whenever a solve, a drag or a diagnostic is wrong or suspicious, before trusting a green test on a change to how sketches solve, and when shrinking a bug.
---

# inspect — dump it to SVG and look

The human has no better view of a sketch than the agent: there is no GUI.
Seeing a sketch means writing it out and reading it. Pictures are for the
agent's eyes during work; a picture is never a test (`docs/README.md`).

## Seeing

- **Text dump**: every parameter, entity and constraint with its id, tag,
  residual at the solution, and the diagnostics (status, DoF per entity,
  redundant and conflicting sets). It diffs in a test.
- **SVG dump**: the entities drawn to scale, construction geometry dashed,
  each constraint marked at its entities with its id or tag, residuals
  above tolerance in red, free directions as arrows, a drag's path as a
  trail. Written under `target/inspect/<slug>/`, then read with the Read
  tool, which shows the image.
- Both live in `laman-debug` (`dump::text`, `dump::svg`). Until the solver
  exists they show the checker's report, not residuals; never claim to have
  looked at a picture that was not made.

Look for: a tangent circle on the wrong side, an arc swept through its
centre, a point that moved during a drag although no constraint reaches
it, a conflict set that names a constraint the picture shows satisfied.

## Diagnosis loop

A failure that resists a first fix gets this before any theory:

1. **A red loop**: the smallest command that shows the failure, run in
   seconds (one test, one fixture).
2. **Ranked hypotheses**: three at most, each with the observation that
   would confirm or kill it. Run the cheapest discriminating one first.
3. Change one thing at a time; when a hypothesis dies, say so in the reply.

## Failure → fixture

1. Reproduce with the smallest sketch: drop entities and constraints while
   it still fails.
2. Commit it under `tests/fixtures/regression/<slug>/` (the sketch's serde
   form and the expected values: coordinates, DoF, conflict set, each from
   a closed form or the construction, never from laman's output) with a
   test that asserts the *desired* result, `#[ignore = "<what is wrong>"]`.
3. The fixing commit removes the `ignore`. The fixture is never deleted.
A finding a run measured but nobody shrank yet goes to `docs/BACKLOG.md`
§Findings first, with its seed and commit.
