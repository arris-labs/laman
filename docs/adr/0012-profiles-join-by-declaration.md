# ADR-0012: Profiles join by declaration

- Status: accepted
- Date: 2026-11-07
- Scope: C3 profile extraction (`plans/m9-profiles`)

## Context

C3 turns solved coordinates into closed loops and holes. Its roadmap left
open whether two distinct points that coincide in the solution join a loop,
or only a shared point id does. ADR-0002 is the precedent for reading
coordinates: structural rank is read from them, so a tangent contact is
regularized where the sketch declares it. A join differs: rank is a
property of the equations at a configuration, but a join is a statement
about what the caller drew, and nothing in the equations can say it
better than the caller did. A measured join would also depend on a
tolerance, on the solve's residual and on scale, and would make a nearly
closed loop flip between open and closed with a drag frame.

## Decision

**D1. A join is declared.** Two edge ends meet when they are the same point
id, or when a chain of `coincident` constraints links their points. The
classes are found by union-find over those constraints, each class
represented by its smallest point id. A join is never measured.

A `coincident` that the solution violates still closes its loop, and the
loop names that constraint in `joins`: the solve's status already reports
the violation, and extraction does not second-guess it.

**D2. Topology.** An edge is a non-construction line or arc; its ends are
join classes. A non-construction circle is a loop by itself. Edges are
pruned at degree-1 classes, repeatedly, in id order. A component pruned
away entirely is an open chain. A pruned edge whose component keeps a core
is a dangling edge. A core in which every class has degree 2 is a loop,
ordered from its smallest edge id. A core with a class of degree above 2 is
branched (a self-touching loop, or edges shared by two loops): its edges and
those classes are reported and none of its edges enters a profile. An edge
with both ends in one class is degenerate: reported and left out.

**D3. Geometry.** Area is Green's `½∮(x dy − y dx)` in loop order, on
coordinates translated to the loop's first join and scaled by an exact
power of two; a line gives its cross product, an arc its chord plus the
segment `r²(θ − sin θ)/2`, with a small-θ form below a threshold measured
when the kernel is written. The area saturates at ±`f64::MAX`. Orientation
is the sign of that area. Containment is the winding number at the midpoint
of the inner loop's first edge. Even depth is an outer boundary
(counter-clockwise), odd a hole (clockwise); a hole's parent is its
innermost container, and an island is its own profile.

**D4. Unjoined coincidences.** Among open ends only (degree-1 classes), a
pair whose points lie within the smaller of the two edges' length
tolerances (`Tolerances::at`, ADR-0004; no new constant) is reported with
both point ids. It never joins them: it tells the caller the join they
probably meant is missing.

## Consequences

- Extraction reads only what the caller wrote plus coordinates for area,
  orientation and nesting; the same sketch gives the same loops at every
  frame of a drag.
- A sketch drawn with two ground points at one corner has an open corner
  until the caller adds a `coincident` or shares the point; D4 names it.
- Edges shared by two loops are reported as branched, not split into faces
  (backlog).
- The result types live in `laman::model` so `laman-check` can name them
  under the layer rule.

## Alternatives considered

- **Measured joins** (ends within tolerance join): the loop set would change
  with the solve's accuracy and with scale, and a drag would flicker between
  open and closed. Rejected.
- **Shared point id only**: simple, but a sketcher that uses `coincident`
  between distinct points, the usual way to join two drawn segments, would
  get open chains for closed shapes. Rejected.
- **Declared joins with a measured warning** is D4: the warning is a report,
  never a join.
