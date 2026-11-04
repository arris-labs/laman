# ADR-0010: A radius is bounded below by zero, inside both engines

- Status: accepted
- Date: 2026-11-04
- Amends: ADR-0005 (the certificate reads the projected gradient at a held
  radius), ADR-0006 (a solve no longer carries a radius through zero)

## Context

A circle's radius is a free parameter, and no residual laman writes reads
its sign: `radius`, `equal_radius` and the tangencies read `r` linearly.
Nothing in either engine kept it at or above zero, so a solve could end
below it, and `Sketch::apply` refuses such a solution
(`DimensionOutOfRange`, `r >= 0`): two findings, both from the drag
property.

- `negative-radius-solve`: circles of radius 0.001 and 3.03e91 externally
  tangent, centres 764.76 apart. Both engines put almost the whole
  correction on the small radius and end at -7.57e90.
- `drag-negative-radius`: a circle tangent to a line through its own
  centre, so the only radius that holds is 0; DogLeg's drag frame ends at
  -2.55e-19, inside every tolerance.

Clamping the answer after the solve fixes `apply` and breaks the solution:
the residual of the first moves by the clamp itself (7.6e90), and a
status computed before the clamp no longer describes the coordinates
returned. The bound has to be one the iteration knows about. No reference
solver was read for this decision.

## Decision

**The bound is `r ≥ 0`, held inside the iteration, by a projected step
with an active set** (Bertsekas, *Nonlinear Programming*, 2nd ed., 1999,
§2.3: projection onto a box, the two-metric form). In normalized
coordinates a radius has origin 0, so the bound is `x ≥ 0` there too. At
every iterate of a descent, under both Levenberg–Marquardt and DogLeg:

- a radius is **active** when it sits on the bound (`x ≤ 0`) and the
  objective's gradient `(Jᵀf)_r` is positive, so descent would carry it
  below. Its Jacobian column is zeroed in the model the step is taken
  from, and its step is zero (LM's damped normal equations give exactly
  that; DogLeg's SVD leaves rounding there, which is dropped), so the
  other parameters take the step the model gives without it;
- the proposed step is **cut** where it would carry any radius below 0
  (`δ = −x`, so `x + δ` is exactly 0), and the predicted decrease the gain
  ratio reads is that of the cut step;
- every trial (`solve::trial`, which the goal phase, the continuation of a
  staged part and the curvature probe share) holds a radius that would land
  below 0 at 0, so no iterate anywhere has a negative radius.

Where no radius is active and no step crosses 0, the model, the step and
the prediction are the unbounded ones, bit for bit: the C1 corpus's 50
digests are unchanged.

**The certificate** (ADR-0005) reads the projected gradient: a component of
`Jᵀf` along a radius at 0 that presses it below is dropped before
`|Jᵀf| ≤ STATIONARY |w|` is tested. A least-squares point pressed against
the bound is stationary for the bounded problem, which is the one laman
solves.

**Statuses.** A radius at 0 is degenerate, as it was (the diagnostics' and
the checker's `ZeroRadius`); the status is read as before, at coordinates
that now apply. Both findings end `UnderConstrained`:

- `negative-radius-solve`: the small radius held at 0, the large one and
  the centres take up the tangency (six parameters, one equation: DoF 5,
  the fixture's `expected.json` corrected from 3 by that count);
- `drag-negative-radius`: the radius at 0 (DogLeg) or 3.7e-14 (LM), DoF 4
  (five parameters, one equation).

DoF is the rank of the Jacobian, as ADR-0003 has it; the bound does not
enter it. A radius held at 0 by an unsatisfiable tangency is a conflict by
the projected certificate, not a free parameter.

`satisfied-status-degenerate-tangent` (a collapsed line through a circle's
centre, the checker's measure NaN) is not lifted by the bound: its radius
ends positive, and its fault is the status at a degenerate contact.

## Consequences

- Every solution a solve or a drag returns has every radius `≥ 0` and
  `apply` accepts it on the sketch it came from; the `DragSession` gap is
  gone.
- A circle no longer flips by its radius passing through zero during a
  solve (ADR-0006's radius subject). The observation stays: it costs
  nothing and still reads a radius given by a caller's coordinates.
- An over-constrained sketch whose only way out was a negative radius is
  now `Unsatisfiable` with a conflict set where it used to "converge" to
  coordinates `apply` refused.
- Points and arcs have no bound; an arc's radius is implied by its points
  (ADR-0001) and is never negative.

## Alternatives considered

- **Clamp the answer.** Moves residuals by the clamp; the status no longer
  describes the coordinates. Rejected (Context).
- **Cut only, no active set.** A step pressing a radius below 0 is cut at
  every iterate, but the other parameters take the step computed as if the
  radius moved: the cut step is often no decrease, rejected, and the
  damping rises until steepest descent crawls along the bound. The active
  set costs one column copy where a radius is active, nothing elsewhere.
- **Reparametrize** (`r = s²` or `r = |s|`). Changes every radius's
  Jacobian, so every digest; `s²` has a zero derivative at `r = 0`, where
  the second finding lives, and `|s|` is the old sign problem moved.
- **Name it a flip and return the negative radius.** `apply` would still
  refuse it, and a caller cannot draw a negative circle.
