---
name: retire-plan
description: Close a completed plan: verify, review its commits against the plan, update the docs, delete the plan.
disable-model-invocation: true
argument-hint: <plan slug>
---

# /retire-plan — move the durable parts, delete the rest

Run by the human, or by `/work`'s conductor after a plan's last step.
Deletion is the "done" signal; anything worth keeping moves first.

## Do

1. **Verify.** Every step of `docs/plans/<slug>.md` ticked (never tick one
   here). Start the plan's *Acceptance* runs and `tools/gate.sh` in the
   background (`run_in_background`), plus `tools/py-test.sh` when the plan
   touched `crates/laman-py` or `laman`'s public API, and the wasm32
   digest comparison once it exists. Steps 2–4 run while they do. The
   retirement commit waits for them green; anything red: stop and report.
   A plan whose diff reaches no code (`crates/`, `tests/`, `Cargo.*`,
   `clippy.toml`, `tools/`, `.githooks/`) runs no gate and says so.
2. **Review the commits against the plan**, in a subagent on tier 3
   (`.agents/tiers.md`) so it reads with fresh eyes. Give it the plan
   file, `.agents/review.md` (its brief and standards),
   `.agents/rules/solver.md`, `GLOSSARY.md`, and the plan's commits: `git
   log --format='%h %s' --grep='plans/<slug>'` (it reads each with `git
   show`). Report its findings by axis. Fix a hard finding in its own
   commit (`fix(scope): … (plans/<slug> review)`). A judgement finding the
   agent decides on the charter's terms (`SEED.md`), stating the decision
   and its reason in the reply; only a product question waits for the
   human. One left for later is a backlog line.
3. Execute *Docs to update* line by line, present tense, no "as of this
   plan". A closed `⚠ OPEN:` gets its ADR now, listed in
   `docs/adr/README.md`.
4. **Drift check** every design doc the plan touched against the code;
   list the fixes in the reply.
5. `docs/ROADMAP.md`: update the status line; if the plan completes a
   milestone, say so.
6. `AGENTS.md` "Current state": one milestone-level sentence, under ~15
   lines.
7. `CHANGELOG.md` `## Unreleased`: the plan's bullets if a consumer would
   notice (`.agents/rules/git.md` §The changelog); every public type or
   signature the commit bodies name goes under `### Breaking` with its fix.
8. Deferred work: one line each in `docs/BACKLOG.md`.
9. `git rm docs/plans/<slug>.md`; commit all as `docs: retire plan <slug>`,
   the body listing the docs updated.
10. End with **one recommended next step** from the roadmap's open lines,
    the other plan and the backlog: the skill and its argument, and one
    sentence on why. Suggest `/retro <slug>` first if the plan's sessions
    had friction worth mining.

Durable facts move to the design docs and ADRs; the plan itself is
deleted, not summarised or kept for reference — git has it.

`$ARGUMENTS`
