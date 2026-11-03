# Idea: ci-job-count

- Status: Open
- Raised: 2026-11-03
- Prompt (verbatim from the human): "why do we have so much jobs? 46 - even more complicated arris has much less" and "Do we really need both python 3.10 and 3.13? Why not just 3.10?"

## Problem
A tagged release run fans out to about 46 jobs, almost all of it one
five-platform matrix repeated by every stage. Cost is runner minutes and a
long, noisy run to read when something fails, not correctness. Repeats:

- `ci / python`: 5 platforms × Python 3.10 and 3.13 = 10 jobs.
- `wheels / test` installs each wheel on the same 5 platforms that
  `ci / python` just exercised.
- `release.yml` runs `ci.yml` again on a commit that already passed it on
  push.

## Constraints it runs into
- The five runners are the determinism evidence: the C1 corpus digests must
  agree on Linux x86_64 and aarch64, macOS arm64 and x86_64, Windows, plus
  wasm32 (SEED.md §5, `.agents/rules/solver.md`). Platform jobs stay.
- The shipped wheel is abi3 (`requires-python >= 3.10`): one wheel per
  platform serves every newer Python.
- `.agents/rules/git.md`: a release publishes only after CI and wheels pass
  from the tag; a cut must keep that gate.

## Options
### A — One Python version per platform, 3.13 once
`ci / python`: 3.10 on all five, 3.13 on Linux x86_64 only (10 → 6 jobs).
3.13 keeps what only it gives: `mypy.stubtest` against the newest
typeshed/stdlib and early notice of a PyO3 or abi3 break on a new
interpreter. Cost: one workflow edit.

### B — A, with the 3.13 check on the shipped wheel
As A, but `wheels / test` on Linux x86_64 also installs the built wheel under
3.13 and runs the package checks, and the source-build 3.13 leg is dropped.
Today nothing installs the wheel users download under anything but 3.10, so
abi3's forward promise is tested on a source build, not the artifact. Cost:
a second `setup-uv` step in one job; `ci / python` becomes 5 jobs.

### C — A or B, plus skip the repeated CI on a tag
`release.yml` stops calling `ci.yml` and instead requires that the tagged
commit already has a green `ci.yml` run on `main` (`gh run list --commit`).
Saves ~16 more jobs per release. Cost: a new release-check step and a new
way for the release to fail (CI not yet finished at tag time); the
rehearsal (`dry_run`) must exercise it.

### Do nothing
Releases are rare and the matrix is the evidence. Cost is minutes of free
runner time and a harder-to-read run; risk is nil.

## Recommendation
B. It cuts `ci / python` in half, and it moves the one extra interpreter
check onto the artifact users get. C saves more but changes what a release
depends on; take it only if run time becomes a problem. Would change my mind:
a PyO3/abi3 break that only a source build shows.

## Decision for the human
❓ **Q1 — Python versions**: A, B or keep both on all platforms?
➡️ B.

❓ **Q2 — Tag-time CI**: C (require a green `main` run) or keep re-running CI?
➡️ Keep re-running for now.

No ADR needed unless C is taken (it changes the release gate).

## Decision

