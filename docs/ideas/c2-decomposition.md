# Idea: c2-decomposition

- Status: Accepted (C, S1); M5 and M6 done (ADR-0009); M7 absorbed by plans/m7-drag-conflicts; M8 remains
- Raised: 2026-11-04
- Prompt (verbatim from the human): "c2 (or you can directly plan first thing from it?)"

## What remains

M5 (corpus and bench) and M6 (stages, ADR-0009) are done; the options,
the Round 1 decision (C with S1) and M6's design are in git history,
ARCHITECTURE.md and ADR-0009. M7 is planned (plans/m7-drag-conflicts);
one milestone remains (ROADMAP.md §C2):

- **M8 — the frame budget at 500 entities**: decided below.

## After M6's bench (2026-11-04): does M8 run?

The numbers are in ARCHITECTURE.md §Frame budget
(release, Ryzen 9 7950X). Stages took the 498-entity cells from not
finishing in 5 s to 11-34 ms, and the 98-entity cells to 1.1-2.8 ms, but
the acceptance is **missed**: one-dimension edit medians at 500 entities are
11.8 / 22.5 / 11.7 ms (rectangles / slots / triangles, LM; DogLeg 11.4 /
22.5 / 10.8), 2.8-5.6 times the 4 ms target; the first drag step is 20 ms on
both families that have one (5 times the target). The 249-entity cells are
5.0 / 7.9 / 4.0 ms. C1 holds its 1 ms budget (plate 0.33-0.46 ms, drag steps
0.39 ms).

**What the numbers say about the options.**

- **A (rigid bodies in the remainder)** attacks the remainder's dense
  solve. Nothing measured shows the remainder dominating: the edit, which
  moves a stage and what depends on it, costs as much as the drag step,
  which moves the remainder. M6 attributed the edit's time to two
  things that are not a remainder solve: every part's evaluation measures
  the whole component's extent for acceptance (O(n^2) a solve), and
  redundancy compiles and cuts the placed sketch again (5-13 ms). The
  248 to 498 scaling (2.3-2.9x for 2x) fits a quadratic term over a linear one.
- **S2 (a cache)** would hide the recompile and re-cut on re-diagnosis, at
  the price of a public type and a second path that can disagree with the
  first, and does not touch the extent term. A drag step already reuses its
  decomposition (ADR-0009), and still costs 20 ms.
- **A driver split of the remainder** helps only when the remainder is
  large; these corpora's remainders are small next to their stages.
  Not indicated by anything measured.
- **Removing the two measured cost sources first** (the extent computed
  once per component and carried, not per part evaluation; redundancy
  reusing the compile and cut of the placed sketch instead of making
  them again) costs no public type, keeps one path and S1, and names the
  term the data points at.

**Recommendation.** M8 runs, but not as A, S2 or the driver split: as a
**profile-then-remove step** on the two named sources, with the bench as its
acceptance, and A, S2 or the driver split reconsidered only if the cells
still miss after it. Caveat, stated plainly: M6's closing bench timed the
cells and did not profile; the attribution to the two sources is from M6's
own measurements, and the first step of M8 confirms it with a profile
before cutting. The acceptance (≤ 4 ms at 498/498/501 entities, both
algorithms) is unchanged.

## Decision (M8)

2026-11-04, taken by the agent (tier 3) at M6's retirement: the human
ruled the questions technical.

- **M8 runs as profile-then-remove.** Its first step profiles the
  500-entity edit and drag step; the plan then removes what the profile
  confirms (the per-part extent measurement, the diagnosis's recompile
  and re-cut, the repeated linear scans the retirement review found in
  the head search and the continuation), with the bench as acceptance.
  Reason: both measured sources sit outside the remainder's solve, which
  is what A and the driver split attack, and S2 would hide only one of
  them behind a public type and a second path; removing them keeps one
  path, S1 and the public API.
- **The 4 ms target holds for the drag step at 500 entities too**, not
  only the edit. Reason: ROADMAP.md §C2 *Accept* states both, and the
  drag is the most frame-bound operation a sketcher has (SEED.md §6); a
  removal that reaches 5–10 ms and stops would leave the cycle's goal
  unmet. If M8's first plan misses, the profile it took chooses between A,
  S2 and the driver split in a second M8 plan; the target is not relaxed
  by a plan.
- The miss is recorded as missed (ROADMAP.md §C2 status,
  ARCHITECTURE.md §Frame budget), with the measured numbers.
