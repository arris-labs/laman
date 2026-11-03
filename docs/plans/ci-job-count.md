# Plan: ci-job-count

- Started: 2026-11-03
- Milestone: none (tooling between C1 and C2)
- Idea: docs/ideas/ci-job-count.md (absorbed: options B and C, plus
  dropping the remote rehearsal, accepted in the retro of session
  14738f20)
- Runs on `plan/ci-job-count`: steps 2–5 are tested only by CI.

Under ~150 lines and ~10 steps; larger work is milestone plans.

## Goal
One commit stops being tested two or three times over. A push to `main`
runs `ci.yml` alone, 7 jobs (it was 28). A release push of `main` and the
tag runs about 31 jobs (it was 60): `ci.yml` on each of the two commits,
and `release.yml`, which builds the wheels once and publishes. The
evidence stays the same: Rust digests on five runners plus wasm32 on
every push, and the shipped wheel's corpus and digests on all five at
every release and every pull request that can change it. A dry run before
a release is a local packaging proof, not a 32-job dispatch.

## Non-goals
- `tools/ci-reuse.sh` (arris's ADR-0069, which skips long jobs when only
  the version changed): laman's CI takes 12 minutes, not hours.
- Fewer Rust runners: the five are the determinism evidence (SEED.md §5).
- The `release.yml` backlog line's other items (refuse a tag off `main`,
  SHA pins): they stay in the backlog, trimmed to them.

## Design deltas
- **ADR-0008, the release gate is the tag's own CI run.** `release.yml`
  no longer calls `ci.yml`. `ci.yml` triggers on `v*` tags too, and that
  run is the gate. `publish-crate`'s first step refuses unless
  `gh run list --workflow ci.yml --commit "$GITHUB_SHA"` shows a run that
  succeeded. The reviewer approves after CI is green, and an early approval
  fails safe: re-run the job once CI is green. Python runs on five platforms
  in the wheels only: on every pull request that touches the binding's
  inputs, and on every release. The ADR also records that the remote
  rehearsal is gone (next bullet). Alternatives: today's re-run (60 jobs),
  and arris's reuse script (a non-goal).
- **`ci.yml`**: on `push` to `main` and `v*` tags, and `pull_request`.
  `python` becomes a single job, Ubuntu with Python 3.10 (`tools/py-test.sh`,
  the source build with stubtest). The `metadata-artifact` input and its
  download steps go.
- **`wheels.yml`**: on `workflow_call`, `workflow_dispatch`, and
  `pull_request` with paths `crates/**`, `tests/fixtures/**`,
  `tools/py-*`, `Cargo.lock` and `.github/workflows/wheels.yml`. No longer
  on `push`. The `linux-x86_64` install job also runs
  `tools/py-package-test.sh` under Python 3.13 (option B: the shipped abi3
  wheel on the newest interpreter). The `metadata-artifact` input goes.
- **`release.yml`**: `wheels` and `crate-dry-run` no longer need
  `release-check`, so a `workflow_dispatch` on a branch builds and checks
  everything, and `release-check` (without a `v*` tag) refuses to publish
  anything (arris's arrangement). The `dry_run` input, its metadata overlay
  and `tools/release-dry-run.py` and `-test.py` go. Publishes need
  `release-check`, `crate-dry-run` and `wheels`.
- **`/release` step 1**: the rehearsal becomes local. `cargo package -p
  laman --locked`, then `maturin build --release` and `maturin sdist` in
  `crates/laman-py`, then `uv tool run twine check --strict` on both, all on
  the release commit before the tag goes out. A release that changed a
  workflow also dispatches `release.yml` on `main` (step 1's current text).
- `.agents/rules/git.md` §Tags and publishing and `docs/ARCHITECTURE.md`
  (the `python` matrix and `wheels.yml` paragraphs) say the above.

## Steps
Grades: **[1]** routine, **[2]** careful, **[3]** unproven; **[CI]** means
the only test is a CI run on `plan/ci-job-count`.

- [x] Step 1 **[1]** — ADR-0008 as in *Design deltas*, plus its row in
  `docs/adr/README.md`.
- [ ] Step 2 **[1, CI]** — `wheels.yml`: the new triggers, and the 3.13
  install on `linux-x86_64`. Test: a `workflow_dispatch` on the branch,
  12 jobs, green, and the 3.13 step's pytest summary in the log.
- [ ] Step 3 **[1, CI]** — `ci.yml`: the tag trigger and a single
  `python` job. Test: the branch push runs 7 jobs, green.
- [ ] Step 4 **[2, CI]** — `release.yml`: drop the `ci` call, make
  `wheels` and `crate-dry-run` independent of `release-check`, and add the
  green-CI check to `publish-crate`. Test: `gh workflow run release.yml
  --ref plan/ci-job-count` builds 14 jobs green, its `release-check`
  refuses, and nothing publishes.
- [ ] Step 5 **[1, CI]** — delete the `dry_run` input and the
  `metadata-artifact` plumbing from all three workflows, and delete
  `tools/release-dry-run.py` and `-test.py`. Test: step 4's dispatch again,
  plus `grep -r metadata-artifact .github tools` is empty.
- [ ] Step 6 **[1]** — `/release` step 1 gets the local packaging proof.
  `.agents/rules/git.md` §Tags is rewritten, and the `docs/BACKLOG.md`
  `release.yml` line is trimmed to its surviving items. Test: the proof's
  commands run green on the branch with the version overridden to stable
  in a scratch copy.

## Acceptance
- The branch's last push runs `ci.yml` with 7 jobs and no `wheels.yml`;
  a pull request to `main` from it runs both (7 + 12).
- A `release.yml` dispatch on the branch: 14 jobs (`release-check`, 12
  wheels jobs, `crate-dry-run`). `release-check` fails with "A real
  release requires a v* tag", and both publish jobs are skipped.
- `tools/gate.sh` green; `tools/release-check-test.sh` green.
- At the next real release (not this plan's to make): `ci.yml` runs on
  the tag, `publish-crate` finds it green, and `github-release` opens the
  Release. `/retire-plan` notes it as not yet observed.

## Docs to update on completion
- `docs/ARCHITECTURE.md`: the `python` matrix sentence (one job), the
  `wheels.yml` paragraph (triggers, the 3.13 install), and that the tag's
  CI run is the release gate.
- `.agents/rules/git.md` §Tags and publishing: no `dry_run`, the gate is
  the tag's CI, the local proof (step 6).
- `CHANGELOG.md`: nothing (no consumer sees CI).

## Open questions
- ⚠ OPEN: the `wheels.yml` pull-request paths. The agent decides by
  step 2: anything that can change the shipped package or its tests runs
  it, and prose cannot.
- ⚠ OPEN: whether `publish-pypi` repeats the green-CI check or relies on
  `publish-crate`, which it already needs. The agent decides by step 4;
  default: it relies on `publish-crate`.
