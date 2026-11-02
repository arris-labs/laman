# ADR-0007: The binding carries the checker, the dumps and the digest

- Status: accepted
- Date: 2026-11-02
- Closes: nothing from the kickoff; M4's Python half (docs/ROADMAP.md)

## Context

M4 asks pytest to run the C1 corpus from Python and to reproduce every
blessed digest, and the `hypothesis` property to check each solve with the
checker. The digest, the text and SVG dumps and the fixture runner live in
`laman-debug`, which ARCHITECTURE.md §Crates called "only ever a
dev-dependency". `laman-debug` also holds the `proptest` strategies, which
the extension module has no use for and which pull `proptest` and its
dependency tree into the wheel.

The alternatives: reimplement the digest in Python (a second definition of
the bits, which the digest exists to prevent), or move the digest and dumps
into `laman` (a public surface of tooling in the published crate).

## Decision

`laman-py` depends on `laman`, `laman-check` and `laman-debug`. `laman-py`
exposes `laman.check` and `laman.debug` (`text`, `svg`, `solution_text`,
`digest_run`, `digest_solution`, `digest_drag_step`); `laman.debug` is
tooling, outside the stability promise of the rest of the API.

`laman-debug` puts `strategy` and its `proptest` dependency behind a
default feature `proptest`; `laman-py` depends on it with
`default-features = false`. The tests of `laman-debug` that use the
strategies require the feature. `tools/check-layers.sh`, run by the gate,
refuses `proptest` in `cargo tree -p laman-py`.

The binding still computes nothing: each function calls the Rust one.

## Consequences

- One definition of the digest, tested natively, on wasm32 and from Python.
- `laman-debug` is a dependency of the Python wheel, though never published
  to crates.io; the wheel is built from source in the workspace.
- A new `laman-debug` dependency that is test-only must be optional behind
  the `proptest` feature or another, or `check-layers.sh` and the wheel's
  size notice it.

## Alternatives considered

- **Digest in Python**: two definitions of one bit layout; rejected.
- **Digest and dumps in `laman`**: tooling in the published crate's API;
  rejected.
- **Hand-written digests in pytest, no `laman.debug`**: the corpus could not
  prove the binding returns the solver's bits.
