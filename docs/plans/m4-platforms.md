# Plan: m4-platforms

- Started: 2026-11-02
- Milestone: M4 (cycle C1, docs/ROADMAP.md), its platforms half
- Idea: none; the roadmap's M4 "In" list and two backlog lines (release
  automation; determinism on more than one OS), absorbed and removed
- Runs on `plan/m4-platforms`: steps 2–6 are tested only by CI runs, which
  need the branch pushed (the human approves the first push)

## Goal
The gate and the Python tests run green in CI on Linux (x86_64, aarch64),
macOS (aarch64, x86_64) and Windows (x86_64), so the 50 blessed digests in
`tests/fixtures/c1/digests.txt` are reproduced on every OS SEED.md §4
names, from `cargo test` and from pytest, beside the native/wasm32 diff
that already runs. Every push to `main` builds abi3 wheels for those five
platforms and the sdist, and each wheel is installed on a clean runner of
its platform and passes the corpus. A pushed `v*` tag publishes the crate
and the wheels through `release.yml`, each publish behind a reviewer, and
a `/release` skill walks the human through it. Nothing is published by this
plan: the release workflow is proven by a dry run.

## Non-goals
- The oracle runs (`sketch-solver`, `acs-solver`, `ezpz` under
  `tools/oracle/`) and the re-read of COMPARISON.md: the next plan,
  `m4-oracle`, which closes M4.
- The 0.1.0 release itself: `/close-cycle` and the human, after `m4-oracle`.
- musllinux, Windows aarch64, PyPy and free-threaded wheels; an MSRV
  (1.85) job; publishing `laman-check`: `docs/BACKLOG.md`.
- Any change to the solver, the digest or a fixture's contents. A digest
  that differs on another OS is a finding (below), not a re-bless.

## Design deltas
- **Line endings** — a new `.gitattributes` holds every text file at LF
  (`* text=auto eol=lf`). The digest hashes the sketch's serde text and
  `determinism.rs` embeds `sketch.json` with `include_str!`, so a Windows
  checkout with `core.autocrlf` would move every digest. A test asserts no
  embedded fixture contains `\r`.
- **CI `test` job** (`.github/workflows/ci.yml`) — a matrix over
  `ubuntu-latest`, `ubuntu-24.04-arm`, `macos-15` (aarch64),
  `macos-15-intel` (x86_64) and `windows-latest`, each running
  `tools/gate.sh` under `shell: bash` (Git Bash on Windows). The existing
  `check` job becomes this matrix; `wasm` stays Linux-only. The cross-OS
  comparison is each runner against the blessed `digests.txt`: no new
  digest tooling.
- **`tools/py-test.sh`** — runs on Windows: the venv's executables
  through `uv run --python .venv` (or `Scripts/` where `bin/` is absent),
  never a hard-coded `.venv/bin`. CI's `python` job runs it over the same
  OS matrix × Python 3.10 (the abi3 floor) and 3.13.
- **`.github/workflows/wheels.yml`** — reusable (`workflow_call`) and on
  push to `main`: `PyO3/maturin-action` builds abi3-py310 wheels for
  `manylinux2014` x86_64 and aarch64 (each on its native runner, in the
  manylinux container), macOS arm64 (`macos-15`) and x86_64
  (`macos-15-intel`), Windows x64, and the sdist.
  A second job per platform installs the wheel (not the source) into a
  fresh venv and runs `crates/laman-py/tests` against it, digests
  included. The sdist builds and passes the same tests on Linux.
- **`.github/workflows/release.yml`** — on a `v*` tag, or
  `workflow_dispatch` with `dry_run` (default true): checks the tag equals
  `[workspace.package].version` with no `-dev` and that `CHANGELOG.md` has
  its heading (`tools/release-check.sh`, also runnable locally); calls
  `ci.yml` and `wheels.yml`; then `publish-crate` (`cargo publish -p
  laman`, crates.io trusted publishing) and `publish-pypi`
  (`pypa/gh-action-pypi-publish`, PyPI trusted publishing), each in the
  GitHub environment `release` with a required reviewer. A dry run ends
  at `cargo publish --dry-run` and the built artefacts, never uploading.
- **`/release` skill** (`.agents/skills/release/SKILL.md`) — drops `-dev`
  in one commit with the `CHANGELOG.md` heading, lets the human tag and
  push, watches `release.yml` in the background, then sets `main` to the
  next `-dev` (patch or minor by `.agents/rules/git.md` §The version).
- **`.agents/rules/git.md` §Tags and publishing** — rewritten: publishing
  is `release.yml` from a tag the human pushes; by hand only if it is down.
- No public type or signature changes.

## Steps
Grades: **[1]** routine; **[2]** careful; **[3]** unproven. **[CI]**: the
only test is a CI run on `plan/m4-platforms`.

- [x] Step 1 **[1]** — `.gitattributes` at LF; `determinism.rs` asserts no
  embedded fixture holds `\r`; `git add --renormalize .` shows no change.
- [x] Step 2 **[2, CI]** — the `test` matrix over the five runners running
  the gate. Green means the 50 digests match on every OS; a mismatch is
  shrunk to a finding and a `regression/` fixture (`inspect`) and the plan
  stops for it (the riskiest unknown, so it goes first).
  Validated by CI run [41481525462](https://github.com/arris-labs/laman/actions/runs/41481525462)
  on `3a9fb44`: all five gates, Python and wasm green.
- [x] Step 3 **[1, CI]** — `py-test.sh` portable; the `python` job over the
  OS matrix × 3.10, 3.13; pytest's digests match on each.
  Run [41483531191](https://github.com/arris-labs/laman/actions/runs/41483531191)
  on `5f126f0` passed nine Python jobs and all five gates; macOS arm64
  Python 3.13 was cancelled before acquiring a runner (GitHub internal
  server error and capacity annotations). Retrying without changing
  implementation or acceptance.
  Validated by [41487227773](https://github.com/arris-labs/laman/actions/runs/41487227773)
  on `286a0bf`: all ten Python jobs green on the pinned runner matrix.
  Four native gates green; the pinned macOS arm64 gate is still queued
  and must be checked before merge.
- [ ] Step 4 **[2, CI]** — `wheels.yml`: the five wheels and the sdist built,
  each installed on a clean runner and its tests green. CHANGELOG bullet:
  wheels for the five platforms (glibc ≥ 2.17), not one.
- [ ] Step 5 **[1]** — `tools/release-check.sh` with its own shell test
  (accepts `v0.1.0` against `0.1.0`; refuses `-dev`, a mismatch, a missing
  heading); the gate runs that self-test as a stage.
- [ ] Step 6 **[2, CI]** — `release.yml`; a `workflow_dispatch` dry run on
  the branch is green end to end and uploads nothing.
- [ ] Step 7 **[1]** — the `/release` skill; `git.md` §Tags and publishing
  rewritten; `/next`'s "release due" reads the new rule.
- [ ] Step 8 **[1]** — merge: `plan/m4-platforms` rebased on `main`,
  `merge --ff-only`, the branch deleted (pushing `main` is the human's).

## Acceptance
On the plan's last CI run, all green:
- `test` on the five runners: the gate, including `determinism.rs`
  matching all 50 blessed digests and `c1.rs`'s every number;
- `python` on five runners × 2 versions: pytest reproduces the corpus and
  the same 50 digests; stubtest and `mypy --strict` green;
- `wasm`: `tools/digest-wasm.sh` unchanged and green;
- `wheels`: five wheels and an sdist, each installed from the artefact on a
  clean runner of its platform, corpus and digests green; the Linux wheels
  tagged `manylinux_2_17` (`auditwheel show`);
- `release` dry run: `release-check` passes, `cargo publish --dry-run`
  green, nothing uploaded; `release-check` refuses a `-dev` version.

## Docs to update on completion
- `docs/ARCHITECTURE.md` §Determinism — CI reproduces the blessed digests
  on the five runners named; §The Python binding — the wheel matrix.
- `docs/ROADMAP.md` §M4 status — platforms landed; the oracle to come.
- `docs/COMPARISON.md` — "without the cross-OS wheels" and "cross-OS
  determinism … do not yet" no longer true.
- `README.md` — the platforms wheels exist for, if it names any.
- `AGENTS.md` current state — M4 platforms landed; next `/plan m4-oracle`.
- `.agents/rules/git.md` — done by step 7.
- `CHANGELOG.md` — step 4's bullet.

## Open questions
- Settled: the human authorized pushing `plan/m4-platforms` and running
  CI, including macOS and Windows, before step 2.
- ⚠ OPEN: crates.io and PyPI trusted publishers for `release.yml`, and the
  `release` environment's reviewer, are set up in their web UIs. The
  human, before the first real release; the dry run does not need them.
- Settled: `macos-15-intel` is available and passed the native gate in
  step 2; no Rosetta fallback is needed.
- Settled: pin the arm64 macOS runner to `macos-15` in the native,
  Python and wheel matrices after `macos-latest` could not acquire a
  runner. GitHub's [runner reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)
  lists `macos-15` as standard arm64, and its
  [image table](https://github.com/actions/runner-images#available-images)
  maps `macos-latest` to macOS 26. Choosing the distinct macOS 15 image
  may avoid that image's capacity shortage (an inference, not a capacity
  guarantee); the five OS/architecture platforms stay the same.
