# ADR-0005: Conflict sets by certified deletion, redundancy by removal

- Status: accepted
- Date: 2026-11-01
- Closes: SEED.md §9 / §10.8, whether the minimal conflict set is exact or
  a bounded approximation

## Context

A conflict is a set of constraints that cannot hold together. Numerically
that is never proved by a failed solve: a solve stops at its caps, at a
singular point, or in the wrong basin, and none of these is a
contradiction (DATA-MODEL.md §Diagnostics). Local linear dependence is not
one either: redundant twins are dependent and consistent. A caller (a human
or an agent) acts on the set it is shown, so a set that is not minimal
blames a constraint that could stay, and a set that is not a conflict at
all sends the caller after a ghost. Exact minimum-cardinality sets are a
combinatorial search over subsets, beyond any frame budget. No reference
solver was read for this decision.

## Decision

**Unsatisfiable, the certificate.** A component's equations are certified
unsatisfiable at coordinates where they are evaluable (no degenerate,
non-finite or off-side contact equation), not within tolerance, and
stationary for least squares: with the solver's scaled residual `f` and
Jacobian `J` (ADR-0003, contacts regularized by ADR-0002),
`|Jᵀf| ≤ STATIONARY · |(f_i |J_i|)|`. This says that `f`, weighted for unit
rows, is a left null vector of `J` to the threshold. No first-order motion
reduces the residual, and the dependent rows disagree. A component with no
free parameter is certified exactly by its value. `STATIONARY = 1e-6`
(`model::tolerance`), between two measured limits:

- an iteration that compares costs finds a stationary point only to a few
  `√ε`. The C1 conflicting twins stop at 1.6e-10 to 3.2e-8;
- a zero-residual solution reached across a quadratic singularity keeps a
  ratio near `√(2 · LENGTH_RELATIVE)` = 4.5e-6 while its residual is still
  above tolerance. A slow solve that would succeed is never certified
  (`slow_singular_approach_is_never_certified`, at every iteration cap).

The certificate is first-order and local. It shows that no solution lies
near the least-squares point the solve reached. It does not rule out a
solution in another basin.

**Satisfiable, the witness.** A re-solve that converges, meaning every
equation within its acceptance (ADR-0004), proves that its constraints hold
together. That proof is global.

**Search.** For each failed component, in compiled order:

1. The caller's best effort must be certified. Otherwise the component's
   constraints alone are re-solved (the caller's budget is shared across
   components, so one component's rejections can starve the next), and
   that re-solve must be certified.
2. Run a deletion filter in source order. Drop a constraint when the set
   without it is certified unsatisfiable. Keep it when the set without it
   converges: that converged re-solve is its removal witness. Any other
   outcome is inconclusive.
3. Record the set. Set aside its greatest id and test the rest. If the
   rest is still certified, filter it again. Stop when it converges, when
   a test is inconclusive, or when the budget is spent.

Each test re-solves a copy of the sketch that holds only the tested
constraints. Contacts, components and scales are recompiled for the copy.
The test starts from the caller's best effort, then once more from the
sketch's own coordinates if the first start settles nothing. Intrinsic
equations and ground are part of their entities, so a test never removes
them and a set never names them. A `fix` is a constraint and can be named.

**Guarantee.** Every reported set is **inclusion-minimal**: removing any
one member leaves a set the solver satisfies (its witness), and every
subset of a satisfiable set is satisfiable. Its unsatisfiability rests on
the local certificate. Minimality is about inclusion, not cardinality.
Not every conflict is listed, and sets may overlap. An inconclusive test
withdraws that component's unfinished set. If no set stands, the status is
`NotConverged`, never `Unsatisfiable`. Sets are deterministic: they follow
source order, and the re-solves are deterministic.

**Bound.** `MAX_ANALYSIS_SOLVES = 256` re-solves per analysis, each under
the caller's own iteration and evaluation caps. Running out is
inconclusive. Measured: the C1 conflicting twins spend 6 (triangle), 14
(belt) and 19 (plate) under both engines, one re-solve per constraint of
the failed component plus one for the rest after the set. Cycle 2 measures
the bound again on its 100–500-entity sketches.

**Redundant.** A satisfied constraint is redundant when it lies in a
satisfied component and removing it, with contacts and scales recompiled,
leaves the geometric DoF unchanged. Constraints are tested in reverse
source order, and a constraint found redundant stays removed for the
tests after it. So of two duplicates the later is named, and a constraint
implied only together with another is named once. A constraint that is
only partly implied (its removal frees fewer DoF than it has equations) is
not named. Redundancy costs no solve. A satisfied fixed-only constraint is
redundant: the ground implies it.

**Degenerate.** At the solution, a line shorter than its component's
length tolerance, a circle of radius below it, or an arc whose centre or
end meets its start is degenerate. It is measured with
`Tolerances::at`, like the checker, but independently of it. Each
degenerate entity carries the equations that read it. Separately, every
equation whose residual is undefined or non-finite there is listed. A
status never changes because of degeneracy, but an invalid evaluation
certifies nothing.

## Consequences

The C1 belt and plate twins get exactly their roadmap sets. For the SSS
triangle the filter returns {`base`, `right`, `apex`}, not the roadmap's
four. With `|oa| = 50` and `b` on the line through `o` at 60° to `oa`, `a`
lies `50 sin 60° = 43.3 > 40` from that line, so `right` cannot hold
whatever `left` says. The four-member set is the support of the
least-squares residual, not a minimal set. The fixture's expectation is a
question for the human (plans/m2, Open questions).

Status follows geometric acceptance at the coordinates: a component within
tolerance is satisfied even if the solve stopped short of its stricter
polishing target. A conflict anywhere makes the status `Unsatisfiable`,
whatever other components do.

A conflict that the least-squares descent can only approach through a
degenerate configuration (the line of a tangent shrinking to nothing)
certifies nothing and stays `NotConverged`
(`degenerating_subset_is_inconclusive`). That is the price of never
blaming a ghost.

## Alternatives considered

- **Failed solve or rank deficiency as proof**: both hold for consistent
  sketches.
- **`RANK` as the stationarity threshold**: cost comparison cannot reach
  it, so true conflicts went uncertified (triangle 3.2e-8).
- **Exact minimum cardinality**: exponential in the constraints, and not
  what a caller needs: any set it can act on, with every member necessary.
- **Keep a member on an inconclusive test**: it reports a set whose
  minimality is unproved.
- **Forward redundancy order**: names the earlier of two duplicates, and
  names a coincidence before the point-on-line it implies.
