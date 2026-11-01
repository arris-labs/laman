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
ADR-0001.

| # | Title | Status |
|---|---|---|
| [0001](0001-an-arc-is-a-centre-and-two-endpoints.md) | An arc is a centre and two endpoints, counter-clockwise, its radius implied | accepted |
| [0002](0002-regularize-structural-tangent-contacts.md) | Regularize structural tangent contacts | accepted |
| [0003](0003-scale-relative-tolerances-and-rank-order.md) | Scale-relative tolerances and deterministic rank ordering | accepted |
