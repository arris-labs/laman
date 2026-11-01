# Backlog

One line per raw idea. Picking one up means `/idea` (needs thinking) or
`/plan` (obvious); the line is removed then — as is a line a roadmap cycle
has committed to, which now lives in `docs/ROADMAP.md` instead. Rejected
ideas keep one line below with the reason, so the same idea is not
re-brainstormed. A *measured* defect is not a raw idea: it goes under
[Findings](#findings) below, in the fixed shape.

- Release automation: a `release.yml` that publishes the crate and multi-platform abi3 wheels (Linux, macOS, Windows; manylinux older than 2_34) from a pushed `v*` tag, each behind an environment reviewer, and a `/release` skill, before the first 0.1.0. 0.0.1 went up by hand with one `manylinux_2_34_x86_64` wheel and the sdist
- `laman-check` as a crates.io name: publish it with the solver's first release, or keep it `publish = false` and say why in ARCHITECTURE.md
- Trademark search for "LAMAN" in software classes (WIPO, EUIPO, USPTO) before the solver's first public release (SEED.md §2); the human's
- The GitHub repository `arris-labs/laman` (the crates' `repository` URL and the README's SEED link point there); the human's
- `/retro`'s session digest: port arris's `session-digest` script when the first plan has sessions to mine
- Determinism on more than one OS: CI's test job on macOS and Windows beside Linux, once the digest exists

## Findings

A defect a run measured, one block each:

```
### <slug>
- What: …
- Where: crate, file, function
- Seen: run / commit / seed / date
- Reproduce: the command that replays it
- Measured: the numbers
- State: raw | measured | regression/<slug> | fixed
```

None yet.

## Rejected

- The working name `clevis`: taken on crates.io and PyPI (SEED.md §2)
