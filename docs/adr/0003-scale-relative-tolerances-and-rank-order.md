# ADR-0003: Scale-relative tolerances and deterministic rank ordering

- Status: accepted
- Date: 2026-11-01
- Closes: SEED.md §9 absolute versus scale-relative tolerances

## Context

Caller units are arbitrary. An absolute tolerance alone changes the answer
when a drawing changes units. Translation must not loosen geometric
acceptance; unrelated distant components must not loosen it either.
Finite input does not guarantee representable differences or a resolvable
geometry. A numerical failure is not proof of conflict.

## Decision

Freeze a scale `S` per structural component at compilation, from its input
coordinates and dimensions: the maximum of its x span, y span, circle/arc
radii, and length-valued dimensions (not angles). Include referenced ground
points and fixed-only equations. For `fix`, include the target point in
the spans. Empty or zero-extent components have `S = 0`. Compute spans
with overflow detection, saturating at `f64::MAX`; use scaled hypot for
radii. Never use distance from the origin. Deleting constraints recompiles
components and scales. Trials do not change their acceptance threshold.

Use `L = max(1e-12, 1e-11 S)` in caller length units. Keep the existing
angular/dimensionless acceptance `1e-12`. Use `L` for zero-length/radius
checks too. Thus a feature below the floor cannot be certified as resolved,
and scale invariance is deliberately limited by that floor. Arc endpoint
coincidence also remains a degeneracy observation.

The compiled parameter coordinates are relative to a component origin
(the lowest-id referenced point), divided by `max(S, 1e-12)`. Length
residuals are divided by that same scale; ratio residuals are unchanged.
These common scales preserve minimum physical coordinate change within
a component. Form differences safely; a non-finite transformation or
trial is rejected within the solve budget, never certified by saturation.
Only the scale's span calculation saturates. Return finite best effort on
exhaustion. The checker measures original geometry with the same length
policy, without using solver residuals or compilation.

For rank, normalize nonzero Jacobian rows to unit Euclidean norm after
scaling. A zero row contributes no rank. Use dimensionless threshold
`1e-10` for a direction outside an existing span. Near this boundary DoF
is a numerical estimate; do not claim a uniform theorem for ill-conditioned
sketches. Residual acceptance never uses the rank threshold.

### Ordering

Parameters sort by `(EntityId, Coordinate)`; intrinsic equations precede
constraints, each in `(id, equation index)` order. Components sort by their
smallest parameter; fixed-only equations keep source order.

Production rank/null-space work uses column-pivoted QR of the scaled
Jacobian, with equal pivot norms resolved by original parameter order.
QR column order must not decide redundancy: diagnose constraints in source
order by removal/recompilation with contact provenance. A later duplicate
is named instead of an earlier one.

Construct null vectors by taking non-pivot parameter columns in original
order, solving for pivot coordinates, unpermuting, then orthonormalizing
in that order with two passes. Normalize each vector; its first component
whose magnitude exceeds the rank threshold is positive. Canonicalize
zero components to positive zero. Per-entity rank uses these vectors
restricted to the entity's parameters, including referenced points.
Fixed summation order and explicit pivot ties are required across targets;
a library's undocumented tie behavior is not a contract.

### Evidence and limits

`numerical_contract.rs` holds the decision as test-only experiments until
compiler/checker adoption in M2 step 2. There is no new public API or
checker behavior in this commit. Production constants belong together in
`model::tolerance`; its existing `LENGTH` remains the legacy absolute
constant until that adoption, with compatibility documented there.

- Translated 3-4-5 distances, scales 1e-6, 1, 1e3 and 1e6 and translations
  up to 1e4 times scale, err by less than 1.5e-7 of `L`. Perturbations of
  100 `L` are rejected. At C1 size 120, `L = 1.2e-9`; fixture coordinate
  comparisons retain their independent, unchanged 1e-9 requirement.
- Tangential motions 1e-3, 1e-6 and 1e-9 have original residual
  `hypot(1,motion)-1`, quadratic and eventually rounded to zero; the
  contact equation resolves them linearly above `L`.
- Unit-row rank tests distinguish directions 100 times above and below
  the rank threshold. The C1 original/contact ranks are in ADR-0002.
- Scales 1e-300, 1e300 and `f64::MAX` yield finite positive tolerances.
  1e-300 lies below the floor; `1e300 + 1` equals `1e300`. Such geometry
  cannot be recovered by increasing a tolerance or pretending it converged.

These measurements establish a baseline, not bounds for every nonlinear
solve. Steps 3–4 measure damping, trust-region defaults and hard budgets;
no choice here widens a fixture's acceptance or guarantees convergence.

## Consequences

The checker keeps its two-argument API. Shared scale-policy helpers belong
in the model, with model-only dependencies; compiler and checker consume
them independently. Source removal must recompute scale as well as contact
provenance. Digests may change when the policy is adopted, with an explicit
reason and revalidation, not as part of these experiments.

## Alternatives considered

Absolute-only tolerances fail unit changes. Origin-magnitude scaling hides
errors after translation. One global scale lets unrelated geometry alter
a component's acceptance. Per-parameter scaling changes the physical
least-change metric. Minimum-cardinality conflict guarantees do not follow
from any rank threshold and remain the separate conflict ADR's concern.
