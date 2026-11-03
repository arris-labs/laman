# ADR-0008: The release gate is the tag's own CI run

- Status: accepted
- Date: 2026-11-03
- Closes: nothing from the kickoff; the CI-cost backlog item (docs/BACKLOG.md)

## Context

One commit was tested two or three times over. A push to `main` ran
`ci.yml` (28 jobs). A release ran `ci.yml` again inside `release.yml`
before it published, and `release.yml` built the wheels a second time for
its own dry run. A release push of `main` and its tag therefore ran about
60 jobs, and a `workflow_dispatch` rehearsal of `release.yml` ran 32 more
on a commit no one would tag. The evidence the jobs produce is the same
each time: the Rust digests on five runners plus wasm32 (SEED.md §5), and
the shipped wheel's corpus and digests on all five platforms.

The rehearsal also made the release workflow hard to check. Its
`dry_run` input overlaid stable metadata onto the tree, and
`tools/release-dry-run.py` and its test existed only to serve that
overlay.

## Decision

- **The gate is the tag's own CI run.** `ci.yml` also triggers on `v*`
  tags. `release.yml` no longer calls `ci.yml`. Its `publish-crate` job's
  first step refuses unless `gh run list --workflow ci.yml --commit
  "$GITHUB_SHA"` shows a successful run for the tagged commit. The
  reviewer approves once CI is green; an approval given early fails safe,
  and the job is re-run once CI is green.
- **`ci.yml`** runs on `push` to `main`, on `v*` tags and on
  `pull_request`. The `python` job is one job, Ubuntu with Python 3.10,
  which builds from source and runs `tools/py-test.sh` with stubtest.
- **Python on five platforms runs in `wheels.yml` only**, on every pull
  request that touches the binding's inputs (`crates/**`,
  `tests/fixtures/**`, `tools/py-*`, `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, the
  workflow itself) and
  on every release. It no longer runs on `push`.
- **`release.yml`** does not gate its build jobs on `release-check`. A
  `workflow_dispatch` on a branch therefore builds and checks everything,
  and `release-check` refuses to publish anything unless the ref is a
  `v*` tag. Publishing needs `release-check`, `crate-dry-run` and `wheels`.
- **The remote rehearsal is gone.** The `dry_run` input, its metadata
  overlay and `tools/release-dry-run.py` and its test are deleted. The
  packaging proof before a release is local (`/release` step 1): `cargo
  package --locked`, the maturin wheel and sdist builds, and `twine check`.

## Consequences

- A push to `main` runs 7 jobs, a release push about 31, down from 28 and
  60. Each commit is tested once by each job that tests it.
- The evidence is unchanged: five Rust runners, wasm32, and the shipped
  wheel on five platforms at every release and every pull request that can
  change it.
- A release is gated by a CI run on its own commit, not by a second CI run
  inside the release workflow. `publish-crate` depends on that run's
  conclusion, so a tag whose CI failed or never ran cannot publish.
- A `release.yml` dispatch no longer proves the publish path end to end on
  a branch. The packaging proof is local, and the first real tag exercises
  `publish-crate`'s green-CI check for the first time (not yet observed).

## Alternatives considered

- **Keep the re-run inside `release.yml`** (60 jobs on a release): tests
  the same commit twice for the same evidence; rejected.
- **Reuse prior results when only the version changed**, as arris's
  `tools/ci-reuse.sh` does (ADR-0069 there): laman's CI takes about 12
  minutes, so the complexity is not worth it; rejected.
- **Fewer Rust runners**: the five are the determinism evidence (SEED.md
  §5); rejected.
- **Keep the `dry_run` rehearsal on `workflow_dispatch`**: it runs the
  32-job build on a commit that is never tagged, and needs its own overlay
  to fake stable metadata; rejected in favour of the local proof.
