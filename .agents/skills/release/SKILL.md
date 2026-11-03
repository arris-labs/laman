---
name: release
description: Walk a release: drop -dev and cut the changelog heading, reopen main at the next -dev, the human tags and pushes once, then watch release.yml and its two reviewed publishes.
disable-model-invocation: true
argument-hint: <version, e.g. 0.1.0 — the workspace version without -dev>
---

# /release — one tag, two reviewed publishes

Publishing is `.github/workflows/release.yml`, run from a `vX.Y.Z` tag the
human pushes (`.agents/rules/git.md` §Tags and publishing). Its publish
jobs wait in two GitHub environments, `crates-io` (`publish-crate`) and
`pypi` (`publish-pypi`), each with a required reviewer. This skill prepares
the commits, then waits for the human at the tag and at each reviewer. It
never tags, pushes, approves a publish, or publishes by hand.

## Do

1. **Check the ground.** On `main`, with a clean tree and `main` equal to
   its upstream (`git status`, `git rev-list --count @{u}..main`; no
   remote is said as such and stops here). A release that changed a
   workflow also dispatches `release.yml` on `main` (`gh workflow run
   release.yml --ref main`, watched with `run_in_background`): it builds
   and checks everything and publishes nothing, since `release-check`
   refuses without a `v*` tag, and it must be green before step 5. Check
   that the `crates-io` and `pypi` environments have a required reviewer
   (`gh api repos/{owner}/{repo}/environments`) and that the commit to tag
   is on `main`; without either, stop and say so. Start `tools/gate.sh`
   with `run_in_background` and go on with step 2.
   **Packaging proof, before step 5** (a local rehearsal; the tag's own CI
   run is the gate, ADR-0008). On the release commit from step 3, in a
   scratch copy outside the tree, nothing uploaded: `cargo package -p laman
   --locked`; in `crates/laman-py`, `maturin build --release` and `maturin
   sdist`; then `uv tool run twine check --strict` on each file they wrote.
   All must pass; a failure stops the release before the tag.
2. **Pick the version.** The argument, else `[workspace.package].version`
   minus `-dev`. It must match the bump `.agents/rules/git.md` §The version
   gives: a closed cycle bumps the minor; a release between cycles bumps
   the patch, or the minor when `CHANGELOG.md` `## Unreleased` has a
   `### Breaking` bullet. Any mismatch: stop and ask.
3. **Prepare the release commit** (the gate runs in the commit hook; a
   commit that runs past a minute goes with `run_in_background`):
   - `Cargo.toml`: `[workspace.package].version` and both internal
     `[workspace.dependencies]` pins (`=…-dev`) become `X.Y.Z`. Then
     `cargo check --workspace` and confirm `Cargo.lock` changed only in the
     workspace crates' versions.
   - `CHANGELOG.md`: `## Unreleased` keeps its bullets under a new
     `## X.Y.Z — <YYYY-MM-DD>` heading, with an empty `## Unreleased` above
     it.
   - `AGENTS.md` "Current state": the release it says is due becomes
     the release made (`**X.Y.Z published** (<date>)`).
   - `tools/release-check.sh vX.Y.Z` and `tools/release-check-test.sh`
     pass.
   - Commit `chore(laman,py): release X.Y.Z`, the body saying the release
     is the one `release.yml` publishes from the tag. Stage by name, never
     `git add -A`.
4. **Reopen `main`** before the tag goes out, so one push carries both
   commits. The next
   `-dev` follows `.agents/rules/git.md` §The version: the next minor
   (`X.(Y+1).0-dev`) when this release closed a cycle, else the patch
   (`X.Y.(Z+1)-dev`), which a later `### Breaking` bullet moves to a minor
   in the commit that adds the bullet. Update `Cargo.toml` and both pins as
   in step 3, then `cargo check --workspace`. Commit
   `chore(laman,py): reopen main at <version>-dev`. Tagging it by mistake
   is safe: `release-check.sh` refuses a `-dev` version.
5. **The human tags and pushes.** Stop and say exactly, the tag on the
   release commit, not on `main`:
   `git tag vX.Y.Z <release-commit> && git push origin main vX.Y.Z`.
   Wait for the human's word that it is pushed. Never push or tag.
6. **Watch the run.** Find it (`gh run list --workflow release.yml --limit
   3`) and start `gh run watch <id>` with `run_in_background`; the
   completion notification is the wait, and it comes only when the run
   ends, never at a review. So say up front that `crates-io`, then `pypi`,
   will each ask for the human's approval once CI and the wheels are green,
   and that the run waits there. A failed job stops here: report
   it with its log link; a publish is never rerun without the human's word.
   If `gh` fails (an invalid token, for one), stop and say so.
7. **Reply.** The release commit, the reopening commit, the tag the
   human pushed, each job's result, and anything surprising. Name the
   reviewers the human still owes if any.

`$ARGUMENTS`
