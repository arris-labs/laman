# Solver rules: what a library owes its callers

Rules an application never writes down. They apply to every crate; the
reasons are in `SEED.md` §5 and §9.

## Correctness

- **No panics on any input.** A malformed sketch (a dangling id, a
  constraint on the wrong kind of entity, a non-finite input) returns a
  typed error naming the caller's ids. `unwrap`/`expect`/`panic!` are
  clippy errors outside tests; indexing is safe only on an index the code
  validated. Panics are for laman's own invariants.
- **Statuses, not errors.** Under-constrained, over-constrained and
  unsatisfiable are a `Solution`'s status with best-effort coordinates and
  diagnostics. `Err` is for a malformed sketch only. "Require fully
  constrained" is a check on the result, never a solver mode.
- **The checker is independent.** `laman-check` depends on `laman`'s model
  types only, never on its solver or residual code. Every solve a test
  makes is checked by it; a test that expects violations names them.
- **Deterministic**: same input, same bits, on Linux, macOS, Windows and
  wasm32. No `HashMap`/`HashSet` iteration where order reaches a result or
  an id (`BTreeMap`, a `Vec`, or sort). A fixed summation order. No result
  that depends on threads. No randomness outside seeded tests.
  Transcendentals go through `libm`, pinned exactly; clippy holds it
  (`clippy.toml`). The digest compares native and wasm32.
- **f64 throughout.**
- **Tolerances are named, never a literal.** A `1e-9` in an algorithm is a
  bug; it is a named, commented constant in one place, with the reason.
  ⚠ OPEN: absolute or relative to the sketch's scale (SEED.md §9), closed
  by an ADR.
- **Every iteration is capped** and says what it returns when it does not
  converge: a status and the best iterate, never a hang.

## Testing

- **Every constraint's Jacobian is checked against finite differences**,
  in every configuration a test can reach (near-degenerate ones included).
- **Fixtures by construction or analysis**: expected coordinates, DoF and
  conflict sets come from a closed form or from how the sketch was built,
  never from laman's own output. An oracle (SolveSpace, PlaneGCS) runs
  out of process under `tools/oracle/` only when those run out, and is
  compared on DoF and conflict sets, not coordinates.
- **Every failure becomes a fixture**: reproduced, shrunk to a minimal
  sketch, committed under `tests/fixtures/regression/<slug>/` with the
  *desired* assertion and `#[ignore = "…"]`. The fixing commit lifts the
  ignore. Never kept as accepted behaviour, never deleted once fixed.
  Procedure: `inspect` skill.
- **Property tests over hand-picked cases**: random sketches built by
  construction (`proptest`, `hypothesis`), shrinking to fixtures.
- **No input panics** is tested by the fuzzer (`cargo fuzz`, outside the
  workspace), not asserted.

## API

- Doc comments say what an item guarantees and refuses; every public
  operation has a runnable example. Every crate forbids `unsafe`.
- **Entity and constraint enums are matched without `_ =>`** inside the
  workspace, so a new kind fails every dispatch to compile until it is
  handled. Clippy holds it (`wildcard_enum_match_arm`).
- **A break updates the binding in the same commit**: `crates/laman-py`,
  its `.pyi` stub and docstring examples; `tools/py-test.sh` holds them
  (pytest, `mypy.stubtest`).
- **A public type or signature change is a design delta** in the plan and
  is named in the commit body and the changelog. Allowed pre-1.0, never
  silent.

## References

- SolveSpace and PlaneGCS are read for ideas and mistakes and
  reimplemented for laman's model: no copied code, comments or
  identifiers, no transliteration. An ADR informed by one names the file
  read (`PROVENANCE.md`). Published algorithms are implemented from the
  paper and cited in the module that uses them.
