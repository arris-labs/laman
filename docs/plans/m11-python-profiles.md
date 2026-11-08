# Plan: m11-python-profiles

- Started: 2026-11-08
- Milestone: M11 (cycle C3, docs/ROADMAP.md)
- Idea: none; verbatim from the human: "m11". The roadmap's C3 *In* list
  ("`laman-py` over the new API with stubs") and *Accept* list ("pytest
  reproduces every number") are the brief.

Under ~150 lines and ~10 steps; larger work is milestone plans.

## Goal
From Python, `laman.profile.profiles(sketch, coordinates)` returns the
same profiles as `laman::profile::profiles`, bit for bit, or raises
`laman.ProfileError` naming the entity. `laman.check.check_profiles`
re-measures any `Profiles` value, one built by hand in Python included, and
`laman.debug` digests profile runs and builds the slot grids. pytest runs
the C3 corpus at its expected coordinates and under both algorithms,
reproduces all 31 blessed digests in `tests/fixtures/c3/digests.txt`, the
roadmap's acceptance numbers and the slot grids' closed forms, and holds a
`hypothesis` property of nested shapes. Stubs pass `mypy.stubtest`, tests
pass `mypy --strict`. C3's last milestone has then landed.

## Non-goals
- No new Rust operation, and no change to `laman`, `laman-check` or the
  digests' bit layout. The binding computes nothing (ARCHITECTURE.md §The
  Python binding). So no fuzz target: `profiles_sketch` already fuzzes
  both Rust operations the binding calls.
- No timing from Python. The budget is ADR-0014's, measured in Rust.
- No wheel-matrix run on a branch: the five-platform wheels run the new
  tests at the next release (Open questions).
- The backlog's Python lines stay there (`DEFAULT_TOLERANCE` copied by
  hand, which `test_c3.py`'s area rule repeats). `/close-cycle` C3 follows
  the retirement in its own session.

## Design deltas
- **`laman.profile`** (new module, `python/laman/profile.py`, re-exporting
  `_laman`): `profiles(sketch, coordinates) -> Profiles`, and the frozen
  value classes `Traversal(entity, reversed)`,
  `Loop(edges, signed_area, joins)`, `Profile(outer, holes, net_area)`,
  `Branched(edges, joins)`, `Unjoined(first, second)` and
  `Profiles(profiles, open_chains, dangling, branched, degenerate,
  unjoined)`. Each has Rust's fields under Rust's names (ids as `int`),
  a keyword constructor (every list defaults to empty, so
  `Profiles()` is `Profiles::default()`), `__eq__` and `__repr__`.
  `Profiles` holds the Rust value, so a check and a digest read the same
  bits extraction wrote. Its getters build the field classes. Its
  constructor converts them back, and a negative or oversized id raises
  `OverflowError`, never a panic. A float crosses unchanged: `±f64::MAX`,
  `-0.0` and subnormals included.
- **`laman.ProfileError(ValueError)`** with `ProfileError.Missing` and
  `ProfileError.NonFinite`, each carrying `entity: int`, after the
  `ModelError.<Variant>` pattern (`_errors.py`, `errors.rs`). A variant
  the binding lacks (`ProfileError` is `#[non_exhaustive]`) reaches Python
  as the base class with the message.
- **`laman.check.check_profiles(sketch, coordinates, profiles) ->
  ProfileReport`**: `violations: list[ProfileViolation]`, `missing:
  list[Missing]` (the existing class), `is_clean()`. `ProfileViolation` is
  Rust's enum flattened, by the binding's rule for enums with data:
  `kind: str` (`area`, `net_area`, `orientation`, `not_chained`,
  `open_join`, `arc_off_radius`, `not_an_edge`, `repeated`, `uncovered`,
  `containment`), `first: int | None`, `entity: int | None`, `measured:
  float | None`, `reported: float | None`. It is matched without `_ =>`,
  so a new variant fails to compile.
- **`laman.debug`**: `digest_profiles(sketch, coordinates, profiles,
  report)` and `digest_solved_profiles(sketch, solution, report, profiles,
  profile_report)` (`of_profiles`, `of_solved_profiles`), and
  `slot_grid(rows, cols) -> tuple[Sketch, Coordinates]`, the corpus's
  sketch and its expected coordinates (`corpus::slot_grid`). ADR-0007's
  rule covers them: tooling, one definition, outside the stability promise.
  No ADR. ARCHITECTURE.md records them.
- **ARCHITECTURE.md §Errors** names `ProfileError`, which it has lacked
  since M9 ("No other error type" is stale): the Rust row and its Python
  subclasses.
- No public Rust type or signature changes. `crates/laman-py/src/` gains
  `profile.rs`. `lib.rs`, `check.rs`, `debug.rs` and `errors.rs` grow.

## Steps
Complexity grades; a step's grade is its tier, the model that executes it
(`.agents/tiers.md`), and `**[1, tier 3]**` overrides the tier: **[1]**
routine — the design says exactly what to write and the tests are
mechanical; **[2]** careful — a numeric or geometric case to get right
within a given design; **[3]** unproven — an algorithm whose robustness, convergence or
bound has to be established here.

Every step runs `tools/py-test.sh` before it commits (`.agents/rules/git.md`)
and updates `_laman.pyi` and the module docstrings with its code.

- [x] Step 1 **[2]** — `laman.profile`'s value classes and `Profiles`
  over the Rust value. Test: a `Profiles` built in Python round-trips to
  Rust and back equal, floats compared by `struct.pack` bits at `±f64::MAX`,
  `-0.0`, the smallest subnormal and `7200 - 400π`. A negative id raises
  `OverflowError`. `Profiles()` is empty.
- [x] Step 2 **[1]** — `laman.profile.profiles` and `laman.ProfileError`.
  Test: the 4 × 3 rectangle by hand gives one profile, net area 12.0. A
  removed coordinate raises `ProfileError.Missing` with its `entity`, a
  NaN raises `ProfileError.NonFinite`. Both are `isinstance` of
  `ProfileError` and `ValueError`. Docstring example runs.
- [x] Step 3 **[1]** — `laman.check.check_profiles`, `ProfileReport`,
  `ProfileViolation`. Test: the rectangle's found profiles check clean.
  A twin built in Python with area 13.0 gives exactly one `area` violation
  with `measured == 12.0` and `reported == 13.0`. A reversed outer gives
  `orientation`, a stale edge id `not_an_edge`, a removed coordinate one
  `missing`.
- [x] Step 4 **[1]** — `laman.debug.digest_profiles`,
  `digest_solved_profiles` and `slot_grid`. Test: `c3-rectangle` and
  `c3-slot-grid-3x3` equal their lines in `tests/fixtures/c3/digests.txt`.
  `slot_grid(7, 7)` has 498 entities.
- [x] Step 5 **[2]** — `tests/test_c3.py`, the corpus: every directory of
  `tests/fixtures/c3/` is listed. Every fixture, at its expected
  coordinates and solved under LM and dogleg, gives `expected.json`'s
  profiles (edges, joins and lists exact, areas by the fixture's rule), and
  `check_profiles` is clean. All 31 digests are reproduced, and the file
  holds exactly 31 lines. Named numbers: the rectangle's outer is 7200
  and counter-clockwise, and the slotted plate's net area is
  `7200 − (400 + 25π)`.
- [ ] Step 6 **[1]** — slot grids 1..7 × 1..7 from Python. One plate, a
  hole per slot, nothing else listed. Plate, hole and net areas match
  `corpus::slot_grid`'s closed form within `1e-9` relative, and the check
  is clean.
- [ ] Step 7 **[2]** — `f64`'s extremes from Python. A square of side 1e300
  saturates to `f64::MAX`, its net to `+f64::MAX`. One of side 1e-300
  gives `0.0` or a subnormal. The check is clean on both, and a reversed
  twin of each still raises `orientation` (ADR-0013 D3).
- [ ] Step 8 **[2]** — a `hypothesis` property, `test_profile_property.py`:
  disjoint and nested axis-aligned rectangles and circles, each built by
  construction. Profile and hole counts and every area come from the
  construction, and the check is clean. A twin built in Python with one loop's
  area nudged by 1e-3 of its magnitude (far past ADR-0013's allowance)
  raises `area` on that loop and `net_area` on its profile, nothing else.
- [ ] Step 9 **[1]** — README: a Python profiles example (the slotted plate
  or a rectangle with a round hole), held by `test_readme.py`. The status
  paragraph gains "read its profiles and re-check them".

## Acceptance
`tools/py-test.sh` green (stubtest and `mypy --strict` included) and the
gate green. In it: `test_c3.py` matches all 10 C3 fixtures at their
expected coordinates and under both algorithms, each checked clean, and
equals all 31 lines of `tests/fixtures/c3/digests.txt`. The rectangle is
7200, counter-clockwise. The slotted plate nets 6721.46…
(`7200 − 400 − 25π`). Slot grids 1..7 × 1..7 meet their closed forms.
1e±300 results reach Python bit for bit. The hypothesis property passes.
The C1 tests and their 50 digests are unchanged.

## Docs to update on completion
- `docs/ARCHITECTURE.md` §The Python binding: `laman.profile`,
  `check_profiles`, the profile digests and `slot_grid`. The C3 corpus is
  run by pytest. The wheels' runners run "all 81 digests" (50 + 31), not
  "all 50". §Errors: the `ProfileError` row, its Python subclasses, and
  "No other error type" corrected. §Profiles' "The Python package exposes
  it as `laman.check`" names `check_profiles`.
- `docs/ROADMAP.md` C3: M11 landed in the status line.
- `AGENTS.md` current state: M11 landed; Next: `/close-cycle` C3.
- `CHANGELOG.md` `Unreleased`: in Python, `laman.profile.profiles`, the
  `ProfileError` refusals and `laman.check.check_profiles`.
- `docs/BACKLOG.md`: the `DEFAULT_TOLERANCE` line also names `test_c3.py`'s
  copy of the fixture area rule.
- `GLOSSARY.md`: no new term. `ProfileViolation.kind` uses *open join*
  and *off-radius arc* as written there.

## Open questions
- ⚠ OPEN: run `wheels.yml` on `plan/m11-python-profiles` so the 31
  digests are seen equal from Python on five OS/arch runners before
  release? Agent's pick: no. It needs a push (asked first). The Rust
  digests are already equal natively and on wasm32, and the binding
  computes nothing. The release's wheel build runs the same tests on every
  runner. The human decides before step 5 whether to add a [CI] step.
- Settled (step 2): `ProfileError` is at the top level (`laman.ProfileError`,
  like `ModelError` and `RigidityError`) and re-exported in
  `laman.profile`, after `ModelError`'s precedent. It is a sibling of
  `ModelError`, not a subclass: both derive `ValueError`, and a profile
  refusal is not a sketch refusal. The classes are `ProfileMissing` and
  `ProfileNonFinite` in `_errors.py`, reached as `ProfileError.Missing` and
  `ProfileError.NonFinite`.
