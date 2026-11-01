# ADR-0002: Regularize structural tangent contacts

- Status: accepted
- Date: 2026-11-01
- Scope: C1 geometric DoF; general singular rigidity is deferred

## Context

At a tangent contact, circle membership and signed line tangency have
parallel gradients. Raw Jacobian nullity therefore overcounts freedom.
`crates/laman-debug/tests/numerical_contract.rs` evaluates the original
analytic gradients at the checker-clean, closed-form C1 coordinates:

| Fixture | Free parameters | Original rank | Contact rank |
|---|---:|---:|---:|
| SSS triangle | 4 | 4 | 4 |
| Belt | 12 | 8 | 12 |
| Slotted plate | 22 | 18 | 22 |

These are analytic contact fixtures, not solved iterates. The experiment
uses ordered, twice-reorthogonalized unit rows, independently of the
production factorization. No reference implementation was consulted.

## Decision

Recognize contact only from shared entity ids: a line endpoint `p` belongs
to a round entity through an explicit `point_on_circle`, or is that arc's
start or end. Do not infer membership from nearby coordinates, coincidence
constraints, or arbitrary chains of constraints in C1.

Write `u = b-a`, `t = u/|u|`, `v = p-c`. Replace the tangent equation by
`h = dot(t,v) = 0`, retaining membership and the stored side condition
`σ cross(t,c-p) > 0`. Check the original signed tangent residual before
accepting convergence; the new equation alone is insufficient. Reject a
trial crossing the stored side or entering degenerate geometry. A start
already outside this domain cannot be certified by the reformulation;
retain finite best effort and report degeneracy/NotConverged unless a
conflict is independently established. Never silently switch senses.

**Proof.** For `|u| > 0`, radius `r > 0`, and membership `|v| = r`, the
orthonormal tangent/normal decomposition gives
`dot(t,v)^2 + cross(t,v)^2 = r^2`. Signed tangency
`cross(t,c-p) = σ r` implies `h = 0`. Conversely `h = 0` and membership
imply `|cross(t,c-p)| = r`; the side condition selects exactly `σ r`.
This is equivalence of zero sets on a nondegenerate side domain, not
of residual magnitudes away from a solution.

For a circle, retain its explicit membership equation and radius parameter.
For an arc start, membership is definitional (`r = |s-c|`). For an arc
end, membership follows from the retained intrinsic equation
`|e-c| - |s-c| = 0`. The proof applies to both; dropping the intrinsic
equation changes the geometry. Reflected circle and both arc-endpoint
fixtures satisfy `h` but fail exactly the stored-sense tangent check.

The analytic differential is
`dh = t·(dp-dc) + ((v-t h)/|u|)·(db-da)`.
Merge contributions when ids coincide. Its finite-difference experiment
covers both line endpoints, scales 1e-6 through 1e3, translated coordinates
and short lines with coordinate/local-scale ratio through 1e6.

Keep one equation per original tangent. If several endpoints qualify,
choose the lowest endpoint id, then lowest explicit membership constraint
id; retain every membership equation. Two distinct line endpoints on the
same nondegenerate tangent circle cannot both be contacts. Do not erase
that contradiction by dropping membership.

Each replacement carries the tangent id and its membership provenance:
explicit constraint id, arc definition, and (for the end) intrinsic source.
Removing a constraint recompiles contact recognition. Original tangent
row dependence alone is not redundancy. Deleting membership or tangency
can restore actual freedom even when the original rank does not change.

## Consequences

The solver and diagnostics use the same regularized system; the checker
continues to measure original geometry independently. This is not a new
public constraint kind. The contact equation has length units.

DoF means geometric freedom at regular configurations and these C1
contacts. Elsewhere it remains an infinitesimal estimate, not a general
singular-rigidity theorem. Per-entity DoF and free directions inherit this
limit. Runtime recognition, provenance and deletion tests land with the
compiler; the experiments in this decision introduce no solver behavior.

## Alternatives considered

- Raw Jacobian nullity: contradicts the analytically fixed C1 contacts.
- Lower rank threshold: cannot restore a direction that is exactly zero.
- Extra contact rows without provenance: can falsely blame an essential
  tangent as redundant and invalidate constraint-removal analysis.
- Full higher-order rigidity: beyond the approved C1 scope.
