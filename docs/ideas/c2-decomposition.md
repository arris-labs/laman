# Idea: c2-decomposition

- Status: Accepted (C, S1); M5 done; M6 absorbed by plans/m6-stages; M7, M8 to follow
- Raised: 2026-11-04
- Prompt (verbatim from the human): "c2 (or you can directly plan first thing from it?)"

## Problem

C2's acceptance (ROADMAP.md §C2) asks for a drag step and a one-dimension
edit on 500-entity sketches in ≤ 4 ms, bits outside the edited part kept,
and DoF, redundant and conflict sets equal to a whole-sketch solve's.
Today every solve is dense over a whole component (ARCHITECTURE.md §Solve):
LM's Cholesky and the diagnostic QR are cubic in its free parameters, and
redundancy repeats a rank computation per satisfied constraint. The C1
plate (22 parameters) already spends 0.9 ms; a grounded 500-entity sketch
is one component of ~1 000 parameters, roughly 10⁵× that work. Nothing
works at C2's sizes until a component is cut into parts that solve alone.

The roadmap names the cut (the (2,3) pebble game, rigid clusters,
GLOSSARY.md *Rigid cluster*) but not what a cluster is for laman's model,
how the diagnostics stay equal, or what holds state between solves. Those
three are contested, so this is an idea before a plan. C2 is also ~25–30
steps in total: it splits into milestone plans (below), and the first can
be planned as soon as Round 1 is answered.

## Constraints it runs into

- **Diagnostics are numbers, unchanged**: DoF by rank (ADR-0002, ADR-0003),
  redundancy and conflict sets by deletion (ADR-0005). A decomposition may
  make them cheaper, never different.
- **Determinism and least change** (`.agents/rules/solver.md`, SEED.md §8):
  a satisfied input takes no step, so its bits are the start's.
- **The C1 digests**: any change to the solve path moves them; the roadmap
  allows a re-bless with the reason.
- **Genericity**: Laman counting is exact for distance bars only. Laman's
  constraints are not bars: `horizontal`, `vertical` and ground break
  rotation and translation invariance, `parallel`/`angle` systems have
  known counting failures, and tangent contacts are non-generic by design
  (ADR-0002). A combinatorial answer is a proposal the numbers must
  confirm.
- **Public API**: `Sketch::solve(&self, &SolverOptions) -> Solution` is
  stateless; `DragSession` is the only stateful object. An edit API would
  be a design delta (`.agents/rules/solver.md` §API).
- SEED.md §7: dense `nalgebra`; "sparse methods are a later, argued
  change". Decomposition is what keeps dense viable.

## Options

### A — Rigid clusters by a generalized pebble game (the roadmap's words)

Run a pebble game over the constraint hypergraph (points 2 pebbles, a
radius 1, each equation an edge), find maximal rigid clusters, solve each,
then substitute each solved cluster by a rigid body (3 transform
variables) and solve the bodies' assembly: the classical DR-plan
(Bouma–Fudos–Hoffmann 1995; Hoffmann–Lomonosov–Sitharam 2001).
- For: true rigid clusters, floating ones included: a free 500-entity
  plate drags as one body of 3 unknowns.
- Against: the count must model which motions each constraint subset
  is invariant to (3, 2, 1 or 0); angle-only subsystems miscount; rigid
  bodies add a second kind of unknown to `System`, the engines and the
  diagnostics (the null basis is then in transform space). Largest change.
- Cost: ~3 milestone plans (count, solve by bodies, diagnostics by bodies).

### B — Block-triangular stages (Dulmage–Mendelsohn on the compiled system)

Laman's unknowns are absolute coordinates and ground fixes the frame, so
the equation–parameter bipartite graph of each component can be
decomposed directly: a maximum matching, then the strongly connected
components of the matched graph (Dulmage–Mendelsohn 1958, in SEED.md §8).
That gives **stages**: square, structurally determined sets of equations
with the parameters they fix, in dependency order, plus one
under-determined **remainder** that depends on them and on which nothing
depends.
- Each stage is a small LM/DogLeg solve over the existing `System`
  (a stage is just a smaller component: no new unknown kind).
  The SSS triangle becomes `{horizontal, base} → a`, then
  `{left, right} → b`.
- An edit to a dimension re-solves its stage and the stages after it; a
  drag moves only the remainder (where all freedom lives), and every
  stage keeps its bits, by least change, not by a cache.
- Diagnostics: a stage of full numerical rank adds its equations to the
  rank exactly; DoF, redundancy and conflicts then run per stage and on
  the remainder. A structurally square stage that is numerically
  deficient (a redundant `parallel`, a tangent contact) is merged with its
  dependents and analysed whole — the fallback that keeps the numbers
  equal, at the cost of locality for that stage.
- Against: no help to a large *ungrounded* or under-constrained region —
  the remainder is solved whole. A floating rigid plate stays one dense
  solve.
- Cost: one milestone plan (~8 steps) plus an ADR.

### C — B now, rigid bodies in the remainder later if measured

B as the decomposition; the bench at 500 entities decides whether the
remainder needs A's rigid-body substitution. If the generated corpus's
remainders fit in 4 ms, A is never built.

### Do nothing

Whole-component dense solves; C2's acceptance is out of reach at ~100
entities. Not viable for the cycle's goal.

### State between solves (applies to B and C)

- **S1 — stateless**: `solve` decomposes every call (a matching on ~1 000
  vertices: microseconds) and satisfied stages take no step. Incremental
  for free, no API change. A test can hold the strongest property: an
  incremental re-solve's digest *equals* the from-scratch solve of the
  same sketch.
- **S2 — a cache**: a solver object holding stages and their diagnostics,
  keyed by `Sketch::revision`. Faster on re-diagnosis; a public type,
  invalidation rules, and a second path that can disagree with the first.
- `DragSession` caches its decomposition inside either way (the required
  equations do not change during a drag): no public change.

## Recommendation

**C with S1.** B fits the model laman already has — absolute coordinates,
ground, the compiled `System` — and its only cost to the diagnostics is a
named fallback, not a second unknown kind. A's generality pays only for
floating rigid regions, which C2's corpus can measure before anyone builds
it. S1 keeps one solve path, so "incremental equals from scratch" is a
digest equality rather than a hope; S2 is added only if the bench says
re-diagnosis misses 4 ms.

Milestones (continuing C1's numbering):

- **M5 — done, corpus and bench at scale**: rectangle chains, slot grids
  and triangle strips, construction properties and the bounded 4 ms
  benchmark. The measured baseline and corpus contract live in
  ARCHITECTURE.md; upstream edit bit drift is an M6 regression in BACKLOG.md.
- **M6 — stages**: absorbed by `plans/m6-stages` (its design lives there
  until retired, then in ARCHITECTURE.md and ADR-0009).
- **M7 — drag robustness**: sweep-jump bisection, the bisected-frame mark,
  the observed subject in the drag property, radius ≥ 0 (lifts
  `negative-radius-solve`, `drag-negative-radius`); conflict sets against
  the exact minimum on the corpus. Independent of M6; can run beside it.
- **M8 — only if M6's bench misses**: rigid bodies in the remainder (A) or
  a cache (S2).

What would change my mind: a consumer whose sketches are mostly
ungrounded (A first), or M5's baseline showing the remainder, not the
stages, dominates on grounded sketches.

**The words.** GLOSSARY.md's *Rigid cluster* is the pebble game's rigid
part and lists *block* under _Avoid_; option B's parts are not rigid
clusters (they are structural, in absolute coordinates). Proposed entries:

- **Stage**: a square, structurally determined set of a component's
  required equations with the free parameters they fix, from the
  Dulmage–Mendelsohn decomposition; stages solve in dependency order, so
  an edit never moves a stage before it. _Avoid_: block, cluster.
- **Remainder**: a component's under-determined part, solved after its
  stages with least change; where every freedom, and every drag, lives.

*Rigid cluster* stays for option A, and the roadmap's §C2 *In* line is
reworded to name stages.

## Decision for the human

Round 1:

❓ **Q1 — Decomposition**: A (pebble-game rigid clusters with rigid-body
substitution), B (Dulmage–Mendelsohn stages only), or C (B now, A later
only if the 500-entity bench misses)?
➡️ C: fits laman's absolute-coordinate model; A's cost is paid only if measured.

❓ **Q2 — State**: S1 (stateless, decompose every solve, incremental by
least change) or S2 (a public cached solver keyed by revision)?
➡️ S1: one path, no API change, and incremental = from-scratch is a digest test.

❓ **Q3 — Milestones**: M5 corpus+bench → M6 stages → M7 drag robustness
(parallel to M6 allowed) → M8 conditional; or fold the corpus into M6?
➡️ The split: the baseline numbers come before the change they judge.

❓ **Q4 — Words**: add *Stage* and *Remainder* as above, keep *Rigid
cluster* for A, reword ROADMAP.md §C2's *In* line?
➡️ Yes: the roadmap's word would otherwise name something B does not build.

An ADR is needed for M6 (the decomposition, the fallback, the digest
re-bless), written by its plan.

*Later*: none for this idea. Whether M8 runs is measured by M6's bench;
the corpus's exact shapes and counts are M5's plan.

## Decision

2026-11-04, the human: "yes" to Round 1.

- Q1: **C** — Dulmage–Mendelsohn stages now; rigid bodies (A) only if
  M6's 500-entity bench misses 4 ms.
- Q2: **S1** — `solve` stays stateless; incremental by least change;
  `DragSession` caches its decomposition internally.
- Q3: **M5** corpus+bench → **M6** stages → **M7** drag robustness
  (may run beside M6) → **M8** conditional.
- Q4: *Stage* and *Remainder* added to GLOSSARY.md, *Rigid cluster*
  re-scoped to option A. ROADMAP.md §C2's *In* line is reworded by M6's
  plan (design docs change with the behaviour).
