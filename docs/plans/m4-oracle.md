# Plan: m4-oracle

- Started: 2026-11-03
- Milestone: M4 (cycle C1, docs/ROADMAP.md §M4, its last "Accept" items)
- Idea: none; verbatim from the roadmap: "`sketch-solver`, `acs-solver` and
  `ezpz` run out of process (`tools/oracle/`) on the four fixtures and their
  twins, with their status, DoF and conflict sets (not coordinates)
  recorded in COMPARISON.md", and "the comparison (COMPARISON.md) re-read
  against the code".

Under ~150 lines and ~10 steps; larger work is milestone plans.

## Goal
`tools/oracle/` is a Cargo project outside the workspace that translates each
of the ten C1 sketches (the four fixtures and six twins) into `sketch-solver`,
`acs-solver` and `ezpz`, solves it from the fixture's guesses, and writes one
report per solver and fixture: status, DoF, redundant and conflict sets by
laman's tags, or the kinds the solver cannot express. Each translation is
proven before its numbers count: the solver's own residuals are zero at the
fixture's expected coordinates. The reports are committed, COMPARISON.md
carries the table and explains every disagreement with laman, any
disagreement that analysis shows is laman's fault is a regression fixture,
and COMPARISON.md is re-read against the code. M4 closes, so C1 is ready
for `/close-cycle`.

## Non-goals
- SolveSpace (`python-solvespace`, `py-slvs`) and PlaneGCS (`planegcs`) as
  comparison rows: GPL/LGPL Python, not the Rust field the roadmap names. A
  `docs/BACKLOG.md` line ("run SolveSpace and PlaneGCS through
  `tools/oracle/` for the Python column"), not this plan.
- Coordinates, speed or frame budgets of the other solvers (SEED.md §7:
  two solvers may pick different branches; ROADMAP.md: speed past budgets).
- Fixture 4's ten drag steps: the three APIs drag differently or not at
  all. Fixture 4 is compared as a static solve (DoF 3, `UnderConstrained`).
- The oracle in the gate or in CI: it builds third-party crates over the
  network; it is run by hand (`tools/oracle/run.sh`) and its reports are
  committed. A CI job is a backlog line if a drift ever matters.
- Fixing another solver; a bug found in one is noted in COMPARISON.md
  (and reported upstream only if the human asks).

## Design deltas
- **Crate boundary.** `tools/oracle/` is its own workspace root like
  `fuzz/` (`[workspace]` in its manifest, its own `Cargo.lock`), `publish =
  false`, never a dependency of any crate here (PROVENANCE.md). It depends
  on `laman` for the model and serde form only and on `laman-check` to
  validate translations; never on `laman`'s solver (the same independence
  as the checker: `tools/check-layers.sh` extends its grep to
  `tools/oracle/src`). The three solvers are pinned exactly (`=x.y.z`) at
  the versions step 1 records.
- **Layout.** `src/main.rs` (CLI: `oracle <solver> <fixture-dir>` → JSON on
  stdout), `src/report.rs` (the report type), one module per solver
  (`sketch_solver.rs`, `acs.rs`, `ezpz.rs`), each a `translate` from
  `laman::model::Sketch` plus a `diagnose` into the report.
  `tools/oracle/run.sh` runs every solver on every C1 fixture, each in its
  own process with a timeout (a panic or hang in a foreign solver is a
  report, `"outcome": "crashed" | "timeout"`, not a red run), and writes
  `tools/oracle/results/<solver>/<fixture>.json`.
- **The report** (JSON, one per solver × fixture): `solver`, `version`,
  `fixture`, `outcome` (`solved` | `unsupported` | `crashed` | `timeout` |
  `error`), `status` mapped to laman's four statuses with the solver's own
  word kept beside it, `dof` (or `null` when the solver does not report
  it), `redundant` and `conflicts` as laman tags (untagged constraints by
  `#id`), `unsupported` (laman kinds with no translation, and how each was
  approximated if it was), `translation_residual` (max over the solver's
  equations at the expected coordinates, step 2's check).
- **A translation is trusted** only when the solver's residuals at the
  fixture's expected coordinates are within the fixture's tolerance; a
  translation that fails it reports `error`, never numbers. A kind with
  no direct counterpart may be built from the solver's primitives only if
  it keeps the equation count (so DoF still compares); otherwise
  `unsupported`.
- **Comparison.** `tools/oracle/compare.py` (stdlib only) reads the
  reports and the fixtures' `expected.json` and prints the COMPARISON.md
  table: per fixture, laman's expected status/DoF/sets beside each
  solver's, a disagreement marked. Conflict sets compare as sets of tags;
  a solver returning a superset or a different minimal set is recorded as
  such, not as wrong.
- **COMPARISON.md** gains §The comparison run (the table, the versions, the
  date, how to rerun, each disagreement explained by analysis), and its
  "nothing here was run" lines become true statements.
- **PROVENANCE.md**: the three crates are run, not read for ideas or
  copied; their licences (MIT, MIT OR Apache-2.0) named.
- **GLOSSARY.md, Oracle**: sharpened to cover both uses, expectations when
  analysis runs out (SolveSpace, PlaneGCS) and the comparison run (the
  Rust field); the entry keeps "compared on status, DoF and conflict sets,
  not coordinates".
- No public type or signature of `laman` changes; no changelog bullet
  (nothing a consumer sees).

## Steps
Complexity grades; a step's grade is its tier (`.agents/tiers.md`):
**[1]** routine; **[2]** careful, a numeric or geometric case to get right
within a given design; **[3]** unproven.

- [x] Step 1 **[2]** — The expressibility matrix, the riskiest unknown
  first: for each solver, pin the current crates.io version, build a
  hello-world against it in `tools/oracle/`, and table each C1 constraint
  and entity kind (arc as centre + endpoints, line–arc tangent,
  `symmetric` about a line, `midpoint`, `point_line_distance`, `angle`,
  construction lines, fixed points) against its API, and which of status,
  DoF, redundant and conflict sets it exposes. Commit the skeleton and the
  matrix as `tools/oracle/README.md`. A solver that can express none of
  the fixtures stays a row with that finding.
- [x] Step 2 **[1]** — The report type, the CLI, `run.sh` with per-process
  timeout and crash capture, `check-layers.sh` extended to
  `tools/oracle/src`; a test that a stub solver's report round-trips.
- [x] Step 3 **[2]** — `sketch-solver` adapter: translate, the
  translation-residual check on the four bases, diagnose, ten reports.
- [x] Step 4 **[2]** — `acs-solver` adapter, the same.
- [x] Step 5 **[2]** — `ezpz` adapter, the same.
- [x] Step 6 **[1]** — `compare.py` and the committed reports under
  `tools/oracle/results/`; its output is the table.
- [x] Step 7 **[2]** — Every disagreement analysed by hand against the
  fixture's construction (ROADMAP.md §C1 acceptance gives the reasoning):
  a solver's or a translation's fault is written in COMPARISON.md; one that
  is laman's becomes `tests/fixtures/regression/<slug>/` with the desired
  assertion, `#[ignore]`, and a `docs/BACKLOG.md` finding.
- [ ] Step 8 **[2]** — COMPARISON.md: §The comparison run with the table,
  versions and date; the whole file re-read against the code and the
  changelog (the roadmap's accept item), each row that stopped being true
  fixed; PROVENANCE.md and GLOSSARY.md as in the deltas.

## Acceptance
`tools/oracle/run.sh` then `python3 tools/oracle/compare.py` reproduces,
from a clean `tools/oracle/target`, the committed reports and the table in
COMPARISON.md byte for byte (the three solvers' versions pinned). Thirty
reports exist (3 solvers × 10 fixtures), each `solved` with a translation
residual within tolerance, or `unsupported`/`crashed`/`timeout` with its
reason; every `solved` report whose status, DoF or sets differ from
`expected.json` has a paragraph in COMPARISON.md. `tools/check-layers.sh`
green with the oracle's sources included; the gate unchanged otherwise.

## Docs to update on completion
- `docs/COMPARISON.md` — done in step 8; the intro's "nothing here was
  run" and §What is not claimed made true.
- `docs/ROADMAP.md` §M4 — status: the comparison run landed; M4 landed.
- `docs/ARCHITECTURE.md` §Crates and the layer rule — `tools/oracle/`
  outside the workspace, model and checker only.
- `PROVENANCE.md` — the three solvers run, not read.
- `GLOSSARY.md` — **Oracle** sharpened.
- `docs/BACKLOG.md` — the SolveSpace/PlaneGCS line; any finding from step 7.
- `AGENTS.md` current state — M4 landed; **Next:** `/close-cycle` C1.

## Open questions
- ⚠ OPEN: a solver that cannot express a kind (say `symmetric` about a
  line, or arcs as centre + endpoints): approximate it from primitives
  with the same equation count, or report `unsupported`. Agent decides per
  kind in step 1, recorded in the README's matrix; default `unsupported`
  unless the equivalence is exact.
- ⚠ OPEN: a solver's status vocabulary that has no clean map to laman's
  four (e.g. "failed" for both conflicting and not converged). Agent
  decides in step 2: the report keeps the solver's word and maps only what
  is unambiguous, `null` otherwise.
- ⚠ OPEN: whether COMPARISON.md names a laman shortfall the run exposes in
  plain words. Human decides at step 8 if one appears (positioning); the
  default is yes, it is a survey, not marketing.
