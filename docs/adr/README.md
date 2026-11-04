# Architecture decision records

One file per decision, numbered `NNNN-<slug>.md`, never edited after
acceptance — a change of mind is a new ADR that supersedes the old one.
Format: Context, Decision, Consequences, Alternatives considered. An ADR
whose design was informed by SolveSpace or PlaneGCS names the file or
module read, so provenance is auditable (`PROVENANCE.md`, SEED.md §8).

The decisions taken at kickoff live in `SEED.md` §9 and are not repeated
here; the first ADR is the first decision taken *after* the seed. Open at
kickoff, each closed by an ADR (SEED.md §10.8): the arc parametrisation;
absolute vs. scale-relative tolerances; how branch preservation is defined
and tested; whether the minimal conflict set is exact or a bounded
approximation on large sketches. The arc parametrisation is closed by
ADR-0001, tolerances by ADR-0003, the conflict set by ADR-0005, branch
preservation by ADR-0006: every kickoff question is closed.

| # | Title | Status |
|---|---|---|
| [0001](0001-an-arc-is-a-centre-and-two-endpoints.md) | An arc is a centre and two endpoints, counter-clockwise, its radius implied | accepted |
| [0002](0002-regularize-structural-tangent-contacts.md) | Regularize structural tangent contacts | accepted |
| [0003](0003-scale-relative-tolerances-and-rank-order.md) | Scale-relative tolerances and deterministic rank ordering | accepted; rank normalization amended by 0009 |
| [0004](0004-checker-tolerance-at-checked-coordinates.md) | The checker measures its tolerance at the coordinates it checks | accepted |
| [0005](0005-conflict-sets-by-certified-deletion.md) | Conflict sets by certified deletion, redundancy by removal | accepted |
| [0006](0006-branches-by-observation.md) | Branches by observation, compared beyond a margin | accepted |
| [0007](0007-the-binding-carries-the-checker-the-dumps-and-the-digest.md) | The binding carries the checker, the dumps and the digest | accepted |
| [0008](0008-the-release-gate-is-the-tags-own-ci-run.md) | The release gate is the tag's own CI run | accepted |
| [0009](0009-stages-and-part-rank.md) | Stages, part rank and the whole-component fallback | accepted |
