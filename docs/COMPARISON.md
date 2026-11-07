# Laman and the alternatives

A survey for someone choosing a 2D constraint solver for Rust or Python,
and the check on laman's own reason to exist (SEED.md §3, §4). Surveyed
2026-11-01 from each project's own pages and registry metadata, cited
inline; no benchmark was run. Three of the Rust solvers were also run on
laman's ten fixture sketches (§The comparison run). Laman's row is what it does
today: it solves, drags and checks sketches from Rust and Python
(`CHANGELOG.md`), with cross-OS wheel builds in CI and its digests held
equal on five OS/arch runners and wasm32.
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

What that leaves laman is in §Where laman would differ, differences in
kind first. Laman goes ahead as planned on those grounds (decided
2026-11-01; SEED.md §3 carries a dated note). Being deterministic and pure
Rust is now what a solver is expected to have, not a reason for laman.

## The field

| Solver | Language | Licence | Python | wasm32 | Diagnostics | Drag / interactive | Determinism claimed | State |
|---|---|---|---|---|---|---|---|---|
| `laman` | Rust | MIT OR Apache-2.0 | `laman`: abi3 wheels (Linux x86_64 and aarch64, macOS arm64 and x86_64, Windows x86_64) and an sdist, from the same crate, typed stubs | yes, no glue (`wasm32-unknown-unknown`) | DoF total, per entity and as free directions; redundant constraints; inclusion-minimal conflict sets; degenerate entities; branch flips; each named by the caller's id and tag; an independent checker | `DragSession`: warm-started, least change, branch-keeping | yes: the fixture digests equal on five OS/arch runners and wasm32-wasip1, checked in CI | this repository; points, lines, circles, arcs and 18 constraint kinds |
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

Items 1 to 3 exist today (the wheels, `laman-check`, the drag session and
the digests); item 4 is the degree of difference described below. Each is
held by a roadmap acceptance (ROADMAP.md), and none is a claim about the
other solvers' future.

1. **Python, permissive, from the same core.** No permissively licensed
   solver is on PyPI. Laman ships abi3 wheels from the Rust crate, with
   typed stubs (SEED.md §4.4). This is the one gap the survey confirms
   outright.
2. **An independent checker.** None of the surveyed solvers documents a
   verifier that shares no code with its residuals; laman's `laman-check`
   does (ARCHITECTURE.md §The checker).
3. **Interactive and deterministic together.** `sketch-solver` is
   deterministic but stateless; `acs-solver` and `arael` drag but claim no
   determinism. Laman's drag session is warm-started, least-change and
   branch-keeping, and its digests are held equal on native and wasm32
   (ARCHITECTURE.md §Drag sessions, §Determinism).
4. **Diagnostics as the product, named by the caller.** `sketch-solver`
   and `acs-solver` already name redundant and conflicting constraints.
   Laman adds the caller's tags and payloads in every item, per-entity DoF
   and free directions mapped to entities, and minimal conflict sets
   (DATA-MODEL.md §Diagnostics). The difference is degree, not kind.

## The comparison run

Run 2026-11-03: `sketch-solver` 0.1.0, `acs-solver` 0.1.19 and `ezpz` 0.2.29
(each pinned exactly) were run out of process, each on laman's ten fixture sketches
(the four fixtures and their six twins under `tests/fixtures/c1/`), started
from the fixture's guesses. Each translation was proven before its numbers
count: each solver's own equations are satisfied, within a tolerance, at the
fixture's expected coordinates, apart from the one extra constraint of a conflicting twin.
The proof has limits. Residual zero at one point is necessary, not
sufficient: a wrong equation can vanish there. Laman's `sense` on tangent and
`point_line_distance` constraints and the arc angles are inferred by the other
solvers from the starting guesses, so the guess side matters. `ezpz` exposes
no residual or DoF: its `translation_residual` is the largest variable
movement. `acs-solver`'s 0.0 is a placeholder, because it skips components
under its `TOLF` = 1e-10. The tolerance is an absolute 1e-9 stand-in, as no
`expected.json` carries one. The reports use std `hypot`/`atan2`, so byte
identity is observed on the reference machine, not guaranteed across targets. The
comparison is on status, DoF and conflict sets, never coordinates
(SEED.md §7). All thirty runs solved. To rerun: `tools/oracle/run.sh`, then
`python3 tools/oracle/compare.py` prints this table from the committed
reports (`tools/oracle/results/`; the matrix of what each API can express is
in `tools/oracle/README.md`).

Versions: sketch-solver 0.1.0, acs-solver 0.1.19, ezpz 0.2.29.

| Fixture | Solver | Status | DoF | Redundant | Conflicts | Verdict |
|---|---|---|---|---|---|---|
| `belt` | laman (expected) | FullyConstrained | 0 | none | none | |
| | sketch-solver | FullyConstrained | 0 | none | none | agrees |
| | acs-solver | FullyConstrained | 0 | none | none | agrees |
| | ezpz | FullyConstrained | not reported | not reported | not reported | agrees |
| `belt-conflicting` | laman (expected) | Unsatisfiable | 0 | none | {r1, r2, span, top-length, top-on-c1, top-on-c2, top-tangent-c1, top-tangent-c2} | |
| | sketch-solver | Unsatisfiable | 0 | none | {r1, r2, span, top-length, top-on-c1, top-on-c2, top-tangent-c1, top-tangent-c2} | agrees |
| | acs-solver | Unsatisfiable | 0 | none | {r1, r2, span, top-length, top-on-c1, top-on-c2, top-tangent-c1, top-tangent-c2} | agrees |
| | ezpz | unsatisfied | not reported | not reported | not reported | agrees |
| `belt-redundant` | laman (expected) | FullyConstrained | 0 | {same-belt} | none | |
| | sketch-solver | FullyConstrained | 0 | {same-belt} | none | agrees |
| | acs-solver | FullyConstrained | 0 | {same-belt} | none | agrees |
| | ezpz | FullyConstrained | not reported | not reported | not reported | agrees |
| `slotted-plate` | laman (expected) | FullyConstrained | 0 | none | none | |
| | sketch-solver | FullyConstrained | 0 | none | none | agrees |
| | acs-solver | FullyConstrained | 0 | none | none | agrees |
| | ezpz | FullyConstrained | not reported | not reported | not reported | agrees |
| `slotted-plate-conflicting` | laman (expected) | Unsatisfiable | 0 | none | {same-ends, slot-r, slot-r2} | |
| | sketch-solver | Unsatisfiable | 0 | none | {same-ends, slot-r, slot-r2} | agrees |
| | acs-solver | Unsatisfiable | 0 | none | {same-ends, slot-r, slot-r2} | agrees |
| | ezpz | unsatisfied | not reported | not reported | not reported | agrees |
| `slotted-plate-drag` | laman (expected) | UnderConstrained | 3 | none | none | |
| | sketch-solver | UnderConstrained | 3 | none | none | agrees |
| | acs-solver | UnderConstrained | 3 | none | none | agrees |
| | ezpz | UnderConstrained | not reported | not reported | not reported | agrees |
| `slotted-plate-redundant` | laman (expected) | FullyConstrained | 0 | {level} | none | |
| | sketch-solver | FullyConstrained | 0 | {level} | none | agrees |
| | acs-solver | FullyConstrained | 0 | {level} | none | agrees |
| | ezpz | FullyConstrained | not reported | not reported | not reported | agrees |
| `sss-triangle` | laman (expected) | FullyConstrained | 0 | none | none | |
| | sketch-solver | FullyConstrained | 0 | none | none | agrees |
| | acs-solver | FullyConstrained | 0 | none | none | agrees |
| | ezpz | FullyConstrained | not reported | not reported | not reported | agrees |
| `sss-triangle-conflicting` | laman (expected) | Unsatisfiable | 0 | none | {apex, base, right} | |
| | sketch-solver | Unsatisfiable | 0 | none | {apex, base, left, right} (superset: +left) | differs: conflicts (superset) |
| | acs-solver | Unsatisfiable | 0 | none | {apex, base, left, right} (superset: +left) | differs: conflicts (superset) |
| | ezpz | unsatisfied | not reported | not reported | not reported | agrees |
| `sss-triangle-redundant` | laman (expected) | FullyConstrained | 0 | {right-angle} | none | |
| | sketch-solver | FullyConstrained | 0 | {right-angle} | none | agrees |
| | acs-solver | FullyConstrained | 0 | {right-angle} | none | agrees |
| | ezpz | FullyConstrained | not reported | not reported | not reported | agrees |

2 rows differ from laman's expectation.

`ezpz` exposes no DoF and no sets (its reports say "not reported"): it
compares on satisfaction only, and its "unsatisfied" does not tell a
conflict from a failure to converge. Laman's own row is each fixture's
`expected.json`, derived by construction, not laman's output; laman's solve
meets it (laman's fixture tests).

### Where the solvers differ from laman

Of the thirty reports, one fixture differs from laman's expectation:
`sss-triangle-conflicting`. `sketch-solver` and `acs-solver` both name the
set {`apex`, `base`, `left`, `right`}; laman names {`apex`, `base`, `right`}.
Laman's set is the conflict set, checked by hand against the construction
(the fixture has `a` = (50, 0), `b` = (18, 24)): `base` fixes `a` 50 from the
origin `o`, `apex` puts `b` on the 60° line through `o`, and `a` is
50·sin 60° = 43.3 from that line while `right` allows `|ab|` = 40
(fixture 1 in ROADMAP.md). These three are contradictory whatever `left`
says, and none of them depends on the rotation `horizontal` removes. For
inclusion-minimality every pair must also be satisfiable: `base` + `right`
+ `left` is the triangle, `base` + `apex` + `left` puts `b` 30 along the 60°
line, and `right` + `apex` leaves `|oa|` free. The solvers' set is what
remains at the least-squares point, where the four rotation-invariant
constraints share the residual (the reasoning is in the `c1_conflicting_twins`
test in `crates/laman/src/diagnose/analysis/tests.rs`). It is an
unsatisfiable superset of laman's set, not a minimal one, and so not a
conflict set in this project's sense (GLOSSARY.md). `ezpz` agrees on
satisfaction and exposes no sets. No disagreement is laman's fault, so no
regression fixture is made.

## What is not claimed

- No benchmark: the run above compares status, DoF and conflict sets on
  ten small sketches, not coordinates, speed or robustness on larger ones.
  `SolveSpace` and `PlaneGCS` were not run (the Python column is from their
  documentation).
- No capability laman does not have: the solver, the drag, the Python
  binding and the checker exist; cross-OS determinism is checked in CI.
  Agreeing with three solvers on ten sketches is evidence for those sketches.
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
