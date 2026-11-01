---
name: retro
description: Read a plan's (or this) session's transcripts and propose improvements to the agent's environment, most severe first.
disable-model-invocation: true
argument-hint: <plan slug, session id, or nothing for this session>
---

# /retro — improve the environment, not the code

The question is what in this repository's environment (steering files,
checks, tools, docs) made the work slower, wronger or more dependent on the
human than it had to be, and the smallest change that fixes it for every
later session.

1. **Read the sessions.** Nothing given: this session, from your own
   context. A plan slug or session id: the transcripts under
   `~/.claude*/projects/<this checkout's path, slashes as dashes>/*.jsonl`
   that mention it (a digest script is a backlog line). Read the plan's
   commits too (`git log --grep='plans/<slug>'`), and count the steps
   that needed a later commit naming them: a tier often reworked is a
   grade candidate.
2. **Find the candidates**, each with its evidence (session id and a quote
   or command):
   - **The human stepping in**: a correction, a repeated instruction, a
     question about where things stand, an answer the agent could have
     looked up. Each is a missing rule, pointer or skill step.
   - **Navigation**: a fact that took many reads to find, or a doc the
     agent had to be pointed at.
   - **Checks**: a mistake a lint, test or hook could have caught. A
     mechanical rule gets a check, not a sentence.
   - **Steering files**: in `AGENTS.md`, `.agents/rules/` and the skills, a
     sentence that changes nothing the agent would do anyway, restates what
     a command or config already says, or has gone stale. Always-loaded
     lines cost every turn.
   - **Tool economy**: a long run repeated, a wait polled by hand, an
     oversized output read whole.
   - **Information access**: a log, value or machine fact the agent could
     not reach.
3. **Reply with the candidates, most severe first**: what happened (the
   evidence), the change (file and wording, or the check), and its cost.
   Change nothing yet.
4. Apply the ones the human accepts, one commit per change as `docs: …`
   (or `build`/`ci` for a check), the body citing the session it came from.

`$ARGUMENTS`
