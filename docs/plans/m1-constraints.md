# Plan: m1-constraints

- Started: 2026-11-01
- Milestone: M1 (cycle C1, docs/ROADMAP.md)
- Idea: none; verbatim from the roadmap: "every cycle-1 constraint has
  residuals and an analytic Jacobian that agree with finite differences"

## Goal

`laman::residual` evaluates every row of DATA-MODEL.md §Constraints and the
arc's intrinsic equation at any coordinates: each equation's value, its
unit, and its analytic gradient over the parameters it reads. A
finite-difference property in `laman-debug` holds every one of them, 1 000
cases per constraint variant, over random non-degenerate and
near-degenerate configurations; and at every C1 fixture's expected
coordinates each residual is within tolerance exactly where the checker
says the constraint holds.

## Non-goals

- The `System` (dense parameter vector, compiled order, components), the
  solve, diagnostics, `Solution`: M2. Nothing here caches `cos θ`/`sin θ`
  per solve; M2's compile does.
- Degenerate entities as a diagnostic: M2. Here an equation only says it
  was evaluated at an undefined point (delta 3).
- The Python binding of `residual`: M4, if a consumer asks.
- Properties by construction (random *satisfied* configurations): M2's.

## Design deltas

1. **New public module `laman::residual`** (ARCHITECTURE.md §Crates already
   lists it). Public so the property tests in `laman-debug` reach it;
   `laman-check` never names it (`tools/check-layers.sh` holds that).
   - `pub enum Unit { Length, Ratio }`: a length residual compares with
     `tolerance::LENGTH`, a dimensionless one (`parallel`, `perpendicular`,
     `angle`) with `tolerance::ANGLE`.
   - `pub struct Equation { value: f64, unit: Unit, degenerate: bool,
     terms }`: `terms` a fixed-capacity list of `((EntityId, Coordinate),
     f64)`, at most 8 (the widest: `symmetric`, `angle`, an arc's radius in
     `tangent`/`equal_radius`), **merged by parameter and sorted** — two
     lines sharing a point (`perpendicular(o→a, o→b)`) sum into one term,
     in a fixed order. Accessors, no public fields.
   - `pub fn equations(sketch: &Sketch, values: &Coordinates, id:
     ConstraintId) -> Option<Equations>` (`None`: no such id; up to 2
     equations, in DATA-MODEL's order), and `pub fn intrinsic(sketch,
     values, id: EntityId) -> Option<Equation>` (an arc's; `None` for other
     kinds). A coordinate `values` lacks is read from the sketch, so both are
     total: no `Result`, no panic.
   - Gradients cover fixed parameters too; M2's compile drops their columns.
   - Every `Constraint` and `Entity` match is wildcard-free.
2. **`laman` depends on `libm`** (`angle`'s `cos θ`, `sin θ`, computed per
   evaluation here). ARCHITECTURE.md's table already says so.
3. **Exact degeneracy** (`|u| = 0`, `p = c`, coincident centres): the
   undefined quotient is taken as 0 and the undefined gradient terms are
   left out, the value stays finite and `degenerate` is set. Never NaN or
   infinity on finite input. (⚠ OPEN 1.)
4. **The finite-difference rule** (`laman_debug::fd`, new): an
   eighth-order central stencil (nine nodes, Fornberg weights on the nodes
   as rounded) with step `h = 5e-3 · ℓ`, `ℓ` the equation's **local scale**
   (the shortest length among the lines, radii and point pairs it reads,
   capped by the sketch's size), and agreement `|J_i − D_i| ≤ 1e-6 ·
   max(‖J‖∞, 1/ℓ for Ratio, 1 for Length)`. Sound while `X/ℓ ≤ 1e6` (`X`
   the coordinates' magnitude), so a strategy keeps every length a local
   scale counts at least `1e-6` of the sketch's size. The roadmap's "step
   `1e-6` of the sketch's size" cannot check a line `1e-6` of the sketch's
   size; ROADMAP.md §M1 is amended. (Settled, OPEN 2.)
5. **Configuration strategies** (`laman_debug::strategy::configuration`,
   new): one minimal sketch per constraint variant (every kind; `Span` as
   line and as points; `Round` as circle and as arc on each side; tangent
   external, internal ±, line–circle ±), coordinates drawn in modes:
   generic; a line `1e-6` of the size; a point on (or `1e-9` off) the
   line; tangent circles of nearly equal radius; shared points between
   the constraint's entities; nearly parallel lines for `angle`.

## Steps

Grades are tiers (`.agents/tiers.md`): **[1]** routine — the design says
exactly what to write; **[2]** careful — a numeric or geometric case to get
right within the design; **[3]** unproven — a robustness or bound
established here.

- [x] Step 1 **[3]** — `laman_debug::fd`: the rule of delta 4 over any
  `Fn(&[f64]) -> f64`, with its own tests on closed forms (`|v|`,
  `cross(u, p − a)/|u|` at `|u| = 1e-6`, `1e-6` and `1e3` scales): exact
  gradients pass, a negated term, a dropped term and a `1e-4` relative
  error each fail. The bound and why it is enough go in the module doc.
- [x] Step 2 **[2]** — `strategy::configuration` (delta 5), with a test
  that every variant's sketch builds and every mode is reached.
- [x] Step 3 **[2]** — `laman::residual` skeleton (deltas 1–3) with the
  point-only kinds: `coincident`, `fix`, `horizontal`/`vertical` (both
  spans), `midpoint`, `distance`, and the arc's intrinsic equation. FD
  property at 1 000 cases each; rustdoc example on `equations`.
- [x] Step 4 **[2]** — line kinds: `point_on_line`, `parallel`,
  `perpendicular`, `equal_length`, `symmetric`, `point_line_distance`
  (both senses), `angle`. FD property at 1 000 cases each, shared-point
  and near-parallel modes included.
- [x] Step 5 **[2]** — round kinds over circles and arcs: `point_on_circle`,
  `equal_radius`, `radius`, `tangent` line–circle (±), `tangent` circles
  (external, internal ±). FD property at 1 000 cases each, the
  nearly-equal-radius internal contact included.
- [x] Step 6 **[1]** — the fixture test (`laman-debug/tests/c1.rs`): at
  every C1 fixture's and twin's expected coordinates, each constraint's
  equations are all within their unit's tolerance iff the checker reports
  no violation for it; every intrinsic equation is within tolerance; in
  each conflicting twin the added constraint, and only it, is not.
- [x] Step 7 **[1]** — the gate's time measured with the 1 000-case
  properties; if the gate passes a minute, the case count moves behind an
  env var with CI running 1 000, and that is recorded here.
  Measured (warm cache, `timeout 590 tools/gate.sh` in the foreground):
  **23 s** wall, green; `residual` alone 11.4 s of it. Under a minute, so
  no env-var fallback: the count stays at 1 000 everywhere.

## Acceptance

`cargo test -p laman-debug --test residual` green with `PROPTEST_CASES`
unset and each variant's property at 1 000 cases (≥ 30 variants, listed by
the test); the fixture test of step 6 green on all ten C1 directories
(`sss-triangle`, `belt`, `slotted-plate` and their twins, the drag); the
`fd` self-tests catching the three planted errors; the wasm32 build of
`laman` and `tools/check-layers.sh` green in the gate.

## Docs to update on completion

- `docs/ARCHITECTURE.md` §Crates (`residual` row: `Equation`, units,
  terms keyed by `(EntityId, Coordinate)`); §Solve step 1 (compile reads
  `residual` and drops fixed columns).
- `docs/DATA-MODEL.md` §Constraints, the Jacobians bullet: the merged
  terms, exact degeneracy (delta 3), the FD rule (delta 4).
- `docs/ROADMAP.md` §M1: the FD step as local scale; status landed.
- `GLOSSARY.md`: **Equation** (one residual `f = 0` a constraint or
  entity contributes, with its unit and gradient; _Avoid_: row, residual
  as a noun for the whole thing) and **Local scale**.
- `CHANGELOG.md` Unreleased: `laman::residual` — each constraint's
  equations and their gradients at any coordinates.
- `AGENTS.md` current state: M1 landed; next `/plan` M2.

## Open questions

- **Settled 1** (step 3): exact degeneracy (delta 3) is a finite value, the
  undefined terms left out and `Equation::is_degenerate`; a subgradient
  would need state (the previous iterate) the residual has not. Taken as
  written in `laman::residual`'s module doc. The FD property treats a
  degenerate or non-finite equation as a failure: no strategy draws one.
  Step 3 also gave `Configuration` its `lengths()` and `local_scale()`
  (moved from the strategy's test), which the FD properties use.
- **Settled 2** (step 1): the FD rule's numbers (delta 4). The planned
  `h = 1e-6 ℓ` three-point stencil rounds to `2e-4` of the bound at `X/ℓ
  = 1e6`; a five-point stencil at `1e-2 ℓ` truncates to `2e-6` where a
  gradient is accidentally small beside its curvature (a point square off
  a short line's end); and rounding the nodes `x + k h` alone measures
  past `1e-6` there. Eighth order at `5e-3 ℓ` with weights for the
  rounded nodes measures under `3e-7` of the bound over 150 000 cases; the
  derivation is `laman_debug::fd`'s module doc. Written into ROADMAP §M1.
- **Settled 3** (step 5): `Equation`'s capacity of 8 holds. The widest are
  `symmetric`, `angle`, and the arc cases of `tangent` (line–arc: the line's
  two ends, the centre, the start; arc–arc: two centres, two starts), each
  exactly 8 distinct parameters; a unit test builds the two arc cases on
  unshared points and asserts 8 terms. The raw list (16) holds the widest
  before merging (arc–arc: 4 + 4 + 4).
