# Changelog

What a user of the `laman` crate or the `laman` Python package can do,
release by release. Written in the commit that makes the change, under
`Unreleased` (`.agents/rules/git.md` §The changelog).

## Unreleased

## 0.0.1 — 2026-11-01

- `laman::rigidity(points, bars)` (Python: `laman.rigidity`): the generic
  rigidity of a bar framework in the plane, by Laman's theorem. Returns the
  remaining internal degrees of freedom and which bars are independent and
  which redundant, by index. A bar naming a point out of range, or the same
  point twice, is refused with a `RigidityError` naming the bar.
