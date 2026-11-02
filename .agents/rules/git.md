# Git: trunk-based, always green

- **Every commit** passes `.githooks/pre-commit`; never `--no-verify`. The
  hook runs `tools/gate.sh` unless the commit stages prose only (`*.md`,
  `.agents/`, `docs/`): fmt, clippy `-D warnings`, the workspace's tests
  and doctests, rustdoc with warnings denied, and the wasm32 build of
  `laman`. While the workspace is small the hook runs all of it; when the
  gate passes a minute, it is split by an ADR, not by skipping stages.
- **The Python binding** is tested by `tools/py-test.sh` (maturin into
  `crates/laman-py/.venv`, pytest, `mypy.stubtest`). The hook does not run
  it; a commit that touches `crates/laman-py/` or `laman`'s public API runs
  it before committing, and CI's `python` job runs it on every push.
- **No foreground waits.** A run expected to take over a minute, and a CI
  watch, runs with `run_in_background`, and the agent goes on with what
  does not depend on its result. A foreground `until`, `while` or `sleep`
  loop, or a `tail --pid`/`tail -f`, waiting on one is never written: the
  completion notification is the wait. In Claude Code `tools/bash-guard.sh`
  refuses it (a PreToolUse hook).
- Commit **directly to `main`**, one plan step per commit. Branch
  (`plan/<slug>`, rebased, `merge --ff-only`, deleted after) only when a
  plan may be thrown away or the human asked to review first.
- Never rewrite `main` (no amend of a pushed commit, no force-push, no
  rebase). Never push unless asked.
- Stage deliberately: read `git status` and `git diff`, no blind `git add
  -A`. No `git stash`: a pop can bring back another session's work
  (`tools/bash-guard.sh` refuses it). A changed fixture expectation
  (`tests/fixtures/**`) is staged only with an intentional change of
  behaviour, and the message says `fixtures:` and why.

## Message format

```
type(scope): imperative summary, lower case, no period  (plans/<slug> step N)

Why, not what. Cite the ADRs and docs that justify it or that it updated.
Name any public type or signature it changes.
```

- `type`: `feat`, `fix`, `refactor`, `perf`, `test`, `docs`, `chore`,
  `build`, `ci`, `fixtures`. `scope`: crate short names (`laman`, `check`,
  `debug`, `py`, several as `laman,py`) or `docs`, `plan`, `adr`, `tools`.
- The `(plans/<slug> step N)` suffix marks every plan-step commit;
  retirement is `docs: retire plan <slug>`.
- **Attribution**: a commit that repairs an earlier step's work carries
  that step's suffix, never a later one's; a fix found by the retirement
  review carries `(plans/<slug> review)`.
- Docs change in the **same commit** as the behaviour; a docs-only catch-up
  is `docs(sync): …`.
- No `Co-Authored-By` or tool trailers.

## Tags and publishing

- Publishing is `.github/workflows/release.yml`, run from a `vX.Y.Z` tag
  that the human pushes (`/release` walks it). A `workflow_dispatch` run
  with `dry_run` (the default) rehearses with temporary stable metadata
  and uploads nothing.
- `release.yml` refuses a tag that is not `v` plus `[workspace.package].version`
  with no `-dev`, or whose `CHANGELOG.md` lacks its `## X.Y.Z` heading
  (`tools/release-check.sh`). It then runs `ci.yml` and `wheels.yml`, and
  publishes: `publish-crate` (`cargo publish -p laman`, crates.io trusted
  publishing, GitHub environment `crates-io`) and `publish-pypi` (the
  wheels and sdist, PyPI trusted publishing, GitHub environment `pypi`).
  Each environment has a required reviewer: every upload waits for the
  human's approval.
- The trusted publishers and the environments' reviewers are configured
  in the crates.io, PyPI and GitHub web UIs by the human, before the first
  real release. An agent never publishes by hand, unless `release.yml` is
  down and the human asks: `cargo publish -p laman`, then the wheels and
  sdist built by maturin from `crates/laman-py` uploaded to PyPI.
- The crate and the Python package carry the same version.
- `laman-debug` and `laman-py` are `publish = false` on crates.io;
  `laman-py` is what PyPI's `laman` is built from. `laman-check` is not
  published (`docs/BACKLOG.md`).

### The version

- `main` carries the next version with `-dev`: `[workspace.package].version
  = "0.N.0-dev"`, and the internal crates pinned to exactly it in
  `[workspace.dependencies]` (`version = "=0.N.0-dev"`). The exact pin is
  the guard: a bump that misses one fails `cargo check`. PyPI reads it as
  PEP 440 (`0.1.0.dev0`).
- Pre-1.0, `0.y.z` is `y` breaking. **Closing a cycle bumps the minor**;
  **a release between cycles bumps the patch**, unless `CHANGELOG.md`'s
  `Unreleased` has a `### Breaking` bullet.
- The version lives only in `Cargo.toml` and `CHANGELOG.md`'s headings —
  no doc, README or rustdoc states it.

### The changelog

`CHANGELOG.md` is what a consumer reads.

- An entry is written in the commit that makes the change; a plan's
  bullets by its `/retire-plan` commit. Changes no consumer can see
  (process, tooling, tests, docs) write nothing.
- Bullets go under `## Unreleased`, for a reader who has read only
  `README.md`: what they can now do, the refusals they will hit, no ADR
  numbers, plan slugs or fixture names. A changed public type or signature
  goes under `### Breaking` with its one-line fix, in the same commit.

## Without asking

While executing a plan: run checks, commit each step, tick boxes, edit the
plan. Branching, tagging, pushing, resetting, rewriting and publishing are
asked first.
