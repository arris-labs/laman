# ADR-0001: An arc is a centre and two endpoints, counter-clockwise, its radius implied

- Status: accepted
- Date: 2026-11-01
- Closes: the arc parametrisation `⚠ OPEN` of SEED.md §9

## Context

An arc's parametrisation decides its DoF, which constraints are intrinsic
to it, how it shares endpoints with lines, and how its branch can flip
(SEED.md §9, §10.3). The two candidates are the ones the field uses:

- **A — centre + start point + end point**, the radius implied by the
  centre–start distance and one intrinsic equation holding the end on the
  same circle. SolveSpace's arc of circle works this way: its library documentation
  (`exposed/DOC.txt`, `SLVS_E_ARC_OF_CIRCLE`) gives a centre, a beginning
  and an end point, counter-clockwise, and the implicit equation
  distance(centre, beginning) = distance(centre, end). No SolveSpace
  source was read. `sketch-solver` does the same (`docs/COMPARISON.md`).
- **B — centre + radius + start angle + end angle**, with the endpoints
  derived, or added as points tied by two intrinsic equations each.
  PlaneGCS works this way: its `Arc` is a `Circle` (centre, `rad`) with
  `startAngle`, `endAngle` and start and end points, the points tied to
  the angles by `System::addConstraintArcRules`, two point-on-curve-at-
  angle equations per endpoint (read in the reference trees:
  `src/Mod/Sketcher/App/planegcs/Geo.h`, `GCS.cpp`). So does
  `arael-sketch-solver`, from its documentation.

## Decision

An arc is **three point references — centre `c`, start `s`, end `e` —
and no parameters of its own.** It runs counter-clockwise from `s` to `e`
around `c`. One intrinsic equation, `|e − c| − |s − c| = 0`, holds the end
on the start's circle; the radius is `|s − c|`. DoF: 6 − 1 = 5. The points
are ordinary points of the sketch, so a line and an arc share an endpoint
by referring to the same point, with no coincident constraint.

## Consequences

- Every arc equation is algebraic in point coordinates plus `sqrt`, which
  IEEE 754 rounds correctly: no transcendental enters an arc's residual
  or Jacobian. Only an angle-valued dimension (an arc's sweep, an angle
  between lines) calls `libm`, once per solve for its target value.
- Shared endpoints need no equations: a closed profile of lines and arcs
  has fewer equations than with B, and its DoF accounting is the obvious
  one.
- The sweep is derived, `atan2(cross(s − c, e − c), dot(s − c, e − c))`
  taken in `(0, 2π)`, so it is not continuous where `e` passes `s`: a
  drag can swing an arc through its full circle with every residual zero.
  That is a branch flip, caught by the branch check
  (`docs/ARCHITECTURE.md` §Branches), never by an equation.
- A zero-radius arc (`s = c`) has a degenerate intrinsic equation; it is
  reported as a degenerate entity (`docs/DATA-MODEL.md` §Diagnostics).
- A circle is not an arc with equal ends: it is a centre point and its
  own radius parameter (`docs/DATA-MODEL.md` §Entities). "Equal radius"
  and "tangent" accept either and read the radius through one accessor.

## Alternatives considered

- **B, angles as parameters.** Its sweep is continuous and explicit, which
  makes a flip through the full circle visible as a parameter jump. It
  costs three more parameters and four intrinsic equations per arc when
  the endpoints are points (as a sketcher needs them), puts `sin`/`cos` in
  every endpoint equation, and its angles wrap at `±π`, which needs its
  own handling. Rejected: branch tracking is needed anyway for tangents
  and angles, so B's advantage is one case of a check that exists.
- **B without endpoint points**, endpoints derived on demand. Sharing an
  endpoint with a line then needs a constraint with trigonometry in it,
  for every joint of every profile. Rejected.
