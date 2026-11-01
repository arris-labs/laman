# Laman and the alternatives

A survey for someone choosing a 2D constraint solver for Rust or Python,
and the check on laman's own reason to exist (SEED.md §3, §4). Surveyed
2026-11-01 from each project's own pages and registry metadata, cited
inline; nothing here was run or benchmarked. Laman's column is its plan,
not its state: today it ships only the rigidity count (`CHANGELOG.md`).
`/close-cycle` re-reads this file against the code and the sources; a row
that stops being true is fixed in the commit that makes it untrue.

## What changed since the seed

SEED.md §3 says no permissively licensed, standalone, sketcher-grade 2D
solver exists for Rust or for Python. **For Rust that is no longer true.**
Since the seed was written (or unnoticed by it), at least four MIT-licensed
Rust solvers with points, lines, circles, arcs and DoF analysis are on
crates.io, and one of them, `sketch-solver`, describes itself in nearly
the seed's words: deterministic, the same bits on x86_64, aarch64 and
wasm32, `libm` for trigonometry, redundant and conflicting constraints
named, MIT OR Apache-2.0. **For Python the gap stands**: the solvers on
PyPI are GPL (SolveSpace's) or LGPL (PlaneGCS's), and none of the Rust
solvers publishes a Python package.

What that leaves laman, in SEED.md §4's order, is in §Where laman would
differ. Whether it is still worth building as planned is the human's call
(AGENTS.md "Current state").

## The field

| Solver | Language | Licence | Python | wasm32 | Diagnostics | Drag / interactive | Determinism claimed | State |
|---|---|---|---|---|---|---|---|---|
| SolveSpace `slvs` | C++ | GPL-3.0 | `python-solvespace` 3.0.8 (GPLv3+, last upload 2022-11) [^pss]; `py-slvs` 1.0.6 (GPL-3.0) [^pyslvs] | not packaged | a `failed[]` list: provably inconsistent, or unsatisfied [^slvsdoc] | no API for it | no | mature, the de-facto Python choice |
| FreeCAD PlaneGCS | C++ / Eigen | LGPL-2.1+ | `planegcs` 0.9.6 (2026-10-10; wheels for Linux and Windows x86_64 only) [^planegcs] | `@salusoft89/planegcs` (npm) [^npmgcs] | DoF, redundant and conflicting (FreeCAD's sketcher) | inside FreeCAD | no | mature; standalone only through these extractions |
| `slvs` (Rust) | Rust wrapper of SolveSpace | GPL (SolveSpace's) | — | — | SolveSpace's | — | no | 0.6.0, last release 2023-06 [^crates] |
| `sketch-solver` (neoscad) | Rust | MIT OR Apache-2.0 | — | yes, no glue | free DoF and free coordinates; redundant and conflicting equations named; branch flips against the drawing [^ss] | not documented; "never depends on an earlier solve" | yes: same bits on aarch64, x86_64, wasm32, checked in CI | 0.1.0 (2026-10-07), 2 commits, 0 stars; written for NeoSCAD [^ss] [^crates] |
| `acs-solver` | Rust | MIT | — | yes, npm `acs-solver`; also a C ABI | conflicting, redundant, remaining DoF, fully constrained entities | yes: temporary constraints as soft goals | not stated | 0.1.19 (2026-10-28), ~30 constraint kinds incl. ellipses, splines; used by the Part3D app [^acs] [^crates] |
| `ezpz` (KittyCAD / Zoo) | Rust | MIT | — | builds (an `ezpz-wasm` check crate) | unsatisfied constraints; per-variable freedom analysis | priorities on requests | not stated | 0.2.29, ~237k downloads; the solver behind KCL and Zoo Design Studio [^ezpz] [^ezpzdocs] |
| `arael-sketch-solver` | Rust | MIT | — | partly (`web_time`) | DoF, Jacobian rank and null basis, blocker analysis | yes: `DragApparatus` | not stated | 0.9.3 (2026-10-27), 15 % documented; part of the arael sketch editor [^arael] |
| `brepkit-sketch` | Rust | AGPL-3.0 | — | via a TypeScript port | DoF from QR rank; redundant | — | not stated | 4.6.17 (2026-11-01) [^brepkit] |
| `vcad-kernel-constraints` | Rust | MIT | — | — | DoF by counting; a status enum | an interactive `SketchSession` | not stated | 0.10.0; bound to the vcad kernel [^vcad] |
| Generic optimisers (`scipy.optimize`, `argmin`) | — | permissive | yes | — | none: residuals only | — | — | the user writes the geometry |
| D-Cubed DCM and other industrial solvers | C++ | proprietary | — | — | the bar | the bar | — | the bar, not a competitor |

## Where laman would differ

Each item is a plan, held by a roadmap acceptance (ROADMAP.md), not a
claim about today.

1. **Python, permissive, from the same core.** No permissively licensed
   solver is on PyPI. Laman ships abi3 wheels from the Rust crate, with
   typed stubs (SEED.md §4.4). This is the one gap the survey confirms
   outright.
2. **An independent checker.** None of the surveyed solvers documents a
   verifier that shares no code with its residuals; laman's `laman-check`
   does (ARCHITECTURE.md §The checker).
3. **Diagnostics as the product, named by the caller.** `sketch-solver`
   and `acs-solver` already name redundant and conflicting constraints.
   Laman adds the caller's tags and payloads in every item, per-entity DoF
   and free directions mapped to entities, and minimal conflict sets
   (DATA-MODEL.md §Diagnostics). The difference is degree, not kind.
4. **Interactive and deterministic together.** `sketch-solver` is
   deterministic but stateless; `acs-solver` and `arael` drag but claim no
   determinism. Laman's drag session is warm-started, least-change and
   branch-keeping, and its digests are held equal on native and wasm32
   (ARCHITECTURE.md §Drag sessions, §Determinism).

## What is not claimed

- No benchmark: nothing in this file was run. A comparison by numbers
  would run each solver out of process on laman's fixture corpus
  (`tools/oracle/`, SEED.md §7) and compare DoF and conflict sets, not
  coordinates.
- No capability laman does not yet have: today it is `laman::rigidity`.
- The rows for other solvers come from their own documentation and
  registry metadata on the survey date; a project's documentation can
  understate or overstate it.

[^pss]: https://pypi.org/project/python-solvespace/ (licence and upload dates from the PyPI JSON API)
[^pyslvs]: https://pypi.org/project/py-slvs/
[^slvsdoc]: https://github.com/solvespace/solvespace/blob/master/exposed/DOC.txt
[^planegcs]: https://pypi.org/project/planegcs/, https://github.com/spookylukey/planegcs
[^npmgcs]: https://github.com/Salusoft89/planegcs
[^crates]: crates.io API search, 2026-11-01: versions, dates and download counts
[^ss]: https://github.com/neoscad/sketch-solver, https://docs.rs/sketch-solver/latest/sketch_solver/
[^acs]: https://github.com/angelobartolome/acs
[^ezpz]: https://github.com/KittyCAD/ezpz
[^ezpzdocs]: https://docs.rs/ezpz
[^arael]: https://docs.rs/arael-sketch-solver/latest/arael_sketch_solver/
[^brepkit]: https://docs.rs/brepkit-sketch/latest/brepkit_sketch/
[^vcad]: https://docs.rs/vcad-kernel-constraints/latest/vcad_kernel_constraints/
