# Plan: m0-model-harness

- Started: 2026-11-01
- Milestone: M0 (cycle C1, docs/ROADMAP.md §M0)
- Idea: none; positioning settled (C1 as planned, e3f4c22). Verbatim from
  the human: "/plan M0"

## Goal

Everything needed to judge a solve exists before the solver. `laman::model`
builds, validates and serialises every C1 entity and constraint;
`laman-check` measures each one from DATA-MODEL.md without touching any
residual; `laman-debug` loads, dumps (text, SVG) and digests fixtures. The
four C1 fixtures and their twins are committed and the checker proves their
expected coordinates. The corpus digests match natively and on
wasm32-wasip1, and a fuzz target loads any bytes without panicking.

## Non-goals

- Residuals, `system`, `solve`, `diagnose`, `drag`, and `Solution`/`Status`
  (M1–M3).
- The Python binding of the model (M4): `laman-py` is untouched, and adding
  types breaks nothing.
- Publishing `laman-check` (`docs/BACKLOG.md`): `publish = false` until
  then.
- The tolerance ADR (M2): M0 uses named constants (*Open questions*).

## Design deltas

- **`laman::model`** (new, public; ARCHITECTURE.md §The sketch and its ids):
  `EntityId`, `ConstraintId`, handles `Point`/`Line`/`Circle`/`Arc`,
  `Coordinate` (`X`, `Y`, `R`), `Entity`, `Constraint` (every DATA-MODEL.md
  row, matched without `_ =>`), `Sense`, `Payload`, `Coordinates`
  (`BTreeMap<(EntityId, Coordinate), f64>`), `Sketch` (builders,
  `*_with_id`, `tag`, `payload`, `set_fixed`, `remove`,
  `remove_with_constraints`, `revision`), `ModelError` (`UnknownId`,
  `WrongKind`, `NonFinite`, `DimensionOutOfRange`, `DuplicateId`,
  `RepeatedEntity`, `InUse`, `FormatVersion`), `model::tolerance`.
- **Senses move from M1 to M0**: the builder must not change signature in
  M1, and a sense is a cross-product sign or a distance comparison, not a
  residual. A centre exactly on the line takes σ = +1. Circle–circle contact
  is external when `|c_X − c_Y|` is nearer `r_X + r_Y` than `|r_X − r_Y|`,
  and a tie is external. Senses are stored in the serde form and computed on
  load when missing.
- **Serde** (feature `serde`, default on): DATA-MODEL.md §The serde form
  exactly. Payloads are base64 (`base64`). `version: 1` with a migration
  seam and no migrations yet. Loading replays the builder.
- **`laman-check`** (new crate; deps: `laman`'s model, `libm`):
  `check(&Sketch, &Coordinates) -> Report`, where `Violation { id, tag,
  kind, measured, wanted }` and degenerate entities are listed. It measures
  meaning, not residuals: angles by `libm::atan2`, tangency as
  |centre–line distance| = r on σ's side, parallel as an angle mod π.
  `tools/check-layers.sh` (in the gate) refuses any other `laman::` path in
  its sources.
- **`laman-debug`** (new crate, `publish = false`):
  - `fixture`: loads `sketch.json`, `expected.json` and `drag.json`
    (ROADMAP.md §Fixtures).
  - `dump::text`, and `dump::svg` (construction geometry dashed, tags as
    labels, violations red).
  - `digest`: a SHA-256 builder (`sha2`) fed f64 bits, ids and strings in
    order. In M0 it covers the loaded sketch plus the report at the
    expected coordinates; M2 feeds it a `Solution`.
  - `strategy`: `proptest` strategies for well-formed sketches.
- **Fixture tests** (`crates/laman-debug/tests/c1.rs`): the checker tests
  run now. The solve-and-compare tests are written against the runner,
  with the solve stubbed to return the guesses, under
  `#[ignore = "solver: M2"]`; the drag tests are `#[ignore = "drag: M3"]`.
- **Determinism tooling**: `tools/digest-wasm.sh` runs the corpus on
  wasm32-wasip1 under `wasmtime` and diffs it against native. CI gets a
  `wasm` job running it, and `rust-toolchain.toml` adds `wasm32-wasip1`.
- **Fuzz**: `fuzz/` (outside the workspace), with the target
  `load_sketch`: bytes → `serde_json` → loader → `ModelError` or a sketch.
- **No ADR**: everything above applies the design docs. The sense move is a
  rescope within the cycle, recorded here.

## Steps

Grades are tiers (`.agents/tiers.md`): **[1]** routine, the design says
exactly what to write; **[2]** careful, a numeric or geometric case to get
right; **[3]** unproven. No step's only test is CI (the wasm script runs
locally first), so the plan stays on `main`.

- [x] Step 1 **[2]**: `laman::model` entities: ids, handles, `BTreeMap`
  storage, points/lines/circles/arcs, fixed parameters, tags, payloads,
  construction, `revision`, `remove`/`InUse`/cascade. A unit test per
  entity-level `ModelError`; doctests on every public item.
- [x] Step 2 **[2]**: the 18 constraint kinds, their builders and refusals
  (`WrongKind`, `RepeatedEntity`, `DimensionOutOfRange` naming
  `coincident`/`point_on_line`, `NonFinite`), senses from the drawing,
  `model::tolerance`. A test per refusal and per sense rule, ties included.
- [x] Step 3 **[2]**: the serde form (versioning, `FormatVersion`, base64,
  defaults omitted, load via the builder); the `laman-debug` skeleton with
  `strategy`. The byte-for-byte round-trip property over 1 000 cases, and a
  test per malformed file naming its id.
- [x] Step 4 **[2]**: `laman-check`: every constraint and the arc's
  intrinsic equation, each tested on a satisfied and a violated hand-built
  configuration; degenerate entities. `tools/check-layers.sh` in the gate,
  proven red once on a planted import.
- [x] Step 5 **[1]**: `laman-debug` `fixture` loader, `digest` (with a
  pinned test vector), and the runner that loads, checks and digests.
- [ ] Step 6 **[2]**: `dump::text` (snapshot-tested) and `dump::svg` (view
  box from the bounds, arcs counter-clockwise from `s` to `e`).
- [ ] Step 7 **[2]**: fixtures `sss-triangle` and `belt` with their twins
  under `tests/fixtures/c1/`, with values from ROADMAP.md §C1 acceptance
  (the belt's to 17 digits from `s = 0.1`). `c1.rs`: the base and redundant
  twins pass at the expected coordinates; each conflicting twin flags only
  its added constraint (`apex`, `top-length`); the solve tests are ignored.
- [ ] Step 8 **[2]**: fixtures `slotted-plate` (its conflicting twin flags
  `slot-r2`) and `slotted-plate-drag` (`drag.json` with k = 1…10, the
  checker passing each closed-form step). Each fixture's SVG is looked at
  once (`inspect`).
- [ ] Step 9 **[2]**: the digest corpus test with its digests blessed in
  `laman-debug`; `tools/digest-wasm.sh`; the CI `wasm` job; `wasm32-wasip1`
  in `rust-toolchain.toml`. Green locally under `wasmtime`.
- [ ] Step 10 **[1]**: `fuzz/` with `load_sketch`, seeded from the fixture
  `sketch.json` files. A 10-minute nightly run with no panic; any crash
  becomes a regression fixture.

## Acceptance

- The gate is green, with the solve and drag tests ignored.
- The checker finds no violation at the expected coordinates of the four
  fixtures and the redundant twins, nor at any of the drag's 10 steps. For
  each conflicting twin it flags exactly `apex`, `top-length` and
  `slot-r2`.
- Every `sketch.json` round-trips byte for byte, and the serde property
  holds over 1 000 cases.
- `tools/digest-wasm.sh` prints identical digests natively and on
  wasm32-wasip1 for every fixture and twin.
- `tools/check-layers.sh` is green, and was red on the planted import.
- `cargo +nightly fuzz run load_sketch -- -max_total_time=600` finds no
  crash.

## Docs to update on completion

- `docs/ROADMAP.md` §M0/§M1: senses moved to M0; M0's status line.
- `docs/ARCHITECTURE.md` §Crates: `laman-check` is `publish = false`
  until the solver's release, and `laman-debug`'s modules. §Determinism:
  what the M0 digest covers until `Solution` exists.
- `docs/DATA-MODEL.md` §Constraints: the sense rules (ties, contact kind).
  §The serde form: the migration seam.
- `GLOSSARY.md`: no new term expected. Add **Coordinates** if the type
  needs a definition.
- `CHANGELOG.md` §Unreleased: the model API, the serde form and the
  checker.
- `AGENTS.md` "Current state": M0 landed; next is `/plan` M1.

## Open questions

- `⚠ OPEN:` the checker's tolerances before the M2 ADR. Agent's pick, by
  step 2: `model::tolerance::LENGTH = 1e-9`, absolute in the caller's units
  (the C1 fixtures are ~100 units across, so this is close to ROADMAP.md's
  "1e-9 of the sketch's size"), and `ANGLE = 1e-12` rad for the
  dimensionless rows. The M2 ADR replaces both.
- `⚠ OPEN:` `wasmtime` is not on this machine. Agent's pick: in step 9,
  install its release binary locally; CI uses
  `bytecodealliance/actions/wasmtime/setup`. The CI job is first green on
  the human's next push.
