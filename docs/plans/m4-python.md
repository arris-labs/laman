# Plan: m4-python

- Started: 2026-11-02
- Milestone: M4 (cycle C1, docs/ROADMAP.md), first of three plans
- Idea: none; verbatim from the human: "/plan M4"

## Goal
`laman-py` covers the whole API: a `laman.Sketch` built by hand or loaded
from the serde form, solved, applied and dragged, its `Solution` and every
diagnostic readable, `laman.check` and the dumps and digests reachable.
pytest runs the C1 corpus (the four fixtures, the six twins, fixture 4's
ten drag steps, under both algorithms) and reproduces every number of the
C1 acceptance and every blessed digest; `hypothesis` builds random
sketches by construction and solves them back; the stubs pass
`mypy.stubtest` and the tests pass `mypy --strict`. The binding computes
nothing.

## Non-goals
- **m4-platforms** (next plan, runs on `plan/m4-platforms`, [CI]): CI's
  test and digest jobs on macOS and Windows, the abi3 wheel matrix, the
  release workflow (both `docs/BACKLOG.md` lines). Publishing stays the
  human's.
- **m4-oracles** (the plan after): `sketch-solver`, `acs-solver`, `ezpz`
  under `tools/oracle/` on the corpus; COMPARISON.md recorded and re-read.
- No Python-only behaviour: no convenience geometry, no NumPy arrays, no
  async. No solver change; a bug the binding exposes becomes a regression
  fixture (`inspect`), not a fix here.

## Design deltas
- **ADR-0007, the binding carries the checker, the dumps and the digest.**
  pytest must reproduce the blessed digests, so `laman.debug` exposes
  `laman-debug`'s digest and dumps. `laman-debug` puts `strategy` (and its
  `proptest`) behind a default feature `proptest`; `laman-py` depends on
  `laman`, `laman-check` and `laman-debug` with `default-features = false`.
  ARCHITECTURE.md §Crates: "`laman-debug` is only ever a dev-dependency"
  becomes "…except under `laman-py`, without `proptest`";
  `tools/check-layers.sh` refuses `proptest` in `cargo tree -p laman-py`.
- **Blessed digests move to `tests/fixtures/c1/digests.txt`**, one
  `<name> <hex>` per line (run, solved-lm/dl, drag-lm/dl-step-NN), with the
  conflicting twins' expected best-effort violations beside them in
  `violations.json`. `determinism.rs` reads them by `include_str!` (wasip1
  has no file system); pytest reads the same files. ROADMAP.md §Fixtures
  says so (it now says `expected.json` holds the digest; none does).
- **The Python API** (ARCHITECTURE.md §The Python binding grows to it):
  - `laman`: `Sketch`, handles `Point`/`Line`/`Circle`/`Arc` (frozen,
    `.id: int`, hashable); constraint ids are plain `int`. Entity builders
    take `*, fixed=False, construction=False, tag=None, payload=None,
    id=None`; constraint builders `*, tag=None, payload=None, id=None`.
    Method names one-for-one with Rust's; `horizontal(line)` or
    `horizontal(p, q)` for `Span`; `Sense` an `enum.Enum`, `Contact`'s
    `Internal(Sense)` flattened to `sense=`. `Sketch.from_json(str)`,
    `to_json()`, `revision`, `coordinates() -> Coordinates`.
  - `SolverOptions(**kwargs)` (frozen, defaults read from Rust's
    `Default`), `Algorithm` and `Status` as `enum.Enum` with Rust's names.
  - `Sketch.solve(options=None) -> Solution`, `apply(solution)`,
    `drag(point, options=None) -> DragSession` with `to(x, y) -> Solution`
    and `finish() -> Solution`; a call on a finished session raises
    `ValueError` (Rust moves the session; Python cannot).
  - `Solution` and every diagnostic (`Diagnostics`, `EntityRef`,
    `ConstraintRef`, `EntityFreedom`, `Motion`, `Degenerate`, `Flip`,
    `FlipSubject`) are frozen pyclasses mirroring the Rust fields; a tag is
    `str | None`, a payload `bytes | None`.
  - `laman.check.check(sketch, coordinates) -> Report` (and its
    `Violation`, `Degenerate`, `Missing`).
  - `laman.debug`: `text`, `svg`, `solution_text`, `digest_run`,
    `digest_solution`, `digest_drag_step` (hex `str`). Tooling: documented
    as outside the API's stability promise.
  - `ModelError(ValueError)` with one subclass per Rust variant, the ids as
    attributes (ARCHITECTURE.md §Errors gains the mapping).
- No public Rust type or signature changes.

## Steps
Grades: **[1]** routine — the design says exactly what to write and the
tests are mechanical; **[2]** careful — a numeric or geometric case to get
right within a given design; **[3]** unproven — an algorithm whose
robustness or bound is established here. Every step keeps `_laman.pyi`
current and `tools/py-test.sh` green.

- [x] Step 1 **[1]** — `fixtures:` the blessed digests and best-effort
  violations out of `determinism.rs` into `tests/fixtures/c1/` (no digest
  changes); `cargo test` and `tools/digest-wasm.sh` unchanged.
- [x] Step 2 **[2]** — ADR-0007; the `proptest` feature on `laman-debug`;
  `laman-py`'s dependencies; the `check-layers.sh` guard. Test: the gate,
  and `cargo tree -p laman-py` free of `proptest`.
- [x] Step 3 **[2]** — the spine, riskiest first: `from_json`/`to_json`,
  `SolverOptions`, `Algorithm`, `Status`, `Solution` (coordinates, status,
  DoF), `laman.check`, `laman.debug` digests. `tests/test_c1.py` with a
  Python fixture loader: the ten fixtures × two algorithms, status, DoF,
  coordinates within `1e-9` of the sketch's size, the checker's report
  (clean, or exactly the listed violations), each run and solved digest
  equal to `digests.txt`.
- [x] Step 4 **[2]** — every diagnostic: redundant and conflict sets by tag
  (the six twins), per-entity DoF and free directions (fixture 4: `Q` 2,
  the plate 1), degenerate entities, flips, tags and payloads round-trip.
- [x] Step 5 **[2]** — `apply` and the drag session: fixture 4's ten steps
  under both algorithms, every listed coordinate, `Q`'s bits, no flips,
  each step digest equal to `digests.txt`; `finish()` then `apply`; a
  stale `apply` raises `ModelError.Stale`; a finished session refuses.
- [x] Step 6 **[1]** — the builder: every entity and constraint method with
  its keywords, `set_fixed`, `set_construction`, `tag`, `payload`,
  `remove`, `remove_with_constraints`; the `ModelError` subclasses. Tests:
  fixtures 1–3 built by hand in Python have the loaded fixture's run
  digest; each `ModelError` subclass raised once.
- [ ] Step 7 **[1]** — the dumps (`text`, `svg`, `solution_text`) with tests
  (the SVG parses as XML and names each tag); `py-test.sh` adds
  `mypy --strict` over `tests/` and runs the docstring examples
  (`--doctest-modules`).
- [ ] Step 8 **[2]** — `hypothesis`: sketches built by construction from
  known coordinates (Henneberg-built distance frameworks with a ground
  point and a `horizontal`, then a random subset of distances dropped,
  each drop +1 DoF): the solve is checker-green with the construction's
  DoF, and two solves of one sketch have one digest. Example count
  measured by this step.
- [ ] Step 9 **[1]** — the user's view: README's Python example becomes the
  SSS triangle solved (run by pytest), `__init__.py` docstring and
  `pyproject.toml` description no longer say "rigidity only".

## Acceptance
`tools/py-test.sh` green: `test_c1.py` reproduces, from Python and under
both algorithms, every number of ROADMAP.md §C1 acceptance (coordinates,
statuses, DoF 0/0/0/3, the redundant tags `right-angle`, `same-belt`,
`level`, the three conflict sets, no flips, `Q`'s bits) and every one of
the corpus's blessed digests (10 run, 20 solved, 20 drag-step);
`mypy.stubtest laman` and `mypy --strict tests` clean; the `hypothesis`
property green at the step-8 count. `cargo test`, the gate and
`tools/digest-wasm.sh` unchanged.

## Docs to update on completion
- `docs/ARCHITECTURE.md` §Crates (laman-py's dependencies, the
  `proptest` feature), §The Python binding (the API above, `laman.debug`,
  the finished session), §Errors (the Python classes).
- `docs/ROADMAP.md` §Fixtures (where the blessed digests live); M4 status
  line: the Python half landed, platforms and oracles to come.
- `docs/COMPARISON.md`: "today it ships only the rigidity count" and
  §What is not claimed, true no longer once Python solves.
- `README.md` (step 9); `CHANGELOG.md` `Unreleased`: the Python sketch,
  solve, drag, check and dumps.
- `AGENTS.md` current state.
- `GLOSSARY.md`: none expected; a binding word that names a new concept
  gets an entry.

## Open questions
- ⚠ OPEN: plan order. Cross-OS determinism (m4-platforms) is arguably the
  riskier unknown, but its steps need pushes, which the human asks for;
  this plan runs locally, so it goes first. The human may swap them.
- ⚠ OPEN: the `hypothesis` example count: the agent, by step 8, sized so
  `py-test.sh` stays under a minute, with a `ci` profile that raises it.
- Settled (step 6): constraint ids stay `int`. `fixed=` is on `point` and
  `circle` only (a line or arc has nothing to fix); `tangent_*` take `sense=`
  (and `external=`) to override the drawing's reading. `IncompleteSolution`
  and `FixedMoved` cannot be raised from Python; Rust doctests hold them.
