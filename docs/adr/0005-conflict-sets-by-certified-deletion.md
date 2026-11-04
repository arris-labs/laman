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

## Amendment 2026-11-02: every start before a certificate

**Search**, as first decided, let the first start settle a test: a test
returned unsatisfiable as soon as the re-solve from the caller's best
effort ended certified, and tried the sketch's own coordinates only when
that first re-solve settled nothing. The certificate is local, so a first
start that stops at a stationary point of a *satisfiable* subset was
certified, and the filter reported false conflicts: a set of eight true
distances that hold together at the guesses
(`false-conflict-rigid-framework`), and a set excluding the one
contradictory distance (`false-conflict-excludes-contradiction`).

A test now re-solves from every start, the best effort and then the
sketch's own coordinates. Any converged start is the satisfiability
witness, whatever another start certified. The subset is unsatisfiable
only when every start was tried, none converged, and at least one ended
certified. A budget spent before the last start is inconclusive. The
filter, the guarantee and the bound are otherwise unchanged. The caller's
own solve, which starts from the sketch's coordinates, stays the gate on
a failed component.

Cost: every test that ends unsatisfiable now spends both starts. The C1
conflicting twins spend 8 (triangle), 19 (belt) and 34 (plate) re-solves
under both engines, not 6, 14 and 19. `MAX_ANALYSIS_SOLVES = 256` already
budgeted both starts of every test (a 127-constraint component's filter
and rest), so it stands. DogLeg still ends `NotConverged` on both
regressions, as on `dogleg-conflict-inconclusive`; that is a separate
defect.

## Amendment 2026-11-04: every test re-solved by the default solve

A whole-path analysis (`laman::reference`, and the fallback of a failed
component, ADR-0009) re-solved each test whole. From the best effort such
a re-solve could stop at a certified stationary point of a satisfiable
subset where the staged re-solve of the same subset from the same start
converges, so the filter dropped a true distance and named a set that
holds together (`regression/whole-false-certificate`, a checker-clean
witness in the fixture). The same certificate's locality as the first
amendment's, met by a start the staged solve escapes and the whole one
does not.

Every test now re-solves by the default staged solve, whatever path the
analysis runs on. A staged solve that does not converge by parts falls
back to the whole solve from the same start with the caps restored
(ADR-0009 §Fallback), so it converges wherever the whole re-solve does,
and where neither converges it ends at the whole re-solve's coordinates
and certifies exactly what the whole one would. The whole-path test can
only gain witnesses. Each start is still one re-solve of the budget, so
the bound and the measured counts stand; the staged path's results are
unchanged bit for bit (it already re-solved staged), and so are the
digests. Both paths' analyses now differ only by the best effort they
start from, so the tests hold staged and whole conflict sets equal and
ADR-0009's witness exemption is withdrawn.

Rejected: a further start, the staged re-solve of the subset, after the
two whole ones. It certifies no less than this and spends up to twice the
re-solves for nothing the staged re-solve's own fallback does not already
try.

The certificate stays local: a subset both re-solves stop short of from
both starts is still certified. M7's exact-minimum oracle in the tests
measures what that leaves.

## Amendment 2026-11-04: every test re-solved by Levenberg–Marquardt

Under `Algorithm::DogLeg` the staged analysis of
`regression/staged-conflict-inconclusive` ended `NotConverged` with no set
where the whole path named the triangle-inequality conflict, and DogLeg
was inconclusive on `dogleg-conflict-inconclusive` and on both halves of
the first amendment's fixtures, where Levenberg–Marquardt proves the set.
Measured on the fixture, the cause is the engine, not the path: a filter
test of a subset with as many equations as it reads parameters that cannot
hold (there, {`leg-a`, `leg-b`, `span`} with the distance from the fixed
origin to the far end) has its least-squares point where the Jacobian is
singular. There the Gauss–Newton step is unbounded, every dog-leg step is
cut to the radius in the span of the gradient and that step, and the
stiff directions are corrected only by the Cauchy step: the descent
converges linearly and needs about 200 iterations, LM's damped step 85.
At the 128-iteration cap every DogLeg re-solve of the subset ended 2–5×
above `STATIONARY` (2.2e-6, 4.6e-6). Neither path proved it reliably: the
whole one concluded only because its start, the best effort, differed
(the budget share an earlier, satisfied component left, 2 iterations
staged against 4 whole, ADR-0009 §Budgets). Giving each component its own
caps made both paths inconclusive.

Every test now re-solves by Levenberg–Marquardt, staged, whatever the
caller's algorithm, under the caller's caps. The test asks whether a
subset holds together, which is the sketch's, not a property of the
caller's step; the caller's algorithm still makes the best effort a test
starts from, the gate on a failed component and every coordinate a solve
returns. Under LM nothing changes, bit for bit; the C1 twins' sets and
re-solve counts are unchanged under both engines, and so are the
digests. `staged-conflict-inconclusive`, `dogleg-conflict-inconclusive`
and the `*_dogleg` halves are lifted, and the tests hold staged and whole
conflict sets equal with no exemption: both sides inconclusive is the
only case left without an answer, and then both name nothing.

Rejected: a dog leg that converges there, which must leave the span of
the gradient and the Gauss–Newton step (the trust-region step of the
Gauss–Newton model is an LM step with its damping set by the radius), so
it would make `DogLeg` a second LM; re-solving with the caller's
algorithm and then LM where the first settles nothing, a fallback that
spends up to twice the re-solves on every unsatisfiable test; a raised
cap or a wider `STATIONARY`, which the slow singular approach to a true
solution bounds (above).

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
