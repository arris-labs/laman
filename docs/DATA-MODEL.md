# Data model

What a sketch holds and what each part means as equations: entity
parametrisations, every constraint's residuals and the DoF it removes,
what the diagnostics report, and the serde form. How the pieces are
stored and solved is [ARCHITECTURE.md](ARCHITECTURE.md).

## Conventions

- The plane is `(x, y)`, right-handed: counter-clockwise is positive.
  Angles are radians, measured counter-clockwise from `+x`.
- Units are the caller's (SEED.md §9). Every length a constraint carries
  is in them.
- `f64` throughout.
- Notation below: a point `p = (x_p, y_p)`; a line `L` from `a` to `b` with
  direction `u = b − a`; `cross(v, w) = v_x w_y − v_y w_x`; `|v|` the
  Euclidean length (`sqrt`, correctly rounded). `r(X)` is the radius of a
  circle or arc `X` (§Entities).

## Entities

| Entity | Defined by | Own parameters | Intrinsic equations | DoF (free, alone) |
|---|---|---|---|---|
| Point | — | `x`, `y` | — | 2 |
| Line segment `L` | start point `a`, end point `b` | — | — | 4 (its points') |
| Circle `C` | centre point `c` | radius `r` | — | 3 |
| Arc `A` | centre `c`, start `s`, end `e` (ADR-0001) | — | `|e − c| − |s − c| = 0` | 5 |

- Lines, circles and arcs **refer** to points; two entities that share a
  point share its parameters. That is how a profile joins up: no
  coincident constraint between a line's end and an arc's end that are the
  same point.
- `r(C)` is the circle's parameter; `r(A) = |s − c|`.
- An arc runs counter-clockwise from `s` to `e`. Its sweep is
  `atan2(cross(s − c, e − c), dot(s − c, e − c))`, taken in `(0, 2π)`. It
  is observed, never an equation (ARCHITECTURE.md §Branches).
- A line's end points are distinct, a circle's centre is not a point of
  the circle, an arc's three points are distinct: the sketch refuses an
  entity that names one point twice. At a solution they can still
  coincide numerically; that is a **degenerate** entity (§Diagnostics).
- A point with both coordinates fixed is **ground**. Any parameter can be
  fixed alone (a circle's radius, a point's `x`).

## Constraints

Each constraint is one or more residual equations `f = 0`, written in
length units (dimensionless ones where noted) so that one tolerance reads
the same everywhere. "Removes" is the DoF the constraint takes away when
its equations are independent of the rest; diagnostics measure the real
number from the rank (§Diagnostics). A **sense** `σ ∈ {+1, −1}` is fixed
when the constraint is added, from the sketch's current coordinates, and
stored with it (ARCHITECTURE.md §Branches).

| Constraint | Entities | Equations | Removes |
|---|---|---|---|
| `coincident` | points `p`, `q` | `x_p − x_q`, `y_p − y_q` | 2 |
| `point_on_line` | point `p`, line `L` | `cross(u, p − a) / |u|` (signed distance to the infinite line) | 1 |
| `point_on_circle` | point `p`, circle or arc `X` | `|p − c| − r(X)` (on the arc's whole circle; whether `p` is inside the sweep is observed) | 1 |
| `horizontal` | line `L`, or points `p`, `q` | `y_b − y_a` | 1 |
| `vertical` | line `L`, or points `p`, `q` | `x_b − x_a` | 1 |
| `parallel` | lines `L1`, `L2` | `cross(u1, u2) / (|u1| |u2|)` (dimensionless) | 1 |
| `perpendicular` | lines `L1`, `L2` | `dot(u1, u2) / (|u1| |u2|)` (dimensionless) | 1 |
| `equal_length` | lines `L1`, `L2` | `|u1| − |u2|` | 1 |
| `equal_radius` | circles or arcs `X`, `Y` | `r(X) − r(Y)` | 1 |
| `tangent` (line–circle) | line `L`, circle or arc `X` | `cross(u, c − a) / |u| − σ r(X)`, σ the side of `L` the centre is on | 1 |
| `tangent` (circle–circle) | circles or arcs `X`, `Y` | external: `|c_X − c_Y| − (r(X) + r(Y))`; internal: `|c_X − c_Y| − σ (r(X) − r(Y))`, the contact kind and σ from the drawing | 1 |
| `midpoint` | point `p`, line `L` | `x_p − (x_a + x_b)/2`, `y_p − (y_a + y_b)/2` | 2 |
| `symmetric` | points `p`, `q`, line `L` | `cross(u, m − a) / |u|` with `m = (p + q)/2`; `dot(u, q − p) / |u|` | 2 |
| `fix` | point `p`, at `(x₀, y₀)` | `x_p − x₀`, `y_p − y₀` | 2 |
| `distance` | points `p`, `q`, value `d > 0` | `|q − p| − d` | 1 |
| `point_line_distance` | point `p`, line `L`, value `d > 0` | `cross(u, p − a) / |u| − σ d`, σ the side of `L` `p` is on | 1 |
| `radius` | circle or arc `X`, value `ρ > 0` | `r(X) − ρ` | 1 |
| `angle` | lines `L1`, `L2`, value `θ ∈ (0, π)` | `(cross(u1, u2) cos θ − dot(u1, u2) sin θ) / (|u1| |u2|)`, i.e. `sin(φ − θ)` for the angle `φ` from `L1` to `L2` (dimensionless) | 1 |

- **Senses** are computed when the constraint is added, from the sketch's
  coordinates. A centre exactly on the line takes `σ = +1`. A circle–circle
  contact is external when `|c_X − c_Y|` is nearer `r(X) + r(Y)` than
  `|r(X) − r(Y)|`, and a tie is external; an internal contact takes `σ`
  from the sign of `r(X) − r(Y)`, a tie `+1`.
- **Distance 0 is coincidence**: `distance` and `point_line_distance`
  refuse `d ≤ 0` (`ModelError::DimensionOutOfRange`, naming `coincident`
  and `point_on_line`), because `|q − p|` has no gradient at `p = q`.
- **`fix` versus a fixed point**: `point_fixed` makes the coordinates
  ground and removes them from the unknowns; `fix` keeps them unknown and
  adds two equations. Only `fix` can be named in a conflict set.
- **`angle` and `parallel` have a second root** (`φ = θ + π`; antiparallel
  lines). The orientation of the two lines is observed, and its change is
  a flip (ARCHITECTURE.md §Branches).
- **Transcendentals**: only `angle` uses any, `cos θ` and `sin θ` of its
  constant value, through `libm`, once when the system is compiled.
- **Jacobians** are analytic, written beside each residual in
  `laman::residual`. An equation's gradient is a list of terms keyed by
  `(EntityId, Coordinate)`, merged by parameter (two lines sharing a point
  sum into one term) and sorted; it covers fixed parameters too. At an
  exactly undefined point (a zero-length line, a point on a centre,
  coincident centres) the undefined quotient is taken as 0, its gradient
  terms are left out, the value stays finite and the equation is flagged
  degenerate; lengths are computed scale-safely, so finite input never
  gives NaN or infinity. Each is checked against central finite
  differences (an eighth-order stencil, step `5e-3` of the equation's
  local scale, agreement within `1e-6` of the gradient's size;
  `laman_debug::fd`) for every constraint over random non-degenerate
  configurations and the near-degenerate ones a strategy can reach (a line
  of length `1e-6` times the sketch's size, a point on the line, tangent
  circles of nearly equal radius) (`.agents/rules/solver.md`).
- **Not in cycle 1**: point–circle distance, concentric, collinear (two
  `point_on_line`), diameter (a `radius` of half), arc length, sweep
  angle, ellipses, splines (SEED.md §6, later cycles). A kind enters when
  a consumer reaches it (SEED.md §8).

## Diagnostics

The M2 contract uses `J`, the Jacobian of required equations after
structural contact regularization (ADR-0002), including intrinsic equations,
with respect to the `n` free parameters. Rank, DoF, per-entity DoF and
free directions are computed (internal `diagnose`, public with the solve
API); redundant, conflicting and degenerate sets are the remaining M2 work.

- **Rank** uses scaled, unit-normalized rows and column-pivoted QR of `J`,
  with deterministic ties and null-space ordering (ADR-0003).
- **DoF** `= n − rank`: geometric freedom for regular configurations and
  the supported C1 contacts, an infinitesimal estimate at other
  singularities. Whole-sketch motions count; a free rigid shape has 3.
- **Per-entity DoF**: the null space `N` of `J` (an orthonormal basis from
  the same factorisation); an entity's DoF is the rank of `N`'s rows for
  its parameters (a line's: its two points'). A fully constrained entity
  has 0.
- **Free directions**: `N`'s columns, each mapped back to the motion of
  every entity it moves (`[(EntityId, Coordinate, f64)]`), in compiled
  order, component by component. A component at or below the rank
  threshold is rounding, not motion: it is a canonical zero and is not
  listed. A row that cannot be evaluated finitely adds no rank, so the
  DoF there is an upper estimate. For a human: "this point can still slide along x". For an agent:
  which dimension to add.
- **Redundant**: a satisfied constraint implied by the others. Analyze in
  source order, recompiling contacts and their provenance on removal;
  dependence of an original tangent equation alone proves nothing.
  A later duplicate is named. Redundancy is a warning, not a status.
- **Conflicting**: constraints established to be jointly unsatisfiable.
  Linear dependence or a failed numerical solve alone proves no conflict.
  `⚠ OPEN:` evidence and bounded inclusion-minimal analysis policy; the
  conflict ADR in M2 defines the guarantee. Minimal means no member can
  be removed, not minimum cardinality.
- **Degenerate entities**: a line shorter than the length tolerance, a
  circle or arc of radius below it, an arc whose end meets its start. Named
  with the equation they make ill-conditioned.
- **Flips**: each observed orientation that changed from the start
  (ARCHITECTURE.md §Branches), by constraint or entity.

Every item names constraints and entities as `(id, kind, tag)`; the caller's
payload is reachable through the id.

### Status

| Status | Means |
|---|---|
| `FullyConstrained` | every required residual within tolerance, DoF 0 (so the sketch has ground or `fix`: the whole sketch's motions count) |
| `UnderConstrained` | every required residual within tolerance, DoF above that |
| `Unsatisfiable` | a conflict set exists; the coordinates are the least-squares best effort |
| `NotConverged` | an iteration cap was hit with residuals above tolerance and no conflict proven |

Redundant constraints, flips and degenerate entities ride on any status
as diagnostics; none of them changes it. `Unsatisfiable` takes precedence
over `UnderConstrained`: a sketch can conflict in one place and be free in
another, and the diagnostics say both.

Tolerance and ordering policy: ADR-0003 and ADR-0004. The compiler and checker
use component-relative length tolerances from `model::tolerance::Tolerances`:
frozen from the input for the solver, measured at the checked coordinates for
the checker. Fixture coordinate acceptance stays unchanged.

## The serde form

JSON (or any serde format) with this shape:

```json
{
  "format": "laman.sketch",
  "version": 1,
  "entities": [
    { "id": 0, "kind": "point", "x": 0.0, "y": 0.0, "fixed": ["x", "y"], "tag": "origin" },
    { "id": 1, "kind": "point", "x": 40.0, "y": 5.0 },
    { "id": 2, "kind": "line", "a": 0, "b": 1, "construction": false },
    { "id": 3, "kind": "circle", "c": 1, "r": 10.0 },
    { "id": 4, "kind": "arc", "c": 1, "s": 5, "e": 6 }
  ],
  "constraints": [
    { "id": 0, "kind": "horizontal", "line": 2 },
    { "id": 1, "kind": "distance", "a": 0, "b": 1, "value": 50.0, "tag": "base" },
    { "id": 2, "kind": "tangent", "line": 2, "circle": 3, "sense": -1 }
  ]
}
```

- Entities and constraints in id order, each entity after the points it
  refers to. Members in the order shown: `id`, `kind`, the kind's own
  fields, then `construction`, `fixed`, `tag`, `payload`. Fields absent
  when default (`fixed: []`, no tag, no payload, `construction: false`). A
  payload is standard base64 with padding.
- Constraint fields by kind: `coincident`, `parallel`, `perpendicular`,
  `equal_length`, `equal_radius`, `angle` (with `value`), `distance` (with
  `value`) take `a`, `b`; `point_on_line` and `midpoint` take `point`,
  `line`; `point_on_circle` takes `point`, `circle`; `horizontal` and
  `vertical` take `line` or `a`, `b` (two points); `symmetric` takes `a`,
  `b`, `axis`; `fix` takes `point`, `x`, `y`; `point_line_distance` takes
  `point`, `line`, `value`, `sense`; `radius` takes `circle`, `value`.
  Wherever a circle is named, an arc's id works too. `tangent` takes
  `line`, `circle`, `sense` (line–circle), or `a`, `b`, `contact`
  (`"external"` or `"internal"`) and, for internal, `sense` (circle–circle).
- Senses are stored, so a reloaded sketch keeps its branches. A missing
  sense, or a missing `contact`, is taken from the coordinates at load, as
  on `add` (a centre exactly on the line is `+1`; a contact tie is
  external).
- Floats round-trip exactly (`float_roundtrip`); a fixture's 17 digits are
  the solver's input bits.
- **Versioning**: `version` is an integer. A reader loads every version up
  to its own, migrating in code, and refuses a newer one with
  `ModelError::FormatVersion { found, supported }`. A change a reader
  could not ignore bumps it; adding an optional field does not. A reader
  ignores members it does not know. Migration is one function per version
  step in `laman::model`; with version 1 the only one, it has no steps yet.
- Loading validates exactly as the builder does (ARCHITECTURE.md §The
  sketch and its ids): a malformed file is a `ModelError` naming the id.
  An error found in an entity or constraint of the file arrives wrapped in
  `ModelError::Entity` or `ModelError::Constraint` with that id, around the
  builder's own error, which names the ids it refers to. A `format` other
  than `laman.sketch` is `WrongFormat`; a payload that is not base64 is
  `InvalidPayload`.

`Solution` serialises in the same style (`"format": "laman.solution"`),
for the dumps and for agents that read it. The checker's `Report` has no
serde form yet; `laman-debug`'s dumps render it.
