# Changelog

What a user of the `laman` crate or the `laman` Python package can do,
release by release. Written in the commit that makes the change, under
`Unreleased` (`.agents/rules/git.md` §The changelog).

## Unreleased

- `laman::model`: build a sketch of points, lines, circles and arcs with
  ids, tags, fixed parameters, construction flags and opaque payloads, and
  add all 18 constraints between them. A malformed request (an unknown id,
  the wrong kind of entity, a repeated entity, a non-finite or out-of-range
  value, removing an entity a constraint uses) is refused with a
  `ModelError` naming your ids. Tangent and circle-contact constraints take
  their side from the drawing.
- `laman::residual`: evaluate each constraint's equations, and an arc's own
  equation, at any coordinates: the value, its unit (length or
  dimensionless) and its gradient over the parameters it reads. A
  coordinate you do not supply is read from the sketch; an undefined point
  (a zero-length line) gives a finite value flagged degenerate, never NaN.
- A sketch saves to and loads from JSON (feature `serde`, on by default),
  versioned; a malformed file is refused with an error naming the id.

## 0.0.1 — 2026-11-01

- `laman::rigidity(points, bars)` (Python: `laman.rigidity`): the generic
  rigidity of a bar framework in the plane, by Laman's theorem. Returns the
  remaining internal degrees of freedom and which bars are independent and
  which redundant, by index. A bar naming a point out of range, or the same
  point twice, is refused with a `RigidityError` naming the bar.
