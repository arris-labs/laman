# ADR-0016: Intersections are measured, classified, never joined

- Status: accepted
- Date: 2026-11-13
- Scope: C4 (`plans/m12-intersections` step 3)

## Context

C4 reports where curves meet without a shared point: the C5 edits (split,
trim, extend) start from such a place, and a sketcher shows it. ADR-0012
made a join declared, never measured, so a place where two
curves merely meet in the coordinates is not a join, and nothing found
here may become one. It has to be measured instead: which pairs are
looked at, what kinds of meeting there are and how they are told apart,
where on each curve a meeting lies, what an end lying on another curve
is, and how the numbers survive scale and translation.

A touch is a double root: two curves at distance `δ` from tangency meet
at two points `√(2rδ)` apart, or not at all. A classification that waits
for the exact root never makes one; one that uses a tolerance has to name
it. ADR-0015 brought a declared contact to the coordinate tolerance, so a
touch the caller asked for (`tangent`) is within the length tolerance of
the curves it joins.

## Decision

**D1. Pairs.** The curves are the non-construction lines, arcs and
circles; an arc or a circle is a *round*. Each unordered pair is measured once, named smaller id first; a
pair with no meeting is not reported. A line no longer than its length
tolerance, or a round whose radius is within it, has no direction to
classify by and meets nothing.

**D2. The tolerance.** A pair is classified at `L`, the smaller of its two
curves' length tolerances from `Tolerances::at(sketch, values)` (ADR-0003,
ADR-0004), as ADR-0012 D4 does for unjoined ends. No new constant. Below
ADR-0003's floor, the floor decides, as it does for the solver and the
checker: geometry smaller than `LENGTH_FLOOR` is not resolved, so every
meeting in it is a touch or an overlap. The kernel takes `L` as an input,
so its closed-form tests at 1e-300 pass a tolerance at that scale;
`intersections()` at 1e-300 promises finite, typed output, never the
closed form.

**D3. Kinds.** With `d` the distance from a centre to a line's carrier or
between two centres:

- An **overlap**: two lines, the shorter one's ends (ties: the second's)
  within `L` of the longer one's carrier, whose projections share a stretch
  longer than `L`; or two rounds with `d ≤ L` and `|r₁ − r₂| ≤ L` whose
  sweeps share such a stretch. Reported by its two ends, each the end of
  one of the curves. Two collinear lines, or two such rounds, whose shared
  stretch is `L` or shorter meet in a **touch** at that place, and do not
  meet when their ends are farther apart than `L`; concentric rounds of different radii
  (`d ≤ L`, `|r₁ − r₂| > L`) do not meet.
- A **touch**: a line and a round with `|d − r| ≤ L`, reported at the foot
  of the perpendicular from the centre; two rounds with `d > L` and
  `|d − (r₁ + r₂)| ≤ L` or `|d − |r₁ − r₂|| ≤ L`, reported at the point of
  the first round on the line of centres. One point, though the curves may
  cross twice within `L`.
- A **crossing**: every other meeting, a simple root of the pair: two
  lines that are not collinear, a line with `d < r − L`, two rounds with
  `|r₁ − r₂| + L < d < r₁ + r₂ − L`. A line and a round meet twice, two
  rounds twice, two lines once.

**D4. An end on another curve** (closes the plan's `⚠ OPEN`). A meeting
within `L` of a curve's end is classified by D3 like any other: a crossing
where the directions differ, a touch where the curves are tangent there.
It is not a kind of its own: a T, an arc end lying on a line and two
unjoined ends at one place are found by the same tests as an interior
meeting, and C5's split treats them alike. On the curve whose end it is,
the meeting's parameter is that end's (0 or 1), and its point is that
end's point, the caller's own; when both curves have an end there, the
first curve's. Two collinear lines meeting end to end without a join are
thus a touch, two lines forming an unjoined corner a crossing (also
reported as an unjoined pair under ADR-0012 D4).

**D5. Joins exclude.** A meeting within `L` of an end the two curves share,
by point id or by join class (ADR-0012 D1), is that join and is not
reported. Their other meetings are: a line joined to an arc that crosses
it again, or an overlap that runs from the join along both. Two lines
joined end to end, collinear or not, therefore report nothing; so does a
slot's tangent line–arc join.

**D6. Arcs.** An arc is measured on its circle, radius `|s − c|`
(ADR-0001). A meeting of the circle counts when its angle lies in the
arc's counter-clockwise sweep from start to end, widened at each end by
`L / r`; a widened sweep of a full turn or more is the whole circle. An
overlap of two arcs, or of an arc and a circle, is the shared part of the
sweeps, which can be two stretches.

**D7. Parameters and order.** Each meeting carries its parameter on both
curves: a line's `t` in [0, 1] from `a` (the projection, held to the
range); an arc's fraction of its sweep from its start, in [0, 1]; a
circle's angle from +x over 2π, in [0, 1). An overlap carries both ends'
parameters, `at` the end with the lower parameter on the first curve; on
the second they may run downward (opposite lines), and on a circle a
stretch runs counter-clockwise from `on` to `to`, past 1 if it wraps. Two
coincident circles are one overlap from the first's point at angle 0, all
the way round (parameters 0 to 1 on both). Meetings in a pair are ordered
by parameter on the first curve, ties by the second; the meetings on one
curve are ordered by its parameter, ties by the other curve's id, then
its parameter. Ordering uses `f64::total_cmp`.

**D8. Numerics.** Each pair is measured in the frame of ADR-0012 D3,
two-stage: scaled by an exact power of two so the largest coordinate is
below 1, translated to the first curve's origin (a line's `a`, a round's
centre), scaled again so the pair's extent is in [½, 1). `L` goes into
the frame with the same powers of two. Nothing overflows near 1e±300 and
a small pair far from the origin keeps its digits. Square roots of
differences are taken as products, `√((r − d)(r + d))`, not
`√(r² − d²)`. Angles through `libm::atan2`, and every result is mapped
back through the same frame.

## Consequences

- A touch the caller asked for (`tangent`, ADR-0002, ADR-0015) is reported
  as a touch. One the coordinates happen to make is a touch within `L` and
  a crossing beyond it, so during a drag it can flip between one touch and
  two crossings; a caller who means a touch declares it.
- An intersection names curves and parameters and changes nothing: profile
  extraction ignores it (ADR-0012), and only C5's split turns one into a
  join.
- Pairs are all `n(n − 1)/2` of them; a bounding-box test that changes no
  result may skip pairs. A spatial index waits until M14 measures the cost.
- The checker (M14) measures the same pairs with its own code, at
  `Tolerances::at` like the rest of it.

## Alternatives considered

- **An end on a curve as a kind of its own** ("end contact"): it would put
  a T and an interior crossing in different kinds although C5 splits both
  the same way, and an arc end lying tangent on a line would need two
  kinds at once. Rejected; D4's parameter of 0 or 1 says it.
- **Touch only at exact tangency** (`d = r` in floating point): no
  computed pair is exactly tangent, so a declared tangent would be reported
  as two crossings or none. Rejected.
- **A relative tolerance with no floor**, so 1e-300 geometry classifies at
  its own scale: it would certify geometry the solver cannot resolve and
  that ADR-0003 says is not resolved. Rejected.
- **Reporting meetings at a join**: every joined corner would be an
  intersection, and C5 would split a curve at a point already shared.
  Rejected.
