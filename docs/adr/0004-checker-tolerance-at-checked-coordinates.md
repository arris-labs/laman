# ADR-0004: The checker measures its tolerance at the coordinates it checks

- Status: accepted
- Date: 2026-11-01
- Amends: ADR-0003 (where the checker's component scale comes from)

## Context

ADR-0003 freezes each structural component's scale from the input sketch,
and says the checker measures original geometry "with the same length
policy". As adopted, the checker computed that scale from the sketch's
stored coordinates, which are the solve's guesses. A guess then decides
how strict the check is. A length-1 distance whose free end is guessed at
(1e6, 0) gets a component scale of 1e6 and a tolerance of 1e-5. At the
solution (1, 0) it accepts a 1e-6 error, 1e5 times looser than the
tolerance at the solution's own scale (regression `check-guess-scale`).
The checker's verdict must be a property of the coordinates it judges,
not of where a solve started (SEED.md §7).

## Decision

- **Checker.** `laman_check::check(sketch, values)` takes its scales from
  `Tolerances::at(sketch, values)`. That is ADR-0003's extent (spans,
  radii, length dimensions, `fix` targets, components unchanged), measured
  at `values`, with a coordinate missing from `values` read from the
  sketch. A non-finite value enters no extent, so one `inf` cannot
  saturate the scale and loosen the rest of its component.
  `Tolerances::new(sketch)` is `at` the stored coordinates.
- **Solver.** The frozen input scale still sets normalization, the
  component origin and degeneracy domains, as ADR-0003 says. Length
  acceptance of an iterate is `min(L(S_input), L(S_iterate))`, with
  `S_iterate` measured exactly as the checker does. It only tightens. So
  a converged component meets the checker's threshold at its own
  coordinates, and a solve cannot loosen acceptance by moving geometry.
  An already satisfied input is measured at its own coordinates, so it
  still takes no step and keeps its bits.

## Consequences

The checker and the solver both call one model function on one structural
partition. The checker's measurements stay its own; only the scale rule
is shared, as ADR-0003 intended. Geometry that a solve grows legitimately
(a pure-geometric sketch with no dimension) is judged at its grown size,
which is the policy: tolerance relative to the sketch as it is.
`check-guess-scale` holds the checker. `solve::tests` holds the solver at
the same fixture, reaching the construction's coordinates.

## Alternatives considered

- **The larger of the guess and solution scales**: still lets the far
  guess win in exactly the failing case.
- **Dimensions only**: a sketch with no length dimension has no scale but
  its geometry, and the floor alone would refuse ordinary drawings.
- **Leave the solver frozen**: it could report success the checker then
  refuses. A solver that stops before the checker's threshold has stopped
  too early.
