---
name: next
description: Where things stand (cycle, plans, findings, uncommitted and unpushed work, CI, release due) and the one command to run next.
disable-model-invocation: true
---

# /next — where we are, and what to run

Read-only: it changes nothing, it orients the human. Gather in parallel:

- `AGENTS.md` "Current state" and the open cycle's section of
  `docs/ROADMAP.md` (its in-list against what landed);
- `docs/plans/*.md` but `TEMPLATE.md`: each plan's unchecked steps with
  their grades;
- `docs/ideas/*.md`: each idea's `Status` and any unanswered round;
- `docs/BACKLOG.md` §Findings: how many, and any naming the open cycle;
- `CHANGELOG.md` `## Unreleased`: whether it has bullets, and a
  `### Breaking`;
- **Release due** (`.agents/rules/git.md` §Tags and publishing and §The
  version): a release is due when the open cycle is closed (its
  `docs/ROADMAP.md` section is compressed to its status line) or
  `## Unreleased` has bullets a consumer would see. The version is the
  one §The version gives (minor on a cycle close or a `### Breaking`
  bullet, patch otherwise). `Due` names it as `release X.Y.Z`: the tag is
  the human's, pushed after `/release` prepares the commit;
- `git status --short`, `git log --oneline origin/main..main | wc -l`
  (unpushed; no remote yet is said as such), `git describe --tags --match
  'v*' --abbrev=0` and the commits since it;
- `gh run list --branch main --limit 3` (skip it quietly if `gh` fails).

Reply in one screen, in this shape:

```
Cycle    C1, the vertical slice: 2 of 7 in-list lines landed
Plans    harness: step 4 next ([2] the digest), 6 left
Ideas    arc-parametrisation: round 2 unanswered
Findings 1 open, on this cycle (fuzz: NaN radius)
Tree     clean | 4 files staged by another session
Git      9 unpushed; last release v0.0.1, 41 commits since; CI green
Due      push; release 0.1.0 (cycle closed; /release)

→ /work harness step 4-5   both [2]; Sonnet fits
  or: /idea arc-parametrisation   (one line on why it waits or wins)
```

The `→` line is one command with its arguments and a reason, and the
model its grade suggests ([1]/[2] Sonnet, [3] Opus). A step that waits on
an `⚠ OPEN:` is named as blocked, with what unblocks it. Staged or
modified files you did not make are another session's: say so, and
recommend nothing that would commit them.
