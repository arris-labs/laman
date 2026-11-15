# ADR-0017: A branched core splits at its joins

- Status: accepted; amends 0012
- Date: 2026-11-15
- Scope: C4 branched profiles (`plans/m13-branched-profiles` step 1)

## Context

ADR-0012 D2 reports a branched core (a join of three or more edge ends) and
lets none of its edges into a profile. A plate with a wall across it, a
window with four panes, two loops touching at one join, a rectangle tied to
the rectangle inside it by one line: every one of these is a sketch a
sketcher draws, and every one of them gives no profile today. C4 splits
them. Joins stay declared (ADR-0012 D1): which edge ends meet is what the
caller wrote, and only the order of the edges around a join, which the
declarations do not say, is read from the coordinates.

The split has to say: how the edges at a join are ordered, including two
that leave it tangent; how loops are walked from that order; which loop of
a connected set is the boundary of the whole rather than a bounded area;
what an edge with the same area on both sides is; and how pieces nest.

**The reference trees.** PlaneGCS solves and has no profile extraction.
FreeCAD builds faces from a sketch in Part: `src/Mod/Part/App/WireJoiner.cpp`
and `FaceMakerBullseye.cpp` were read. WireJoiner orders nothing at a
vertex: it finds some closed wire through each edge by a depth-first search,
then repeatedly splits a wire wherever a branch's midpoint is classified
inside the face the wire bounds (a point-in-face test by the modelling
kernel), and in outline mode drops every edge two wires use. Its result
depends on the search order and on a classification per branch, and it
measures joins by distance. Nothing of it is taken here except the lesson:
splitting by containment tests is costly and order-dependent, where an
order at each join gives every bounded area in one pass. SolveSpace's tree
was not on hand for this ADR and was not read.

## Decision

**D1. Bridges.** After ADR-0012's pruning, a core edge whose removal
disconnects its core is a **bridge**: the same area lies on both its sides,
so it bounds nothing. Bridges are found by one depth-first search over join
classes with low links, iterative, started from the smallest class id and
following each class's edges in id order; parallel edges (two edges joining
the same two classes, a slot's arcs) are told apart by edge id, so neither
is a bridge. Bridges are reported in `Profiles::bridges`, ascending, and
removed. What remains falls apart into **pieces**: the connected parts of
the core without its bridges. A rectangle tied by a line to a rectangle
inside it is two pieces and one bridge, so a plate with a hole.

**D2. Order at a join.** Each edge end leaving a join has a direction and a
signed curvature, in the `crate::frame` frame of the edges meeting there:
a line leaves toward its other end, curvature 0; an arc leaves
perpendicular to its radius, turning with its traversal, curvature `+1/r`
leaving from its start (counter-clockwise) and `−1/r` leaving from its end.
Ends are sorted counter-clockwise by direction, compared exactly by
half-plane and cross product, no angle computed. Two directions **tie**
when `s · |sin Δθ| ≤ L` with `cos Δθ > 0`: `s` the shorter of the two edges'
lengths (a line's length, an arc's `r` times its sweep), `L` the smaller of
their length tolerances (`Tolerances::at`, as ADR-0012 D4; no new
constant): ends that, by their directions alone, would stay within `L`
of one another over the shorter edge. Ends chained by ties form a tangent
run (one run may wrap past −x); inside it they are sorted by curvature
ascending, which is counter-clockwise just past the join: of a line and an
arc leaving east, the arc turning left lies above the line. Curvatures tie
when `s² |Δκ| / 2 ≤ L`, chained the same way, and ends whose curvatures
tie are ordered by edge id, an edge's start before its end. Such ends are
an overlap at the join: deterministic, its nesting not specified.

**D3. The tangent test's distance** (closes the plan's `⚠ OPEN`). `s` is the
shorter edge's whole length, not a fraction. A declared tangent solves to
the coordinate tolerance (ADR-0015), so its directions differ by about that
over the radius, and `s` up to a full turn keeps `s · Δθ` far below `L`.
Two ends that tie but do not touch at the join part within `L` over the
shorter edge, which ADR-0016 D3 classifies as a touch at that place: the
order reads them as the touch the intersection measure reports. Step 2's
closed-form tests (line and line, line and arc tangent on either side, two
arcs of different and of equal radii, at scales 1e-6 to 1e6, translated,
and near 1e±300) are the measurement; a case they show this rule orders
wrongly is an addendum here, never a second constant.

**D4. Loops.** Every piece edge is used once in each direction. From a
directed edge arriving at a join, the walk leaves along the end next
clockwise from the arriving edge's own end there. Each directed edge lies
in exactly one loop, and each loop keeps its area on its left, so a bounded
area is walked counter-clockwise. A loop starts at its smallest edge id.

**D5. Outlines.** In each piece the loop with the most negative area is its
**outline**, ties broken by the smallest edge id; it keeps the area outside
the piece on its left and never becomes a profile. Every other loop of the
piece is one of its bounded areas. Since every edge is walked once each
way, a piece's loop areas sum to zero, so a piece with any area has an
outline of negative area. Where curves of a piece cross (closes the plan's
`⚠ OPEN`), the rule still picks exactly one outline by the same comparison,
deterministically, which may not be the outer boundary; every other loop
still gets a role, none is dropped, and the nesting is not specified, as
`profiles()` already says of crossing curves. A plain loop is a piece of
two loops: its outline, and the same edges counter-clockwise, its one
bounded area; ADR-0012 D3 holds for it unchanged.

**D6. Nesting.** All of a piece's bounded areas take one role, read from
the outline's probe (ADR-0012 D3, ADR-0013 D4): the innermost bounded area
of another piece (least absolute area, ties by smallest edge id) whose
winding number there is nonzero. Inside an outer boundary, every bounded
area of the piece is a hole of that profile, walked reversed so it is
clockwise; inside a hole, or inside nothing, every bounded area is an outer
boundary, a profile of its own. A circle is a piece whose one bounded area
is itself. So a branched core inside a plate gives the plate one hole per
bounded area (closes the plan's `⚠ OPEN`), the roles stay uniform per
piece, and an island inside one of those holes nests as any island does.

**D7. Types** (closes the plan's `⚠ OPEN`). `Branched` and
`Profiles::branched` are removed, and `Profiles::bridges` is added. A core
whose order ties exactly still gives loops, in id order, so nothing is left
that `branched` would report. A line or arc may appear in two loops, once
in each direction.

**ADR-0012 amended.** D2's sentence on a branched core is replaced by D1 to
D5 here; its consequence "edges shared by two loops are reported as
branched" no longer holds. D1 (declared joins), D3 (area, containment by
probe, roles by depth, now per piece) and D4 (unjoined ends) stand.

## Consequences

- Every bounded area of a core is a loop, and every core edge that is not a
  bridge enters exactly two loops, once each way; one of them may be an
  outline. Profiles of a branched sketch share edges.
- The coordinates order ends at a join and never make one: the same sketch
  gives the same loops at every frame of a drag unless a drag swings one
  edge past another at a join, which is a change of the drawing.
- `laman_check::check_profiles` (M14) re-derives the split with its own
  code; its "each edge listed once" becomes "at most once in each
  direction, a bridge in no loop, every other core edge in some list".
- Extraction stays near linear in edges for the walk; the sort at a join is
  `d log d` in its degree. ADR-0014's budget is re-measured in M14.

## Alternatives considered

- **Keep `branched`** for cores whose order ties exactly: a tie gives loops
  in id order anyway, and the docs already leave crossing curves
  unspecified, so the list would be empty in every sketch the order can
  read. Rejected.
- **The outline as one hole** of the plate a branched core lies in: one
  hole would cover areas that are themselves profiles' boundaries, and an
  island inside one of those areas would nest against the outline, not the
  area it is in. Rejected for D6's per-area roles.
- **Splitting by containment** (FreeCAD's WireJoiner, above): a point
  classification per branch, results that depend on search order. Rejected.
- **An angle tolerance** for the tangent tie: a new constant, and one that
  means a different distance at every edge length. Rejected for D3's `L`
  over the shorter edge.
- **A fixed fraction of the shorter edge** for `s`: no case was found where
  the whole length orders wrongly, and a fraction is a constant to choose.
  Left to step 2's measurement.
