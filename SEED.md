# Project Seed Document: Laman

## 1. Project Overview
**Name:** Laman
**Tagline:** A deterministic 2D geometric constraint solver in pure Rust, for Rust and Python, built to be trusted.
**Core Concept:** `laman` is a library, not an application. It owns a 2D sketch: points, lines, circles and arcs; the geometric and dimensional constraints between them; the numerical solve that satisfies them; and the diagnostics that say what is free, what is fixed, and which constraints conflict. Any sketcher, code-CAD tool, mechanism designer, web drawing tool or CAD kernel front end can be built on top of it.

It knows nothing about any CAD kernel or application. It belongs to the `arris-labs` family by sharing its conventions (determinism, zero panics, typed diagnostics, verification-first, agentic development), not by sharing types. A consumer's ids, units and references stay the consumer's.

Development is fully agentic. The human sets direction and judges results; the agent designs, implements, tests and documents. Everything the solver does must therefore be checkable as numbers, without a human looking at a screen (§5, §10).

## 2. The Naming Rationale
The name honours **Laman's theorem** (G. Laman, *On graphs and rigidity of plane skeletal structures*, 1970). It says when points joined by rigid bars in the plane are exactly rigid. *n* points have 2*n* freedoms, minus 3 for moving the whole shape, so a rigid shape needs exactly 2*n* − 3 bars. On top of that, no subset of *k* points may carry more than 2*k* − 3 bars among them, or the excess is wasted there and some other part stays floppy. That is the product in one sentence: count the remaining freedom exactly, and say *where* the constraints are missing or in excess (§4, differentiator 1). The theorem also underlies the classic rigid-cluster decompositions (the pebble game) that cycle 2's subsystem decomposition builds on (§6), and the SSS triangle in cycle 1's acceptance is its base case.

The name stands for the idea, not a claim of scope. Laman's theorem strictly covers distance constraints between generic points. The solver handles the full constraint vocabulary (§6) and counts freedom numerically, from the rank of the Jacobian. Documentation says "named after Laman's theorem" and implies no endorsement by Laman or his estate.

`laman` was free on crates.io and PyPI as of 2026-11-01. The working name `clevis` was dropped because it is taken on both registries. Reserving and publishing the names (a minimal real 0.0.x release, not an empty placeholder) is the human's act. Before the first public release, check trademark databases (WIPO, EUIPO, USPTO) for "LAMAN" in software classes. Bare "laman" is a noisy search term (Malay/Indonesian for "page"), so docs and the README pair it with "constraint solver".

## 3. The Problem Statement
There is no permissively licensed, standalone 2D constraint solver of sketcher grade, for Rust or for Python:

* **SolveSpace's `slvs` library** is the de-facto choice in Python (through py-slvs / python-solvespace). It is GPLv3, so anything that links it is GPLv3. It is a C/C++ build, which makes wheels fragile and wasm hard. Its diagnostics are a result code and a list of failed constraints.
* **FreeCAD's PlaneGCS** is the most capable open 2D sketch solver. It is LGPL, written in C++ on Eigen, and not distributed by its authors as a standalone library. Using it means vendoring part of FreeCAD.
* **Generic optimisers** (`scipy.optimize` and the like) are what code-CAD users fall back on. They minimise residuals but know nothing of degrees of freedom, redundancy or conflict, and the user writes the trigonometry by hand.
* **In Rust** there is, as far as we know, no maintained crate at this level. This is surveyed and recorded in `docs/COMPARISON.md` before the first release (§10).

> **Note (2026-11-01).** The re-survey (`docs/COMPARISON.md`) found the Rust half of this false: several MIT or MIT/Apache crates already offer DoF analysis, and one (`sketch-solver`) claims determinism across targets. The Python half stands. Cycle 1 goes ahead as planned. Laman's claim now rests on what no neighbour offers: a permissive Python package, the independent checker, and drag that is both deterministic and branch-keeping. Diagnostics in the caller's ids lead in degree.

So the problem is not "minimise some residuals". It is: give a sketch an exact count of its remaining freedom, name the constraints that are to blame when it cannot be satisfied, behave smoothly under interactive editing, and return the same bits on every platform, under a licence nobody has to think about.

## 4. Competitive Landscape (as of November 2026; to be re-surveyed, §10)

| Solver | What it is | Why it does not close the gap |
|---|---|---|
| SolveSpace `slvs` (+ py-slvs) | 2D and 3D constraint solver, Newton with DoF analysis | GPLv3; C/C++ build; coarse diagnostics; an application's library, not designed as a product |
| FreeCAD PlaneGCS | 2D sketch solver: DogLeg/LM/BFGS, redundancy and conflict detection, conics and B-splines | LGPL; C++/Eigen; embedded in FreeCAD, no standalone distribution |
| Generic optimisers | Least-squares minimisers | No DoF, no conflict sets, no geometry vocabulary |
| D-Cubed DCM, others | Industrial 2D/3D solvers | Proprietary. The bar, not a competitor |

**Why Laman, the differentiators in priority order:**
1. **Diagnostics are the product.** Exact remaining DoF, per entity. Redundant-but-consistent constraints told apart from conflicting ones. The minimal set of constraints to blame, named by the caller's ids and tags. Written for a human in a sketcher and equally for an LLM or RL agent that has to correct its own sketch.
2. **Interactive by design.** Drag, incremental edits and warm starts are first-class. The solver keeps the solution branch the user is on and disturbs as little as it can. A batch-only solver cannot be retrofitted into a sketcher.
3. **Deterministic everywhere.** Same input, same bits, on Linux, macOS, Windows and wasm32.
4. **Pure Rust, MIT OR Apache-2.0, on crates.io and PyPI.** No C toolchain, abi3 wheels, builds for wasm32.

**Explicit non-goals:** 3D constraints (workplanes, 3D points; a later cycle may revisit, §6); rendering or UI; snapping and inference (they are the sketcher's, and they depend on its view); a B-Rep or any solid modelling; file formats beyond the serde form of a sketch; assemblies and 3D kinematics; speed as a headline. Performance is a design constraint only: interactive sizes must solve inside a frame budget (§6), and nothing in the design may preclude sparse methods later.

## 5. The Solution & Developer Experience
Laman should feel like a well-typed Rust library and an idiomatic Python package, not a port of a C API:

* **A `Sketch` owns everything; ids are cheap handles.** Parameters, entities and constraints live in one arena with typed ids. The caller may mint ids itself (a document that persists them) or let the sketch mint them. An optional tag and an opaque caller payload ride on every entity and constraint and come back in every diagnostic.
* **`solve` always returns a `Solution`.** Coordinates (best effort when unsatisfiable), a status, and `Diagnostics`. Under- and over-constrained are statuses, not errors. `Err` is reserved for a malformed sketch: a dangling id, a constraint on the wrong kind of entity, a non-finite input. "Require fully constrained" is a check on the result, not a solver mode.
* **Diagnostics name things.** DoF per entity; the free directions of an under-constrained sketch mapped back to entities; redundant and conflicting sets as caller ids and tags.
* **Interactive sessions.** A drag session moves a target point while the rest of the sketch follows, by priority, with least change. Re-solves after an edit start from the previous solution.
* **Independent verification.** `laman-check` re-evaluates every constraint from the solved coordinates with code that shares nothing with the solver's residuals, and reports every violation.
* **Everything is inspectable without a GUI.** A sketch and its diagnostics dump to text that diffs in a test and to an SVG the agent can read as an image.

```rust
let mut sk = Sketch::new();
let o = sk.point_fixed(0.0, 0.0).tag("origin");
let a = sk.point(40.0, 5.0);
let b = sk.point(10.0, 30.0);
let base = sk.line(o, a);
sk.horizontal(base);
sk.distance(o, a, 50.0).tag("base");
sk.distance(o, b, 30.0).tag("left");
sk.distance(a, b, 40.0).tag("right");
let sol = sk.solve(&SolverOptions::default());
assert_eq!(sol.status, Status::FullyConstrained);
assert_eq!(sol.diagnostics.dof(), 0);
```

```python
import laman
sk = laman.Sketch()
o = sk.point(0, 0, fixed=True, tag="origin")
...
sol = sk.solve()
for c in sol.diagnostics.conflicting:
    print(c.tag, c.kind)
```

## 6. Core Features

### Cycle 1: the vertical slice
One path through every crate before any crate is broad. **Acceptance:** the SSS triangle, the belt (two circles with both external tangents), a symmetric slotted plate, and one drag of a vertex across a fully constrained profile with one free dimension. Each is solved, checker-green, with exact DoF; the over-constrained twins name the right constraints; the digests are identical natively and on wasm32; the same sketches solve from Python.

1. **Model:** the arena; points, line segments, circles, arcs; fixed and free parameters; tags and payloads; the serde form.
2. **Constraints:** coincident, point on line, point on circle, horizontal, vertical, parallel, perpendicular, equal length, equal radius, tangent (line–circle, circle–circle), midpoint, symmetric, fixed point, and the dimensions distance, point–line distance, radius and angle. Each with analytic residuals and Jacobian entries.
3. **Solver:** damped least squares (Levenberg–Marquardt and DogLeg), hard iteration caps, warm start, priorities, a drag session.
4. **Diagnostics:** rank and DoF, per-entity freedom, redundant vs. conflicting sets.
5. **Checker:** `laman-check`, independent of the solver's residual code.
6. **Harness:** fixtures by construction, analytic fixtures, Jacobian-vs-finite-difference checks, property tests, the determinism digest, the fuzzer, the SVG and text dumps, benchmarks with frame budgets.
7. **Python:** PyO3 + maturin bindings, typed stubs, the same fixtures from pytest.

### Cycle 2: the sketcher gate
Laman can sit under an interactive sketcher. Incremental edits and drags at interactive sizes (about 100 to 500 entities) solve inside a per-frame budget, keep their branch, and leave unrelated geometry where it was. Subsystem decomposition makes a local edit cost a local solve. The first consumer's requirements are written as laman fixtures; the consumer integrates on its own schedule.

### Later cycles, roughly in order
Closed profiles (loops and holes from the solved, non-construction curves) · trim, extend, offset and mirror · ellipses and their arcs · B-splines · projected external references as fixed geometry · 3D constraints (to be argued, §4). Each earns its own cycle with an acceptance corpus; none is scheduled here.

## 7. Tech Stack (decided)

| Layer | Choice | Why / notes |
|---|---|---|
| Language | Rust, edition 2024, MIT OR Apache-2.0 | Pure Rust, wasm-capable; matches the `arris-labs` family |
| Linear algebra | `nalgebra` (f64, `libm-force`) | Small dense LU/QR/SVD for rank and steps; the same choice and configuration as arris. Sparse methods are a later, argued change |
| Transcendentals | `libm`, pinned exactly | The same bits natively and on wasm32. `std`'s `sin`, `cos` and `atan2` are not guaranteed to be |
| Errors | `thiserror` | Typed errors naming ids; no panics on any input |
| Serialization | `serde`, `serde_json` with `float_roundtrip` | The sketch's serde form; fixtures round-trip exactly |
| Property tests | `proptest` (Rust), `hypothesis` (Python) | Random sketches by construction; shrinking to fixtures |
| Fuzzing | `cargo fuzz` (nightly, excluded from the workspace) | "No input panics" is tested, not asserted |
| Python | PyO3 + maturin, abi3 wheels, `uv` | One wheel per platform; strict `.pyi` stubs |
| Visual debug | Own SVG writer | The agent reads SVGs; nothing in a gate depends on looking |
| Oracle | None by default. SolveSpace (py-slvs) or PlaneGCS run out of process under `tools/oracle/` only when analytic expectations run out | Run, never linked or copied; DoF and conflict sets are compared, not coordinates (two solvers may pick different branches) |

### Workspace layout
```
laman          model, constraints, solver, diagnostics, drag sessions (the published crate)
laman-check    the independent verifier (its own crate so it can never share the solver's residuals)
laman-debug    text and SVG dumps, fixtures, proptest strategies, benchmarks (unpublished, dev-facing)
laman-py       PyO3 bindings (published to PyPI as `laman`, not to crates.io)
```
**Layer rule:** `laman-check` depends on `laman`'s model types only, never on its solver. Splitting `laman` further waits for a layer rule that needs it.

## 8. Heritage and References

**From building the sketcher in arrix (experience only; no code is carried over):**
* Over-constrained and unsatisfiable are different. A redundant constraint that agrees with the rest is a warning; one that disagrees is a conflict. Users need to be told which.
* A sketcher solves every frame. An over-constrained sketch must still return its best geometry and diagnostics, never just an error.
* Dragging needs priorities and least change, not a plain solve. A drag that flips a tangent to its other side or swings an arc through its centre is a bug, even when every residual is zero.
* The analytic Jacobian is checked against finite differences for every constraint, in every configuration a test can reach.
* Re-solving an already solved sketch must return the same bits.
* Constraint kinds no consumer reaches are not added (scope is argued, not accreted).

**From SolveSpace and PlaneGCS (read-only):** the constraint vocabularies, the DoF and redundancy analyses, and the catalogue of known failure modes. Read for ideas and mistakes, reimplemented from scratch for `laman`'s own model. Any design decision informed by them names the source in its ADR, so provenance is auditable (`PROVENANCE.md`).

**Literature:** Laman, *On graphs and rigidity of plane skeletal structures* (1970); Dulmage & Mendelsohn, *Coverings of bipartite graphs* (1958); Bouma, Fudos, Hoffmann et al., *Geometric constraint solver* (1995); Hoffmann, Lomonosov & Sitharam, *Decomposition plans for geometric constraint systems* (2001); Jermann, Trombettoni, Neveu & Mathis, *Decomposition of geometric constraint systems: a survey* (2006); Madsen, Nielsen & Tingleff, *Methods for Non-Linear Least Squares Problems* (DTU, 2004); Nocedal & Wright, *Numerical Optimization*.

## 9. Decisions taken in this seed

| Question | Decision | Reason |
|---|---|---|
| Port the arrix solver, or start from scratch? | **From scratch; arrix's experience distilled into §8, its code not read for copying** | A clean MIT provenance and an architecture chosen for a standalone library rather than inherited from an application |
| Coupling to arris or arrix | **None, in either direction** | A standalone library; family fit comes from shared conventions. Adapters live in consumers |
| Licence | **MIT OR Apache-2.0** | Matches the family; permissive is the reason to exist (§3) |
| Home | **GitHub `arris-labs/laman`** | One family, visible together |
| Distribution | **crates.io (`laman`) and PyPI (`laman`)** | Rust and Python users are both the audience |
| Dimensions | **2D only** | Sketches are 2D; 3D constraints are a different product, to be argued later |
| Over- and under-constrained | **Statuses with best-effort results, not errors** | A sketcher solves every frame; an agent needs the geometry and the blame together |
| Jacobian | **Analytic, checked against finite differences** | Exact, cheap, and testable; automatic differentiation adds a layer without adding correctness |
| Determinism | **Bit-identical across targets: pinned `libm`, ordered containers, fixed summation order, no thread-dependent results** | The gate checks it with a digest on native and wasm32 |
| Verification | **Independent checker plus fixtures by construction; competitor oracles only on demand** | Most expectations are known analytically or by construction |
| Units | **Unitless: the caller's units** | ⚠ OPEN: whether tolerances are absolute or relative to the sketch's scale |
| Arc parametrisation | ⚠ OPEN | Centre + two endpoints with an implied radius, or centre + radius + angles. Decides the arc's DoF, branch behaviour and which constraints are intrinsic |
| JS bindings | **Not in v1** | wasm32 means "builds and passes its tests"; JS consumers can bind the crate themselves |
| First milestone | **Vertical slice (§6), not a broad constraint catalogue** | Proves the model, the diagnostics and the drag path before the catalogue grows |

## 10. Directives for the AI Agent
With this seed agreed, and before writing solver code:
1. **Scaffold the docs system and tooling** from arris: `docs/README.md` with the three tiers, `docs/adr/`, `docs/ideas/`, `docs/plans/`, `BACKLOG.md`, `GLOSSARY.md`, `AGENTS.md`/`CLAUDE.md` with a short current state, the `/idea` `/plan` `/work` `/retire-plan` `/close-cycle` skills, the gate, hooks and lints (`unsafe_code = "forbid"`, clippy `unwrap_used`, `expect_used`, `panic`), and `PROVENANCE.md`. Seeing a sketch means "dump it to SVG and look".
2. **Architecture document** (`docs/ARCHITECTURE.md`): crates and the layer rule, the arena and id model, the `solve` signature and error model, the drag session, the checker's place, determinism rules, wasm32.
3. **Data model document** (`docs/DATA-MODEL.md`): entity parametrisations (closing the arc ⚠ OPEN), each constraint's equations, DoF removed and intrinsic constraints, diagnostic semantics (DoF, redundant, conflicting), the serde form and its versioning.
4. **Comparison** (`docs/COMPARISON.md`): re-survey §4 (SolveSpace, PlaneGCS, any Rust crates) with sources, before the first release.
5. **Roadmap** (`docs/ROADMAP.md`): cycle 1 with its acceptance fixtures spelled out as numbers (coordinates, DoF, the ids of conflict sets, digests, frame budgets); cycle 2 as the sketcher gate; later cycles as one line each.
6. **Build the harness before the solver is broad.** Fixtures by construction, the checker, the determinism digest and the SVG dump are cycle-1 deliverables in their own right and come first.
7. **Every failure becomes a fixture.** A reproduced bug is shrunk to a minimal case and committed with the desired assertion, `#[ignore]`d until it passes.
8. **Write an ADR for each ⚠ OPEN when it closes.** Open at kickoff: arc parametrisation; absolute vs. scale-relative tolerances; how branch preservation is defined and tested; whether the minimal conflict set is exact or a bounded approximation on large sketches.
