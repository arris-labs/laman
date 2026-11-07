# Plan: m10-profile-check

- Started: 2026-11-07
- Milestone: M10 (cycle C3, docs/ROADMAP.md §C3)
- Idea: none; verbatim from the human: "/plan m10". ADR-0012 is settled.

## Goal
`laman_check::check_profiles` re-measures what `laman::profile::profiles`
reported, from model types only: it agrees on every C3 fixture and slot grid
and names the fault in each broken twin. The digest covers profiles and is
equal natively and on wasm32-wasip1, with the 50 C1 digests unmoved.
Extraction at 500 entities has a named budget, which closes the roadmap's
⚠ OPEN.

## Non-goals
- `laman-py` over profiles and the check: M11. No changelog bullet here,
  since nothing in M10 reaches a published API (`laman-check` is unpublished).
- Re-deriving the open, dangling and branched classification in the checker:
  it holds that each edge is listed once, not which list (backlog line).
- The tangent contact drift (`regression/coincident-contact-drift`), and
  splitting the gate (backlog).

## Design deltas
**D1. The measure (ADR-0013).** Written from DATA-MODEL.md, in the checker's
own power-of-two frame. Per edge, about the loop's first point: a line is one
triangle; an arc is triangle, sector `½r²θ`, triangle. Extraction takes chord
plus segment with a series, so the sector form shares neither the formula nor
the small-sweep path. A circle is `πr²`. Orientation is the sign of this area:
a turning number is undefined on the crossing loops ADR-0012 lets through.

**D2. A loop with an open join has no area to compare.** Green's sum depends
on the origin wherever a join's ends lie apart, and a `coincident` the coordinates
violate still closes its loop (ADR-0012 D1): `profiles` measures from its
first join, the check from elsewhere, and neither is wrong. Likewise an arc
whose ends sit at different radii, since extraction reads `r` from the start.
Such a loop gets `OpenJoin` or `ArcOffRadius`, and its area is not compared.
Below that threshold, the allowance is `L·P + ½·D·Σg`: `L` the loop's largest
`Tolerances::at` entity tolerance, `P` the perimeter, `D` the extent, `g` each
join's separation. Any rounding term is a named constant, measured by step 2. Under the
allowance the sign is undecided, not a violation.

**D3. Extremes.** Compared in the frame, never in caller units: a reported
`±f64::MAX` agrees with a measure at or past it of the same sign, and a
reported 0 or subnormal with a measure that underflows. No NaN verdict.

**D4. Containment (step 4).** Each hole's probe point, by signed ray
crossings against its parent's boundary, and against no other, where
extraction sums angles for its winding number. A probe within `L` of the
boundary is undecided. Every profile's outer probe is held outside every
other profile's plate the same way, islands included.

**D5. Public types, `laman-check`, additive.**
`check_profiles(&Sketch, &Coordinates, &Profiles) -> ProfileReport`;
`ProfileReport { violations: Vec<ProfileViolation>, missing: Vec<Missing> }`
with `is_clean`; `ProfileViolation`, each variant naming a loop by its first
edge id: `Area { measured, reported }`, `Orientation`, `NotChained`, `OpenJoin`,
`ArcOffRadius`, `NotAnEdge`, `Repeated`, `Uncovered`, `Containment`. Join
classes are the checker's own walk over `coincident`, not `laman::profile`.
Any `Profiles` value is an input: an empty loop, a stale id or a wrong kind
is a violation, never a panic.

**D6. Digest, `laman-debug`, additive.** `of_profiles(sketch, values, profiles,
report)` for a run at expected coordinates; `of_solved_profiles(sketch,
solution, report, profiles, profile_report)` feeds `feed_solution` first.
`of_solution` is untouched, so no C1 digest moves. Blessed in
`tests/fixtures/c3/digests.txt`, embedded in `tests/determinism.rs`, the one
test `tools/digest-wasm.sh` runs. `c3.rs`'s `every_directory_is_listed` must
skip the new file.

**D7. Budget (ADR-0014).** The stop rule of ADR-0011 applied: the number is
the worst across three runs, rounded up to the next 0.5 ms. `bench_c2` gains a
`profiles` operation over the three families, and a row of 249 circles in the
bench alone, the shape extraction's quadratic containment is worst on. A cell
past the 4 ms frame gets one profile-led pass, and then its number stands.

## Steps
Complexity grades; a step's grade is its tier (`.agents/tiers.md`): **[1]**
routine — the design says exactly what to write and the tests are mechanical;
**[2]** careful — a numeric or geometric case to get right within a given
design; **[3]** unproven — an algorithm whose robustness or bound is
established here, or a rule the design leaves open.

- [x] Step 1 **[3]** — ADR-0013: D1 to D4, with the alternatives (turning
  number, flattening to a polygon, comparing such loops at extraction's
  origin). Its glossary entries in the same commit.
- [ ] Step 2 **[3]** — The measure in `laman-check`, private: frame, area,
  join separations. Held to closed forms (rectangle, slot, circle, a near-straight
  arc of huge radius, a sweep past π), at 1e±300, and on a small loop far from
  the origin. Fixes the rounding term, if one is needed.
- [ ] Step 3 **[2]** — `check_profiles` and `ProfileReport` (D5) without
  containment: join walk, closure, coverage, area, orientation. `c3.rs` runs it
  on every fixture at solved and expected coordinates, slot grids included.
- [ ] Step 4 **[2]** — Containment (D4): rays against lines and arcs, the
  half-open rule at vertices, the undecided band. `plate-island` and the nested
  property shapes held.
- [ ] Step 5 **[2]** — Broken twins: a table of mutations over the fixtures
  in `tests/c3_review.rs`, each naming the variant it must raise (area nudged,
  a loop reversed, a hole moved to the wrong parent, an edge dropped or
  repeated, a stale id, an empty loop, values moved after extraction).
- [ ] Step 6 **[2]** — `c3_props.rs`: `check_profiles` clean on every built
  nest under every transform and in `extreme` at 1e±300. A failure is
  shrunk to a fixture under `regression/`.
- [ ] Step 7 **[2]** — `profiles_sketch` fuzz target: the check on found and
  on mutated `Profiles`, never a panic, and clean wherever `check` is clean.
- [ ] Step 8 **[2]** — Digest (D6): `of_profiles`, `of_solved_profiles`,
  `c3/digests.txt` per fixture (run, `solved-lm-`, `solved-dogleg-`) and for one
  slot grid, blessed after `c3.rs` holds the same runs. `tools/digest-wasm.sh`
  green; `c1/digests.txt` has no diff.
- [ ] Step 9 **[2]** — Bench and ADR-0014 (D7): measure, name the budget,
  restate C3's *Accept* with it, delete the ⚠ OPEN.
- [ ] Step 10 **[2]** — Only if a cell of step 9 is past 4 ms: the one
  profile-led pass, changing no bit (`c3/digests.txt` unmoved), and ADR-0014's
  number measured again. Otherwise ticked with a note that it was not needed.

## Acceptance
- `cargo test -p laman-debug --test c3 --test c3_review --test c3_props`: the
  checker clean on all 10 C3 fixtures at solved and expected coordinates and
  on the slot grids. Its measured areas match the closed forms: 7200,
  7200 − (400 + 25π) = 6721.46, 7200 − 400π with its island 100π, 25π.
- Every mutation in the twin table raises exactly the variant it names.
- `extreme` is clean at 1e±300; no verdict is NaN.
- `tools/digest-wasm.sh` prints every C1 and C3 digest identical natively and
  on wasm32-wasip1. `tests/fixtures/c1/digests.txt` is unchanged. The five
  runners hold the same digests through the gate at the next push.
- `bench_c2` prints the `profiles` cells at 500 entities inside ADR-0014's
  number. `cargo fuzz run profiles_sketch` for 10 minutes finds nothing new.
- `tools/check-layers.sh` green: the checker still names only `laman::model`.

## Docs to update on completion
- `docs/ARCHITECTURE.md` §The checker — `check_profiles`, ADR-0013;
  §Profiles — what an open join means for a loop's area;
  §Determinism — the profile digests, the count, the fuzz target's new clause.
- `docs/ROADMAP.md` §C3 — status line, the budget (done by step 9).
- `docs/BACKLOG.md` — the classification re-derivation of *Non-goals*.
- `tools/digest-wasm.sh` and `tests/determinism.rs` header comments — C3.
- `docs/adr/README.md` — ADRs 0013 and 0014 in its index.
- `GLOSSARY.md` — **Open join**; any other term ADR-0013 sharpens.
- `AGENTS.md` current state — M10 landed; next M11.

## Open questions
- Settled (conductor, 2026-11-08): containment (D4, step 4) is kept. It goes
  past the roadmap's in-list ("area and orientation"), but the cycle's goal and
  risk both name nesting and nothing else holds it independently; the checker
  is unpublished, so no caller sees the widening. Step 1 widens the in-list
  line. The human may drop it later (steps 4 and the wrong-parent twin go).
- Settled (step 1, ADR-0013 D4): containment counts ray crossings signed,
  not even-odd. Even-odd disagrees with extraction's nonzero winding rule
  wherever a crossing loop (which ADR-0012 lets through) winds twice, so a
  correct extraction would fail its check; the signed count costs the same
  ray. Also settled there: an arc's sector reads `r² = |s − c|·|e − c|`, and
  a non-finite coordinate is listed in `missing` like an absent one (D3).
- ⚠ OPEN: the rounding term in D2, if one is needed at all. The agent, by
  step 2, from the near-straight arc and the 500-edge loop.
