# ADR-0018: The checker re-measures intersections and re-derives the split

- Status: accepted
- Date: 2026-11-16
- Scope: C4's checks, `laman_check::check_intersections` and
  `laman_check::check_profiles` (`plans/m14-check-digest-python` step 1)

## Context

M12 and M13 gave extraction two new outputs that nothing independent
checks. `laman::intersect::intersections` reports where curves meet
(ADR-0016); `laman::profile::profiles` now splits a branched core into
bridges, pieces and loops (ADR-0017). `check_profiles` holds only that each
edge is listed at most once each way, not which list it belongs in, and
tests a hole against its parent alone (ADR-0013 D4). The checker is a check
only if it shares nothing with what it checks (SEED.md §7): it reads
`laman::model` and the ADRs, never `laman::intersect` or
`laman::profile`.

Three things have no single right answer and need a rule.

- **The kernel's tests are at `L`.** ADR-0016 classifies and places by
  "within `L`": a root within `L` of a line's end along its carrier, a
  touch where the carriers part by `L` or less, a meeting within `L` of an
  end taken at that end. A second measure, by other formulas and other
  rounding, cannot agree with every verdict at exactly `L`. ADR-0013 D2 met
  the same problem for an area's sign and left a band undecided.
- **A meeting is moved by up to `2L`.** ADR-0016 D4 puts a meeting within
  `L` of an end at that end, and the other curve's parameter is then read
  at the moved point, clamped to the curve. So the reported point may lie
  `L` from the other curve, and the other curve's parameter may map back to
  a point `2L` from it (the root `L` past the second's end, the first's end
  `L` before the root).
- **A closest distance is not a meeting.** The kernel extends a curve by
  `L` along itself at a free end and looks for roots there. Where a line's
  end stops `1.1L` short of a circle along its carrier but crosses it at a
  shallow angle `α`, the end lies `1.1L·sin α` from the circle, far inside
  `L`, and the kernel rightly reports nothing. "Every unreported pair is
  more than `L` apart" would fail a correct result.

## Decision

**D1. The undecided band.** Where ADR-0016 decides at `L`, the checker
decides at `L/4` and `4L`: a quantity the kernel holds within `L` is a
violation only past `4L`, a quantity the kernel holds beyond `L` is a
violation only at `L/4` or below, and between them the verdict is
undecided, never a violation. The factor is `BAND = 4`, a named constant
in `laman-check`: twice the `2L` by which ADR-0016 D4's move to an end can shift a point
or a parameter, so the second-order terms of curvature (`L²/r`, with
`r > L` by ADR-0016 D1) and rounding (`ε` of the frame's extent, about
`1e-5·L` at `LENGTH_RELATIVE`) never reach it. It is not a tolerance of its
own: it scales with `L`, and a broken twin moves a meeting by a fraction of
its curve, many times `4L`. `L` is the smaller of the pair's two
`Tolerances::at` length tolerances (ADR-0004, ADR-0016 D2); no new
tolerance.

**D2. Each reported meeting**, measured in the checker's own frame of the
pair (D6):

- **`OffCurve`**: its point (and an overlap's other end) lies farther than
  `4L` from either curve, by the checker's distances: to a line between its ends,
  to a round's circle where the point's angle is in the sweep and to the
  nearer arc end where it is not. A point within `4L` of an arc's circle but
  past its sweep, farther than `4L` from both its ends, is
  **`OutsideSweep`** instead.
- **`Parameter`**: a parameter outside its range (a line's and an arc's in
  [0, 1], a circle's in [0, 1), an overlap's far end on a circle up to 2),
  or one that maps back farther than `4L` from the point it belongs to: a
  line's `a + t(b − a)`; an arc's start turned by `t` times its sweep about
  the centre at the start's radius (ADR-0001); a circle's point at angle
  `2πt`. NaN is outside every range.
- **`Kind`**: the kind the checker reads (D3) is decided and differs.
- **`AtJoin`**: a crossing or a touch within `L/4` of a point of a join
  class both curves have an end in, by the checker's own walk over
  `coincident` (ADR-0016 D5); an overlap never is.

**D3. How a kind is read.** At the reported point the checker takes each
curve's tangent direction there (a line's own; a round's perpendicular to
the radius through its nearest point) and reads a **depth**: how far the
curves pass to each other's far side near the point, from the angle `Δθ`
between the tangents and the curves' own sizes, not from the carriers'
distance the kernel tests.

- Two lines: `|sin Δθ|` times the larger distance from the point to the
  shorter line's ends (ties: the second's), how far its ends leave the
  other's carrier.
- A line and a round of radius `r`: `r·(1 − |cos Δθ|)`.
- Two rounds with centres `d` apart, `φ` the angle between their radii at
  the point: the smaller of `2r₁r₂(1 − cos φ) / (d + |r₁ − r₂|)` and
  `2r₁r₂(1 + cos φ) / (r₁ + r₂ + d)`, the inner and outer touch.

At a root these equal the carriers' parting the kernel measures
(`r − d` for a line and a circle), and at a touch's reported point the
tangents are parallel and the depth is about 0. A depth past `4L` reads a
crossing, one at `L/4` or below a touch or an overlap, and between them
the kind is undecided. Two lines whose depth reads a touch, and two rounds
with centres and radii within `L/4`, are **parallel**: they read as an
overlap where the stretch they share (the shorter's projection clipped to
the longer, or the shared sweep times the radius) is longer than `4L`, as
a touch where it is `L/4` or shorter. Rounds with centres between `L/4`
and `4L` apart are undecided. Two lines are never read a touch except
parallel: ADR-0016 D3 has no other line–line touch.

**D4. Completeness: a missed pair.** For every pair of non-construction
curves (ADR-0016 D1, a curve no longer than `4L`, or a round within `4L`
of radius 0, skipped), the checker finds **places** where they decidedly
meet, by its own minimisation of their distance, and each must have a
reported meeting of the pair within `4L` (within `4L` of an overlap's
stretch). One that has none is **`Missed { first, second }`**: the pair is
missing from the result, or a meeting of a reported pair is. The places:

- **Roots.** The signed distance of points of the first curve to the
  second's carrier (to a line, across it; to a circle, `|p − c| − r`) is
  sampled at the first curve's ends and at its distance's extremes (a
  line's foot of the other's centre, a round's points on the line of
  centres); each sign change brackets one root, placed by bisection
  capped at 64 halvings (the frame's 53 bits, with room), never by the
  closed form the kernel solves. A root is a place when it lies on both
  curves (an arc's angle in its sweep), farther than `4L` from every end
  of both, and its depth (D3) reads a crossing.
- **Extremes.** The common normals of the pair: a line's foot of a round's
  centre with the round's two points on that normal, two rounds' points on
  the line of centres. One is a place when its distance is `L/4` or less
  and both its points are farther than `4L` from every end: a touch.
- **Ends.** An end `E` of one curve whose nearest point `Q` on the other
  lies farther than `4L` from the other's ends, with `|EQ| ≤ (L/4)·|sin
  Δθ|` (`Δθ` between the tangents at `E` and `Q`) and a depth at `Q` that
  reads a crossing: the end's own curve, continued by `L` as the kernel
  continues it, reaches the other within `L/4`. Not a place when `E`'s join
  class holds an end of the other curve (D5 of ADR-0016 leaves it out).
- **Parallel pairs** (D3) are places where they share a stretch longer
  than `4L`, which an overlap must cover end to end.

Everything closer to an end than `4L`, every touch place between `L/4` and
`4L`, and every root whose depth is undecided is undecided: an end within
`L` of another curve at a shallow angle, the Context's case, is never a
missed pair. A **missed pair** is thus decided only where the kernel's
rules leave it no choice.

**D5. Per-pair and form tests.** **`NotACurve`**: a pair names an entity
that is not a non-construction line, arc or circle of the sketch, names
one curve twice, or names a line no longer than `L/4` or a round of radius
`L/4` or less (ADR-0016 D1: it meets nothing). **`Order`**: the list's form breaks ADR-0016 D1 or D7:
`first < second`, pairs ascending by `(first, second)`, meetings ascending
by their parameter on the first then the second (`f64::total_cmp`), an
overlap's `at` the end of lower parameter on the first, no empty pair.
**`Repeated`**: a pair listed twice. A pair one of whose curves lacks a
coordinate, or holds a non-finite one, is left unchecked and its parameter
listed in the report's `missing`, as ADR-0013 D3 does; nothing panics on
any `Intersections` value.

**D6. The frame.** Each pair is measured as ADR-0016 D8 and ADR-0013 D1
describe, by the checker's own code: scaled by an exact power of two so the
largest coordinate is below 1, translated to the first curve's origin,
scaled again so the pair's extent is in [½, 1), `L` with it; distances and
depths come back through the same powers of two. Angles by `libm::atan2`.
Nothing overflows near 1e±300, and a small pair far from the origin keeps
its digits. Square roots of differences are taken as products.

**D7. The split re-derived.** `check_profiles` reads no class from
`Profiles` to decide another:

- **Its own pruning.** Self-joined edges (both ends one class, by its own
  walk over `coincident`) are left out first, as ADR-0012 does. Classes of
  degree 1 are pruned to a fixed point; an edge pruned from a connected set
  that keeps no core is open, one pruned from one that does is dangling.
  The fixed point is the 2-core, the same in any order, so the derivation
  cannot differ from a correct extraction's.
- **Its own bridges.** A core edge is a bridge when its two classes are
  not connected by a union-find over the core's other edges: a route other
  than extraction's depth-first low links. Quadratic in a core's edges;
  the checker has no budget (ADR-0014 is extraction's).
- Each edge must be listed in its derived class: open chains (each listed
  chain one derived chain), dangling, bridges, self-joined, or a loop for a
  core edge that is no bridge. An edge listed elsewhere is
  **`Misclassified { entity }`**; the existing `Repeated`, `Uncovered` and
  `NotAnEdge` stand.
- **One bounded area per loop.** A piece is a connected part of the derived
  core less its derived bridges. For each listed loop, the probe (ADR-0013
  D4) of every other edge of its piece is tested against the loop by
  ADR-0013 D4's signed ray: inside, and farther than `L` from the loop,
  names **`NotOneArea { first }`**, `first` the loop's first edge. Within
  `L` it is undecided. An edge that comes within `L` of the loop anywhere
  but at a join they share, by D4's distances, leaves the test undecided:
  that piece's curves cross, and ADR-0017 D5 does not specify its loops.
  The distances run only on an edge whose probe tested inside.
- **Sibling holes.** A hole's probe is tested against each other hole of
  its profile from another piece, as against its parent (ADR-0013 D4):
  inside one, beyond `L`, is `Containment`. Holes of one piece share edges,
  and extraction gives none of them inside another.

`Misclassified` and `NotOneArea` are new `ProfileViolation` variants: a
public change.

## Consequences

- Every C4 fixture checks clean, and a broken twin (a meeting moved,
  dropped or its kind or parameter changed, a pair dropped, two bounded
  areas merged, a bridge in a loop, a hole inside a sibling) is named,
  since a twin moves things by far more than `4L`.
- A result wrong only inside the band passes: the check names what is
  wrong by more than the kernel's own `L` could explain, and no more.
- A dropped meeting at a curve's end (a T whose stem is dropped from the
  result) is found only when the end crosses at an angle (D4's end rule); a
  shallow one, or one at a join, is undecided.
- `check_intersections` is quadratic in curves, as `intersections()` is
  before any pruning; `check_profiles`'s bridge test is quadratic in a
  core's edges. Neither is budgeted.
- A piece whose curves cross gets no `NotOneArea`: the check says nothing
  of loops ADR-0017 leaves unspecified.

## Alternatives considered

- **The kernel's `L` as the checker's threshold**: a correct result fails
  wherever the two measures' rounding falls on opposite sides of `L`, and
  every meeting moved to an end fails outright. Rejected.
- **A band of `2L`**, the bound of the move to an end: reached exactly
  by a meeting moved at both ends, so the rounding of the two frames decides it.
  Rejected for twice that.
- **Completeness by closest distance alone**, the plan's first wording:
  fails a correct result at every shallow end, the Context's case.
  Rejected for D4's places.
- **Completeness by pair**, a pair reported at all counting as complete: a
  result dropping one of a line's two crossings with a circle passes.
  Rejected for places.
- **Roots by the closed form**: the kernel's own route, so a mistake in its
  algebra would be repeated. Rejected for bisection between the extremes.
- **The kind by tangent directions alone** (ADR-0017 D3's `s·|sin Δθ| ≤ L`):
  a correct crossing at depth `2L` has `Δθ ≈ 2√(L/r)`, read as a touch by
  no length, and a correct touch read at its foot has `Δθ = 0` but a depth
  up to `L`: the angle has to be turned into a distance by the curvature.
  Rejected for the depth.
- **`NotOneArea` on crossing pieces too**: a correct extraction of a drawing
  ADR-0017 D5 leaves unspecified would fail. Rejected.

No reference tree was read for this ADR.
