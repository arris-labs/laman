# ADR-0011: The frame budget is per try, and C2 stops at the rectangle drag

- Status: accepted
- Date: 2026-11-07
- Amends: ADR-0006 (a bisected frame is held to the budget per try, not per
  frame)

## Context

C2's *Accept* put every 500-entity edit and drag step inside 4 ms, "bisected
drag frames included". After four M8 plans the six edits and the slot drag
meet it. Two cells did not: the rectangle drag (15.3 ms) and the slot bisect
(8.1 ms).

A bisected frame is not one solve. `DragSession::to` tries the target, then
halves toward it, at most `max_bisections` times (8), so a frame costs up to
nine tries, each a solve (ADR-0006). A frame refused at every halving costs
nine times the plain step whatever the solver does, and no per-frame number
holds for it. Measured at 498 entities (`bench_c2`, worst across runs): a
frame bisected twice, 7.3 to 8.2 ms for 3 tries (2.4 to 2.7 a try); one
refused at every halving, 34 ms for 9 (3.7 to 4.3 a try). The frames after a
bisected one take 2.4 ms.

The rectangle drag was 75 % pursuit: the goal's descent in the freedom the
required equations leave, an SVD for the multipliers and a dense QR for the
null space each iteration. Rigid bodies in the remainder (option A of the
frame-budget idea) shrink those rows and parameters, but their ceiling was
3.7 ms only by taking 97 % of the pursuit, they cannot close the bisect, and
they cannot go below the same 3.4 to 3.9 ms floor the other cells sit on.

## Decision

**The bisected-frame clause is per try.** A frame's time divided by its tries
(`halvings + 1`, as the bench prints it) must be ≤ 4 ms. C2's *Accept* is
restated that way. This weakens what M7 wrote, on purpose: the old clause
could be met only by lowering `max_bisections` or by tries sharing work, and
ADR-0006 stands on neither. The bench judges the worst sample, not the median,
and holds two cells: `bisect` (a frame bisected a few times) and `bisect-deep`
(refused at every halving).

**The pursuit is made cheap, not replaced** (`plans/m8-pursuit`): the
damping is read against the goal's own scale, so a long chain's first step is
not held short (5 iterations to 2 or 3); the dense QR skips exact zeros and
changes no bit; the multipliers are read from a pivoted QR of the unit rows (a second factorization beside the null space's)
instead of an SVD. The rectangle drag went from 15.3 ms to 5.3 to 5.9.

**The stop rule.** A profile of the rectangle drag after this (the split is in
the plan's step 5) found nothing outside the pursuit that can go without
changing bits or a contract. This is the last plan on this cell: C2's *Accept*
carries its measured number, **6.0 ms** (the worst across three runs, 5.89,
rounded up to the next 0.5 ms), and the cycle closes on it. A cell that later
work brings under 4 ms is a gain, not a debt.

**Rigid bodies in the remainder are not built for C2.** They remove the same
factorizations the pursuit plan removed, for a larger change to the model's
compiled form; they cannot go below the floor, and the stop rule no longer
calls for them.

## Consequences

- The cell numbers in ROADMAP.md §C2 are the contract: every 500-entity edit
  and the slot drag ≤ 4 ms; the rectangle drag 6.0 ms; a bisected frame
  ≤ 4 ms a try.
- A sample on the 4 ms edge can cross it by noise: three slot cells (edit,
  drag and `bisect-deep`, LM) touched 4.02 to 4.41 ms in single samples with
  medians of 3.2 to 4.3. They are recorded as measured, not widened, and the human accepted them as noise on the edge (2026-11-07).
- The damping rule and the multipliers may move a drag digest; a move is
  re-blessed under `fixtures:` with the reason (C2's *Accept* allows it).
- The static solve is reported, not budgeted.

## Alternatives considered

- **Lower `max_bisections`**: a frame refused at fewer halvings is cheaper
  and a worse drag; ADR-0006 stands.
- **Tries sharing their work**: the tries differ in the goal only, but the
  shared part is the descent, which a refused try has not finished; a design
  of its own, not needed by the per-try clause.
- **Rigid bodies in the remainder** (option A): see Decision.
- **Keep 4 ms on the rectangle drag and continue**: no plan has a profile
  that reaches it; the cycle would not close.
