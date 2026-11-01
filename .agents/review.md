# Review: a plan's diff, read against the plan and the standards

Read by the reviewer subagent `/retire-plan` starts, never by the agent
implementing a step: the reviewer gets the commits and has no exploration
to do, so the judgement calls live here instead of in every session's
context. What the compiler, clippy, the hook and the tests already enforce
is left to them.

## The brief

Two axes, reported apart so one cannot mask the other.

**Plan.** Quote the plan line for each finding:

1. a *Goal*, *Design deltas* or *Acceptance* item the commits leave missing
   or partial;
2. behaviour the commits add that the plan did not ask for, or that its
   *Non-goals* fence out;
3. an item that looks done but looks wrong;
4. a box ticked whose commit carries no fixture, property or
   finite-difference check for it;
5. a *Docs to update* line or a design doc the commits make untrue, and a
   public type or signature change missing from a commit body or from
   `CHANGELOG.md` §Unreleased.

**Standards.** Cite the rule (`.agents/rules/solver.md`, this file, an
ADR) for each finding, and quote the hunk. Mark each **hard** (a stated
rule broken) or **judgement** (a smell worth a look).

Under 500 words in all; nothing found on an axis is reported as such.

## The standards

`.agents/rules/solver.md` is the hard list: no panics, statuses not
errors, the checker independent of the solver, determinism, named
tolerances, capped iterations, Jacobians checked by finite differences,
every failure a fixture, the binding updated with a break, references read
and never copied. Beside it:

### Numerics
- A tolerance widened, an iteration cap raised, a fallback path or a
  special case added to make a fixture pass is a hard finding unless the
  plan's open questions decided it.
- A numeric constant is named, commented and says why it is not a
  tolerance.
- An iteration states its stopping rule and what it returns when it does
  not converge.
- A diagnostic names the caller's ids and tags, so a consumer (a human or
  an agent) can act on it.
- A change that can move a solution's bits says so, and the digest
  fixtures moved with it in the same commit, with the reason.

### Tests
- A fixture's expected values come from a closed form or the sketch's
  construction, never from laman's own output.
- A changed expectation says in its commit why the behaviour changed.
- A test asserts through the public API and the checker, not a private
  helper, wherever the API reaches the case.

### API and docs
- Rustdoc says what an item guarantees and refuses, not how it works; a
  public operation has a runnable example.
- A new public item that a consumer would use is bound in `laman-py` or
  named in the plan as deliberately unbound.
- Docs change in the commit that changes the behaviour, in present tense,
  without the plan's history; a `CHANGELOG.md` bullet speaks to a reader of
  `README.md` only.
- A term `GLOSSARY.md` defines is used as written; one it lists under
  _Avoid_ is a judgement finding. A new concept named in public API or a
  design doc has its glossary entry.

### Code smells (always judgement)
A name that does not say what it holds; the same logic twice in the diff;
a function reaching into another module's data more than its own;
parameters that always travel together; an `f64` or `usize` standing for a
concept that deserves a type; abstraction or hooks for a case no fixture
has.
