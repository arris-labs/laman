# Laman

A deterministic 2D geometric constraint solver in pure Rust, for Rust and
Python: points, lines, circles and arcs; geometric and dimensional
constraints; a damped least-squares solve; and diagnostics that name what
is free, what is redundant and what conflicts. A library: any sketcher,
code-CAD tool or mechanism designer can build on it. It belongs to the
`arris-labs` family by conventions, not types: no dependency on arris or
arrix in either direction.

Read in this order: `SEED.md` (charter, competition, stack, the decisions
taken at kickoff), `docs/ARCHITECTURE.md`, `docs/DATA-MODEL.md`,
`docs/ROADMAP.md`, `docs/adr/`, then the rules in `.agents/rules/*.md`
(git, docs lifecycle, solver) — Claude Code loads them automatically via
`.claude/rules`; any other agent reads them here. `GLOSSARY.md` holds the
project's words, one meaning each: use them as written. Skills live in
`.agents/skills/` (same symlink arrangement): the idea → plan → work →
retire → close-cycle pipeline, `inspect` for seeing a sketch, and for the
human `/next`, `/handoff`, `/retro` and `/wait-what`.

## Setup (once per clone)

```sh
git config core.hooksPath .githooks   # the gate (tools/gate.sh) before every commit that touches code
rustup target add wasm32-unknown-unknown   # rust-toolchain.toml asks for it; the gate builds it
# Optional, only for the Python binding (tools/py-test.sh): uv
```

## Current state

**C1 closed and 0.1.0 published** (2026-11-03): the solver, its diagnostics,
`DragSession`, the checker and `laman-py` over the whole API, the corpus
green from Rust and Python, digests equal on five runners and wasm32
(ADRs 0001–0007, `docs/COMPARISON.md`).
**C2 closed** (2026-11-07, 0.2.0 published): stages (ADR-0009), edits keep
unreached bits, no radius below 0 (ADR-0010); at 500 entities every edit
and the slot drag are inside 4 ms, the rectangle drag accepted at 6.0 ms
(ADR-0011).
**C3 closed** (2026-11-08): `laman::profile::profiles` gives loops, holes and
open ends with declared joins (ADR-0012); `laman_check::check_profiles`
measures by sectors (ADR-0013); extraction at 500 entities inside 1.5 ms
(ADR-0014); profiles and their check in Python, 31 digests equal natively
and on wasm32.
**Next:** C4 (edits: intersections, split, trim, extend, mirror, offset),
proposed in `docs/ROADMAP.md` for the human to confirm; release 0.3.0.

## Rules that are not derivable from the code

- `laman-check` depends on `laman`'s model types only, never on its solver
  or residual code; that independence is what makes it a check (SEED.md §7).
- No panics on any input; typed errors naming the caller's ids. Under- and
  over-constrained are statuses with a best-effort `Solution`, never
  errors. Same input, same bits on every target. Details:
  `.agents/rules/solver.md`.
- Every failure becomes a fixture; acceptance is numbers (coordinates, DoF,
  conflict sets, digests), never a picture. The agent looks at a sketch
  itself with the `inspect` skill (dump it to SVG and look); the human has
  no better view of it.
- Decisions go in `docs/adr/`; `⚠ OPEN:` in a doc marks a deferred one.
  Kickoff decisions are in `SEED.md` §9 and are not re-litigated.
- Update this file's "Current state" when a milestone lands; keep it under
  ~15 lines — the roadmap holds the detail.
- Backlog line → `/idea` (brainstorm, `docs/ideas/`) → `/plan` (todo,
  `docs/plans/`) → `/work` (one step, one commit) → `/retire-plan` (the
  diff reviewed against the plan, docs updated, plan deleted);
  `/close-cycle` at a roadmap boundary. Not every idea becomes a plan.
  Details: `.agents/rules/docs-lifecycle.md`.
- `/work <slug>` with no step **conducts** the plan: each step a fresh
  subagent on its tier (`.agents/tiers.md` maps tiers to models), then the
  conducting session runs `/retire-plan`. Waits (gate, CI) run in the
  background, never in a foreground loop.
- Trunk-based git, `main` always green, commit per plan step, never push
  or publish unasked: `.agents/rules/git.md`. `main` carries the next
  version with `-dev`. A change a consumer would notice writes its
  `CHANGELOG.md` bullet under `Unreleased` in the same commit.
- SolveSpace and FreeCAD's PlaneGCS are read-only references: read for
  ideas and mistakes, never copied, never linked (`PROVENANCE.md`). Where
  their checkouts live on this machine: `docs/notes/reference-trees.md`,
  gitignored and machine-local, so a fresh clone asks the human. Tracked
  docs say "the reference trees", never a path.
