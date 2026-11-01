---
name: close-cycle
description: Close a finished cycle: verify, reconcile findings, drift-review the docs and glossary, compress its roadmap section, open the next.
disable-model-invocation: true
argument-hint: <cycle, e.g. C1 — and optionally the next cycle's theme>
---

# /close-cycle — one roadmap, one section per cycle

`docs/ROADMAP.md` is a living design doc, not a log: a finished cycle
shrinks to its status line and the next is appended below it. Never a
second roadmap file — git holds the history.

## Do

1. **Check it is finished.** Every line of the section done or moved to
   `docs/BACKLOG.md`; `docs/plans/` holds only `TEMPLATE.md`;
   `tools/gate.sh` and `tools/py-test.sh` green — run them now, in the
   background; the cycle's acceptance fixtures green, the digest equal on
   native and wasm32; and the last finished CI run on `main` green in every
   job (`gh run list --branch main`). Anything open: stop and report it;
   never close around it.
2. **Reconcile the findings** (`docs/BACKLOG.md` §Findings): every block's
   `State` current, `Reproduce` still replays, fixed blocks deleted with
   their fixture lifted.
3. **Drift review** — the only place it happens (`docs/README.md`): read
   `ARCHITECTURE.md`, `DATA-MODEL.md`, `ROADMAP.md` and `GLOSSARY.md`
   against the code and fix every sentence no longer true; a term the docs
   now use and the glossary lacks gets its entry. List the fixes, or say
   why nothing drifted.
4. **Public docs**: `README.md` and `docs/COMPARISON.md` read against the
   code and `CHANGELOG.md`: every capability claimed exists, every
   limitation still holds, every link resolves, each comparison cell is
   true at the version being cut with its source. Neither names a cycle or
   a plan.
5. **Compress the finished section** to: goal, one `**Status: done
   <date>.**` line naming the risk retired and the ADRs taken, then the in
   / out / accept lists. Before deleting a sentence, grep `docs/`,
   `crates/` and `README.md` for its fact: one that lives only here moves
   to the doc or code comment that wants it.
6. **Open the next section**: `## Cn — <name>` with goal, in, out, accept,
   from the roadmap's outline, the backlog and the first consumer's
   requirements. Theme and in/out split are the **human's call**: propose;
   if the outline and backlog don't make it obvious, ask.
7. **`AGENTS.md` "Current state"**: one sentence for the closed cycle, a
   new `**Next:**`, still under ~15 lines.
8. Commit `docs: close <cycle>`, the body listing the docs updated and the
   drift fixed.
9. **The release**: a closed cycle bumps the minor (`.agents/rules/git.md`
   §The version). Tagging, pushing and publishing stay the human's; say
   what is due.

`$ARGUMENTS`
