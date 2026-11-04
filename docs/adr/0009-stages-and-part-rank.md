# ADR-0009: Stages, part rank and the whole-component fallback

- Status: accepted
- Date: 2026-11-04
- Closes: nothing from the kickoff; M6 of cycle C2 (docs/ROADMAP.md,
  ideas/c2-decomposition: option C with S1)
- Amends: ADR-0003's rank (the row normalization of a part's QR)

## Context

Every solve and every diagnosis was dense over a whole component: LM's
Cholesky and the diagnostic QR are cubic in its free parameters, and the
redundancy test repeats a rank per constraint. A grounded 500-entity
sketch is one component of about 1 000 parameters, beyond C2's 4 ms frame.
An edit also moved geometry it could not reach: a whole-component descent
re-solves everything, so an upstream point drifted by up to 248 ULP
(`regression/upstream-edit-drift`). The diagnostics are numbers (ADR-0002,
ADR-0003, ADR-0005) and a decomposition may make them cheaper, never
different. Laman's unknowns are absolute coordinates with ground fixing
the frame, so the compiled system can be cut directly, without rigid
bodies. No reference solver was read for this decision; the algorithms
are from the papers cited in `stage.rs` and `solve.rs`.

## Decision

**Parts.** Each component's required equations and free parameters form
a bipartite graph, an edge where an equation's gradient has a term (the
term keys, never their values; a contact's regularized terms are its four
points, ADR-0002). A maximum matching (augmenting paths in compiled order,
the smallest parameter first) gives the coarse Dulmage–Mendelsohn split
(Dulmage & Mendelsohn 1958): the **head**, rows an unmatched row reaches
by alternating paths; the **remainder**, columns an unmatched column
reaches; the square, perfectly matched middle. Tarjan's strongly connected
components of the middle (Tarjan 1972) are the **stages**, in dependency
order, ties to the smallest parameter. A constraint's equations, and an
arc with its intrinsic equation, never split: parts sharing one merge and
the graph is condensed again. Order is head, stages, remainder; each part
reads only its own and earlier parts' parameters. A part is a `Component`
subset carrying its parent's frozen scale, origin and acceptance, so no
tolerance changes (ADR-0003). `solve` stays stateless and cuts every call
(S1); a `DragSession` cuts once at `drag` and reuses the cut for every
frame and bisection, used only for the component whose parameters and
sources it was cut from: the equations it solves never change, and the
cut reads coordinates only where an equation is degenerate (coincident
ends: no gradient where it stands). Such an equation is read at a fixed
generic position, and failing that as everything it could read, so a
cut is coarser there, never wrong. The kept cut is the one a frame
would make while the dragged sketch's degenerate equations stay the same.

**Part rank.** Order the scaled Jacobian's rows and columns by part. It is
block lower triangular: block row `i` reads only columns of parts `≤ i`.
Write `J_ii` for the diagonal blocks, `k` parts in all.

- *Null space.* Let `J v = 0`. Block row 1 gives `J_11 v_1 = 0`; if
  `J_11` has full column rank, `v_1 = 0`. Then block row 2 reduces to
  `J_22 v_2 = 0`, and so on: when every part before the last has full
  column rank, `v_i = 0` for `i < k` and `J_kk v_k = 0`. So the null space
  is the last part's, padded with zeros, `rank J = Σ rank J_ii`, DoF and
  the free directions come from per-part QRs, and per-entity DoF from
  that basis (ADR-0003's construction, on the last part's columns).
- *Redundancy.* A constraint is redundant when removing it leaves the
  rank, that is when some left null vector `yᵀ J = 0` is nonzero on its
  rows. Take the block columns from the last: if every part after the
  head has full row rank, `y_k = 0`, then `y_{k−1}ᵀ J_{k−1,k−1} = 0`
  gives `y_{k−1} = 0`, down to the head. Every left null vector lives on
  the head's rows, so only a constraint with a head equation can be
  redundant, and a square stage or the remainder never is.

Both hypotheses are numerical, so they are checked at the coordinates,
never assumed from the structure. A part before the last that is column
deficient, or an entry that breaks the triangular pattern, sends the
component to the whole factorization. A part after the head with a row
dependent on the others (a point on a line through a point it coincides
with: the structure hides it) sends the component to the full redundancy
test. Otherwise a constraint without a head equation skips the deletion
test, which saves the factorizations; each test still compares the whole
DoF.

**Normalization (amends ADR-0003).** Rows are unit-normalized over the
whole row, as ADR-0003 says, then cut to the part's columns; the cut row
is not normalized again. A row that depends on earlier parts carries only
rounding on its own columns (1e-17 of a unit row); normalized over those
alone it became a unit row and counted a rank the whole factorization
does not (seed 226646: DoF 2 against 3,
`a_row_dependent_on_earlier_parts_adds_no_rank`). Cut, it stays below
`RANK`. The rank threshold and its meaning are unchanged.

**Only satisfied components are staged.** A failed or degenerate
component is the whole path's, in its solve, its freedom and its conflict
search. Staging unsatisfied best efforts gave projector gaps to 8e-3; with
satisfied components only, 60 000 random solves gave DoF and per-entity
DoF equal at every magnitude and projectors within 4e-14. A staged solve
keeps its result only where the diagnosis would stage it
(`diagnose::stageable`), so a solve and its analysis take one path.

**The staged solve.** Parts are descended in order, earlier parts'
parameters constant, under the shared budgets. A satisfied part takes no
step and keeps every bit, so an edit moves only its part and the parts
that read it (`upstream-edit-drift` lifted, LM and DogLeg). Goals are
pursued in the last part only (a stage when there is no remainder): the
required null space lies in its columns, so a goal on an earlier part's
parameter is stationary, as its null projection was.

**Continuation.** A part that held at the component's start and is moved
only by earlier parts starts from the first-order continuation of
`solve.rs::continued`: the Gauss–Newton step of its equations linearized
at the start with the earlier parameters at their new values,
`δ = −J_x⁺ (F + J_u Δu)` (the Euler predictor; Allgower & Georg 1990). A
geometric equation is unchanged under a translation of all it reads, so a
translated parent carries the part exactly, and a rotation by `θ` errs by
`θ²` times the part's reach. One evaluation; a fresh solve or an edited
part is descended as it stands, so no digest moved. Without it the
250- and 500-entity strips' edits took a mirror root: a descent from
where the part stood, left behind by parents that moved further than the
part's own size (point 70 of 84: 32.9 from the answer, side 20, the exact
mirror to 4e-8).

**Budgets in whole-component work.** `max_iterations` and
`max_evaluations` count whole-component units: a step or evaluation of a
part spends its share of the component's parameters, one of a component
solved whole exactly one, so the whole path's counts and bits are
unchanged. Each descent is also held to the cap alone. Counting each part
as one, 147 parts spent the shared 128 iterations and fell back whole. A
component's acceptance measures its extent at the coordinates
(ADR-0004's policy); the structure that extent is measured over (points,
radii, stated dimensions, pins, arcs) is a `Frame` found once at
compilation and shared by its parts, so a part's acceptance reads the
component alone, never the sketch, and gives `extent`'s number.

**Fallback.** A part that does not converge, or a result the diagnosis
would not stage, sends the component back to the whole solve and
analysis, from the component's start, with the budget counters restored
to their values there. The grain is the whole component: every failure
keeps the whole path's bits, conflict sets and best effort, and failures
are outside the frame budget. A component then costs at most twice its
budgets' work.

**The reference hook.** A non-default feature `reference` exposes
`#[doc(hidden)] laman::reference::{solve, diagnose, downstream}`: the
whole path's solve, its diagnosis at given coordinates with no step taken
(staged and whole coordinates differ within acceptance, so comparisons
diagnose the staged coordinates whole), and the parameters a change of one
constraint can move by the structure (its part and every later part that
reads one, transitively). It is for laman's own tests, outside the
stability promise and the binding; the default build's public API is
unchanged.

**Conflict sets against the reference.** The whole path's deletion filter
can certify a stationary point of a satisfiable subset (ADR-0005's
certificate is local) and name a set that holds together, where the
staged re-solve of that subset converges and keeps the constraint
(`regression/whole-false-certificate`, a checker-clean witness in the
fixture). Staged conflict sets are held to the reference's only where the
reference's sets are unsatisfiable: a set a converged, checker-clean
re-solve disproves is the reference's error, not a staging one, and
staged sets keep ADR-0005's own guarantee. The tests hold it so: staged
and whole sets may differ only where every whole set the staged path
lacks has a checker-clean witness (the set alone, or the sketch without
one constraint outside it, from the guesses or the best effort), and a
an analysis inconclusive on either side (`NotConverged`, no set) gives
no pair of answers to compare: the staged one proves sets the whole one
does not, and under DogLeg the reverse occurs (`staged-conflict-inconclusive`
in the backlog, M7's conflict work). That is
not a weakening: the witness is the independent checker's, so equality
there would assert a known-false answer, and the case that found it runs
on every gate from its fixture. Fixing the whole path's filter
is M7's conflict work (conflict sets against the exact minimum), not a
precondition of staging.

## Consequences

- An edit keeps the bits of every parameter outside its stage's
  downstream closure; `c2.rs` and `c2_props.rs` compare them by bits, not
  tolerance, and `reference::downstream` gives the closure.
- **Digests re-blessed**, with these reasons and no other: the
  slotted-plate drag's solved and ten drag-step digests (both algorithms),
  its free directions now the part-wise null basis, same span, other bits;
  the solved digests of the sss triangle, belt, slotted plate, its
  redundant twin and its drag (both algorithms) and all twenty drag steps,
  the staged descent taking other steps to the same solution, within
  1e-10 of the whole path's and the expected coordinates. The conflicting
  twins and the sss and belt redundant twins do not move (fallback);
  `violations.json` is unchanged. Native, wasm32-wasip1 and pytest
  digests are equal.
- What `max_iterations` and `max_evaluations` count changes for a staged
  component (their rustdoc says so); no public type or signature changes.
- Each part's evaluation still measures its whole component's extent, so
  a solve is O(n²) in a component's size, and redundancy cuts the placed
  sketch again. Measured, C2's frame budget is **missed** at 500 entities
  (edits 10.8–22.5 ms, the first drag step 20 ms, against 4 ms;
  ARCHITECTURE.md §Frame budget). The 4 ms target holds for both the edit
  and the drag step; M8 profiles and removes these costs first, keeping
  this decision's one path (ROADMAP.md §C2).
- A floating or under-constrained region is one remainder, solved whole;
  rigid bodies in it (option A) or a cache (S2) are M8, only if measured.

## Alternatives considered

- **A generalized pebble game with rigid bodies** (ideas/c2-decomposition,
  option A): true rigid clusters, floating ones included, but a second
  unknown kind through `System`, the engines and the diagnostics, and
  counting failures on angle-only systems. Deferred to M8, only if
  measured.
- **Normalizing a part's rows over its own columns** (the plan's first
  pick): blows rounding of a dependent row up to a rank (seed 226646).
- **Staging failed components too**: projector gaps to 8e-3; a failure's
  best effort is not where part rank's hypotheses hold.
- **A finer fallback** (re-solving only the failed part and its
  dependents): failures would lose the whole path's bits and conflict
  sets, for speed outside the frame budget.
- **Each part spending one unit of the caps**: 400 small parts exhaust a
  cap sized for one component and fall back whole.
- **Instead of the continuation**: a joint solve of the downstream
  closure costs the whole tail; a branch check with a fallback costs the
  whole solve exactly where edits are; a plain descent from where the part
  stood takes the mirror root.
- **Holding staged conflict sets to the reference everywhere, or fixing
  the whole filter first**: the first holds staging to a known false
  certificate; the second pulls M7's conflict work into M6 to correct a
  path staging does not use for that answer.
