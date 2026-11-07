# ADR-0013: The profile check measures by sectors, and an open join is not compared

- Status: accepted
- Date: 2026-11-08
- Scope: C3's profile check, `laman_check::check_profiles` (`plans/m10-profile-check`)

## Context

M10 gives `laman-check` its own measure of what `laman::profile::profiles`
reports: each loop's area and orientation, and which loop holds which. The
checker is a check only if it shares nothing with what it checks (SEED.md
§7). It reads `laman::model` and DATA-MODEL.md, never `laman::profile`, so
it needs its own formula, its own nesting test, and a rule for when two
honest measures may differ.

Extraction (ADR-0012 D3) sums Green's `½∮(x dy − y dx)` from the loop's
first join, each arc as its chord plus the segment `r²(θ − sin θ)/2`, by a
series below one radian. It reads `r` from the arc's start, and tests
nesting by the winding number of an angle sum.

Two cases have no single right area. Green's sum is independent of its
origin only on a closed curve. ADR-0012 D1 closes a loop at every declared
join, a `coincident` the coordinates violate included. Where a join's two
ends lie `g` apart, moving the origin by `d` moves the sum by up to
`½|d|g`, and the two measures pick different origins. Likewise an arc whose
ends sit at radii `r_s ≠ r_e` violates its intrinsic equation, and its area
depends on which radius is read. ADR-0012 also lets through loops that
cross themselves, on which the turning number is undefined and even-odd
and nonzero containment disagree.

## Decision

**D1. The measure.** Each loop is measured in the checker's own frame: its
coordinates translated to the loop's first point (where its first edge's
traversal enters) and scaled by an exact power of two to an extent near 1,
arc centres and a circle's radius counted in the extent. Per edge, about
that origin `o`:

- a line `a → b` is one triangle, `½ cross(a − o, b − o)`;
- an arc `s → e`, counter-clockwise about `c` with sweep `θ` in `(0, 2π)`
  (DATA-MODEL.md §Entities), is triangle, sector, triangle:
  `½ cross(s − o, c − o) + ½ r²θ + ½ cross(c − o, e − o)`, with
  `r² = |s − c|·|e − c|`, negated when traversed reversed;
- a circle is `πr²`, negated when reversed.

The arc form is exact for any origin and has no series and no small-sweep
branch, so it shares neither formula nor code path with extraction's.
Orientation is the sign of this area. No turning number is computed.

**D2. Open joins and off-radius arcs.** `L` is the largest
`Tolerances::at` entity tolerance among the loop's edges (ADR-0004). A join
is the pair of ends where one edge meets the next; its separation `g` is
their distance in the coordinates, 0 for a shared point id. A loop with a
join where `g > L` is an **open join**, and with an arc where
`|r_s − r_e| > L`, an **off-radius arc**. It gets `OpenJoin` or
`ArcOffRadius`, and its area and orientation are not compared: no origin
is the right one, and `check` already names the violated constraint.

Under that threshold the two measures agree within the **allowance**
`L·P + ½·D·Σg`. `P` is the loop's perimeter and `D` its extent. `L·P`
bounds what a boundary moved by `L` changes, an arc's choice of radius
included. `½·D·Σg` bounds the origin's effect across nearly-closed joins.
The allowance gets a rounding term only if step 2 measures one past it,
and then a named constant in `laman-check` whose comment gives that
measurement. Sector cancellation on a near-straight arc of radius `R` is
of order `ε·R·chord`. Radii enter the extent `L` is scaled by (ADR-0003),
so `L·P` is expected to cover it; step 2 tests this. (Settled by step
2: a rounding term is needed, see *Addendum*.)

`|measured − reported|` above the allowance is `Area`. A measured area
within the allowance of 0 has no sign: orientation is undecided, never a
violation. A decided sign against the loop's role (outer
counter-clockwise, hole clockwise) is `Orientation`.

**D3. Extremes.** The comparison runs in the frame, never in caller units.
The reported area is brought into the frame by the same power of two. A
reported `±f64::MAX` agrees with a measure of the same sign at or past it
in caller units. A reported 0 or subnormal agrees with a measure that
underflows there. A coordinate the values lack, or one that is not finite,
leaves its loop unmeasured and is listed in `missing` by its parameter.
Extraction refuses both, so such a report shows the values are not those
it read. No verdict is NaN.

**D4. Containment.** A loop's **probe** is its first edge's midpoint (an
arc's at half its sweep), or a circle's point at angle 0. Each hole's probe
is tested against its own profile's outer loop, and against no other.
Every profile's outer probe is tested against every other profile's plate:
inside its outer and inside none of its holes. That holds islands in their
hole and disjoint loops apart. The test is a ray from the probe toward +x,
its crossings with lines and arcs counted signed by the edge's direction.
The count is half-open in `y`: a line, or an arc cut at its highest and
lowest points, crosses where one end lies strictly above the probe and the
other at or below it, so a vertex on the ray counts once. The signed
count is the winding number by another route than extraction's angle sum.
A probe is inside where it is nonzero, the rule extraction nests by. A
probe within `L` of the boundary it is tested against is undecided, never a
violation. A hole outside its outer, or a probe inside another profile's
plate, is `Containment`.

## Consequences

- The check agrees with extraction on every closed loop up to the
  allowance. On a loop with an open join it reports the join, not an area.
- A sketch whose solve left a joining `coincident` violated gets `OpenJoin`
  from the profile check as well as the violation from `check`.
- Containment costs one ray per probe per boundary: quadratic in loops, as
  extraction's is.
- A broken twin with a hole moved, an area nudged past the allowance or a
  loop reversed raises its variant. One nudged within the allowance is
  indistinguishable from rounding, and the twins table nudges past it.

## Alternatives considered

- **Orientation by turning number** (total tangent turning, ±2π): it is
  undefined, or ±2πk, on the crossing loops ADR-0012 lets through, and it
  needs tangent angles at every corner. The area's sign is defined on every
  loop. Rejected.
- **Flattening arcs to polygons** and measuring the polygon: simpler, and
  fully independent of the series, but the chord error depends on a vertex
  count. A closed form such as `7200 − 400π` would then match only to that
  error, which is no exact check. Rejected.
- **Comparing open-join loops at extraction's origin** (the first join):
  the two would agree by construction. The checker would only repeat
  extraction's arbitrary choice and could not catch an error in it.
  Rejected, in favour of naming the join.
- **Even-odd containment** (crossing count mod 2), as the plan first wrote
  it: it disagrees with extraction's nonzero rule wherever a crossing loop
  winds twice, so a correct extraction would fail its check. The signed
  count costs the same ray. Rejected.
- **An absolute allowance**, or one relative to the area: the first breaks
  at 1e±300; the second is zero for a loop of zero area, and arbitrary
  where a large outer minus holes is small. Rejected for one scaled by the
  sketch's own tolerance (ADR-0003).

## Addendum: the rounding term (2026-11-08, `plans/m10-profile-check` step 2)

Measured against closed forms in `laman-check`'s `area` module. The
allowance is `L·P + ½·D·Σg + ROUNDING·M`.

- `L` is not the loop's scale. A component is the parameters linked by shared
  equations (ADR-0003), so on a loop no equation reads each edge is measured
  alone, and `L` is its longest edge's tolerance. The expectation in D2
  holds for the near-straight arc all the same: an arc's own extent counts
  its centre. Worst error 1.6e-6 of `L·P`, radii 1e3 to 1e15, the arc up to
  1e6 from the origin.
- It fails on a long loop. Each cross product rounds by `ε` of its two
  products, of order `D²` however thin its triangle, so the error grows
  with the edge count and `L·P` does not. A regular polygon of
  unconstrained lines: 2.8e-3 of `L·P` at 500 edges, 2.2 at 50 000.
- So the allowance adds `ROUNDING = 4ε` times the sum's magnitude `M`:
  half the sizes of every product and partial sum the measure accumulates.
  About `ε·M` bounds one measure to first order; twice covers extraction's
  sum, whose terms are no larger, and twice again the second order and the
  two frames' exponents. Worst error measured: 0.070 of `ROUNDING·M` (the
  near-straight arc), 1.8e-4 on the 50 000-edge polygon.
- The term is relative to the measure's own sums, so it scales with the
  sketch as `L` does and holds at 1e±300. It loosens the check by about
  `4ε·n` of the area on an `n`-edge loop: 4e-13 at 500 edges.
