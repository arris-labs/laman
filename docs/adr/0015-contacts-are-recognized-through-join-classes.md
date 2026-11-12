# ADR-0015: Contacts are recognized through join classes

- Status: accepted; amends 0002
- Date: 2026-11-13
- Scope: C4 (`plans/m12-intersections` step 1)

## Context

ADR-0002 recognizes a tangent contact only from shared ids: a line end that
is the arc's own start or end, or that carries an explicit `point_on_circle`
of the round. A sketcher that draws a slot as separate lines and arcs and joins
each line end to an arc end by `coincident` gets no contact: the tangent
keeps its raw equation, the signed distance from the centre to the line
minus the radius. Together with membership, which pins the line end to the
circle, that equation has a double root at the contact: the line end slides
along the circle by √(residual · r) for a residual inside the length
tolerance. regression/`coincident-contact-drift` (the slotted plate of
`c3/slotted-plate` with eight `coincident` joins) solves `FullyConstrained`
and checker-clean, its contact points up to 3.7e-5 from the closed form,
against 1e-11 for the shared-point plate. C4 classifies a touch between two
edges, which cannot be done at √residual.

ADR-0012 D1 already defines when two points are one place by declaration:
the join class, union-find over `coincident`. Profiles read it; the solver
did not.

## Decision

**Recognition.** A tangent contact is recognized first as ADR-0002 says, by
shared id, unchanged: the lower line end that is the arc's start or end,
else carries an explicit membership (lowest constraint id). Failing that,
through join classes: the lower line end whose join class holds the arc's
start, else the arc's end, else a point with an explicit membership of the
round (lowest constraint id). The contact's point is the line end; its
*member* is the joined point whose membership holds. A sketch that had a
contact before keeps the same one, so no shared-point solve changes.

**Equation.** The regularized equation is ADR-0002's, unchanged:
`h = dot(t, p − c)` with `p` the line end, its gradient
`dh = t·(dp − dc) + ((v − t h)/|u|)·(db − da)`, the stored side condition
`σ cross(t, c − p) > 0`, the original signed tangent residual checked before
acceptance. The `coincident` equations and the member's membership
(explicit, definitional for an arc start, the intrinsic equation for an arc
end) are all retained.

**Proof.** ADR-0002's proof needs membership of `p`: `|p − c| = r`. Here the
retained equations give `|m − c| = r` for the member `m` and, along the
chain of `coincident` constraints linking `p` to `m`, `p = m`; together
`|p − c| = r`. From there ADR-0002's argument holds word for word: `h = 0`
and membership give `|cross(t, c − p)| = r`, the side condition selects
`σ r`, and signed tangency with membership gives `h = 0`. The zero sets of
the original and the regularized equations are equal on the nondegenerate side
domain. A `coincident` that is violated breaks membership of `p` exactly as
a violated membership does under ADR-0002: the solve's status and the
checker report it, and the original tangent residual is still checked.

**The other option, recorded.** The plan's fallback was a last step on the
regularized equation before acceptance, keeping raw recognition. It is not
taken: it would leave diagnostics (DoF, redundancy, conflicts) reading the
raw Jacobian, and the closer the last step brings the contact to its
double root, the nearer that Jacobian's rank comes to the drop ADR-0002's
table shows for shared ids (the `coincident` equations are linear and carry
the parallel gradients over unchanged); it would also add a second
acceptance path. Recognition through join classes makes the solve and the
diagnostics read one set of equations, as ADR-0002 does.

## Consequences

- regression/`coincident-contact-drift` solves to the default coordinate
  tolerance with both algorithms (7.4e-11 at worst, from the plate's outer
  corners; the contacts within 4e-11), checker-clean. Its ignore is lifted.
- The solved digests of `c3/slotted-plate-coincident` move, the only C1 or
  C3 digests that do: its solve now ends at the closed form.
- Removing a `coincident` recompiles recognition: the contact is dropped
  with the join, as removing a membership drops it under ADR-0002.
- A tangent whose line end is joined to a point merely lying on the round
  by another tangent, or near it in the solution, is still not a contact:
  only a declared join carries membership, never a measured one
  (ADR-0012).

## Alternatives considered

- **Last step before acceptance** (the plan's fallback): above.
- **Tighter stop**: a residual tolerance `ε` leaves the contact at `√(ε r)`;
  reaching 1e-11 at r = 5 asks `ε` near 1e-23, below f64's resolution of
  the residual. Rejected.
- **Substituting the member for the line end in `h`**: an equal zero set,
  but the equation then reads a point the original tangent does not, so its
  support differs from the equation it replaces; with the line end it reads
  the line's ends and the centre, as in ADR-0002's shared case. Rejected.
