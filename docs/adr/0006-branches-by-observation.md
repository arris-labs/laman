# ADR-0006: Branches by observation, compared beyond a margin

- Status: accepted
- Date: 2026-11-02
- Closes: SEED.md §10.8, how branch preservation is defined and tested

## Context

A constraint system has discrete solutions. Keeping the one the sketch sits
on, across a solve and every drag step, is a kickoff differentiator (SEED.md
§4). Some roots are excluded by the equations themselves: a tangent, a
circle–circle contact and a point–line distance carry a **sense** chosen
from the drawing, and their signed residuals do not vanish on the other
root (DATA-MODEL.md §Constraints). Others are not, and nothing in a solve
stops it from landing there:

- an arc's sweep: `|e − c| = |s − c|` holds whichever way `e` lies from
  `s`, so an arc can turn from the short way to the long way;
- `parallel` (the cross product vanishes for antiparallel lines too),
  `perpendicular` (+90° or −90°), `angle` (`sin(φ − θ)` vanishes at
  `θ + π`, where the segments meet at the supplement);
- a circle's radius, a free parameter whose sign no distance reads.

Horizontal and vertical lines also reverse, but their equations are
linear and a reversal passes the line through zero length, which the
degenerate diagnostics name. A point on an arc's circle outside its sweep
is the same root as inside it, not another one.

The obvious observation, the sign of `cross(s − c, e − c)` for an arc, is
wrong where arcs are most common: at a semicircle the cross product is
rounding (the slotted plate's slot ends sit at sweep π), and its sign
changes with the last bit of a coordinate. Any sign near its zero has the
same problem. No reference solver was read for this decision.

## Decision

**Observations.** At every set of coordinates a solve or a drag step
returns, laman records one quantity per subject, in subject order
(entities by id, then constraints by id):

| Subject | Quantity | Branch |
|---|---|---|
| arc | its sweep, tracked | `⌊sweep / 2π⌋`, 0 as drawn |
| circle | its radius `r` | sign |
| `parallel` | `dot(e1, e2)` of the unit directions | sign |
| `perpendicular` | `cross(e1, e2)` | sign |
| `angle` θ | `cos(φ − θ) = dot cos θ + cross sin θ` | sign |

Quantities are read on unit directions, so no product of two lengths can
overflow, and `θ`'s transcendentals go through `libm`.

**Tracking.** An arc's sweep is `atan2(cross, dot)` of its unit radii, read
in `[0, 2π)` when there is no earlier observation (the guesses, a drag's
first frame). After one, it is the representative of that reading nearest
the earlier tracked sweep: each solve is taken to move a sweep by less than
half a turn. An arc **flips** when its tracked sweep leaves `(0, 2π)`
through either end: `e` carried past `s` clockwise (sweep below 0) or
counter-clockwise (above 2π). A semicircle's sweep stays at π however its
cross product rounds.

**The margin.** A branch is **known** only beyond a margin, the size at
which the quantity no longer tells its roots apart:

- a circle's radius, and an arc's radius and chord `|e − s|`, against the
  entity's length tolerance at the observed coordinates
  (`Tolerances::at`, ADR-0003/0004): a branch is known exactly when its
  entity is not degenerate;
- a line pair's quantity beyond `tolerance::ANGLE` in magnitude (the lines
  more than that angle from where the two roots meet), with both lines at
  least their own length tolerance.

No new tolerance is introduced. A non-finite quantity is never known.

**Inside the margin, the last known branch is carried.** An observation
inside its margin keeps the branch, and the quantity it was read from, of
the observation before it; with none, it is unknown. So a crossing spread
over two solves (one frame inside the margin, the next beyond it on the
other side) is still a flip, and a branch first known after an unknown
start is the sketch's branch, not a flip.

**Flips.** Two observation sets are compared subject by subject; a subject
unknown on either side is not compared. A **flip** names the subject, the
quantity where the old branch was last known (`was`) and the quantity on
the new one (`now`). A static solve compares the solution with the
guesses; a drag step compares with the session's last solution.

**A step that still flips.** A drag step that flips is bisected toward the
last good target, at most `SolverOptions::max_bisections` halvings. If the
nearest halving still flips, the step returns the solution at the farthest
target that kept every branch (the last solution itself when every halving
flips), with the flips the nearest flipping halving would make named. The
session's last solution stays on its branch, so the next step starts there.
Keeping the branch is the guarantee (SEED.md §4); the named flip tells the
caller why the point stopped short, and taking the other branch is an
explicit act (finish the session, edit or solve statically).

**Tests.** Branch preservation is tested by construction, never by laman's
own output: `tests/fixtures/branch/` holds a sketch and moved coordinates
whose flips are known in closed form (an arc through its start,
antiparallel lines, an angle's supplement, a circle through radius zero)
and a semicircle moved by one ulp that must not flip; the moves are also
observed back (the mirror flip), at 1e300 (the same flips) and at 1e-300
(below the length floor, nothing known). Drag fixtures assert flips
empty along a constructed path, and bisection fixtures a flip kept or
named.

## Consequences

- Diagnostics gain flips; DoF, redundancy and conflicts are unchanged.
- Tracking assumes less than half a turn of sweep per solve. A static solve
  whose guesses are more than that from the solution reads the sweep at the
  nearest representative; a drag keeps its steps under it by bisection.
- A sketch below the length floor (ADR-0003) has no known branch, so
  nothing in it can flip; that is the floor's meaning, not a hole.
- A caller who wants the other branch does not get it from a drag session:
  the session refuses to leave its branch, and says so.
- A new constraint kind with an unheld root adds a row here and an
  observation; the exhaustive match in `branch` fails to compile until it
  is classified.

## Alternatives considered

- **Sign of `cross(s − c, e − c)` for arcs.** Rounding at sweep π, the
  most common arc in a profile; rejected.
- **A fixed dimensionless margin for every quantity.** A radius and a
  chord are lengths and scale with the sketch; the degeneracy tolerance
  already says when a length is resolvable. Rejected for a second
  threshold that could disagree with the degenerate diagnostics.
- **Adopt the new sign inside the margin** (no carry). A drag through the
  boundary in two frames would never be a flip. Rejected.
- **Return the flipped solution after the last halving.** Follows the
  cursor, but leaves the session on the other branch, and a drag back is
  then refused as a flip of its own. Rejected for the guarantee.
- **Encode branches as signed equations** (as senses do). Would forbid a
  deliberate flip in a static solve and turn the arc's intrinsic equation
  into an inequality; rejected for observation.
