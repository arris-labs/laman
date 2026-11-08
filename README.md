# laman

A deterministic 2D geometric constraint solver in pure Rust, for Rust and
Python, built to be trusted.

`laman` is a library for sketchers, code-CAD tools, mechanism designers and
anything else that places points, lines, circles and arcs under geometric
and dimensional constraints. Its aim: the permissively licensed sketch
solver Python does not have, built from the same Rust core as the crate;
diagnostics in the caller's own ids and tags (the exact remaining degrees
of freedom, per entity, and the constraints to blame when a sketch cannot
be satisfied); dragging that keeps the solution the user is on and returns
the same bits on every platform; and every solve re-checkable by an
independent verifier that shares no code with the solver. MIT OR
Apache-2.0.

## Status

Early. The sketch solver is being built in the open
([`SEED.md`](https://github.com/arris-labs/laman/blob/main/SEED.md) is the
charter). In Python you can build a sketch, solve it, drag it, read every
diagnostic, read its profiles and re-check them; the package also holds the
combinatorial core the project is named after, the **generic rigidity
count of a bar framework** in the plane.

```rust
// A square flexes: one internal freedom. A diagonal makes it rigid,
// and a second diagonal is redundant.
let bars = [(0, 1), (1, 2), (2, 3), (3, 0), (0, 2), (1, 3)];
let r = laman::rigidity(4, &bars[..4]).unwrap();
assert_eq!(r.dof, 1);
let r = laman::rigidity(4, &bars).unwrap();
assert_eq!(r.dof, 0);
assert_eq!(r.redundant, vec![5]);
```

```python
import laman
from laman.check import check

# A triangle from its three side lengths (50, 30, 40): the origin is
# fixed and the first side lies along the x axis.
sk = laman.Sketch()
a = sk.point(0, 0, fixed=True, tag="origin")
b = sk.point(40, 5)
c = sk.point(10, 30)
ab = sk.line(a, b)
sk.horizontal(ab)
sk.distance(a, b, 50, tag="base")
sk.distance(a, c, 30, tag="left")
sk.distance(b, c, 40, tag="right")

solution = sk.solve()
assert solution.status is laman.Status.FullyConstrained
assert solution.diagnostics.dof == 0
assert abs(solution.coordinates[(b.id, "x")] - 50) < 1e-9
assert abs(solution.coordinates[(c.id, "y")] - 24) < 1e-9
assert check(sk, solution.coordinates).is_clean()  # re-checked, independently
```

The closed outlines of a sketch and the holes in them are its profiles:

```python
import math

import laman
from laman.check import check_profiles
from laman.profile import profiles

# A 40 x 30 plate with a round hole of radius 5.
sk = laman.Sketch()
corners = [sk.point(x, y) for x, y in [(0, 0), (40, 0), (40, 30), (0, 30)]]
for i in range(4):
    sk.line(corners[i], corners[(i + 1) % 4])
sk.circle(sk.point(20, 15), 5)

found = profiles(sk, sk.coordinates())
(plate,) = found.profiles
assert plate.outer.signed_area == 1200  # counter-clockwise: positive
assert len(plate.holes) == 1 and plate.holes[0].signed_area < 0
assert abs(plate.net_area - (1200 - 25 * math.pi)) < 1e-9
assert check_profiles(sk, sk.coordinates(), found).is_clean()  # re-measured
```

The generic rigidity count of a bar framework is `laman.rigidity`:

```python
import laman

r = laman.rigidity(4, [(0, 1), (1, 2), (2, 3), (3, 0), (0, 2), (1, 3)])
assert r.dof == 0 and r.redundant == [5]
```

```sh
cargo add laman
pip install laman
```

## The name

`laman` is named after Laman's theorem (G. Laman, *On graphs and rigidity
of plane skeletal structures*, 1970), which says exactly when points
joined by rigid bars in the plane are rigid: `2n − 3` bars for `n` points,
and no subset of `k` points carrying more than `2k − 3` of them. Counting
the freedom that remains, and saying where constraints are missing or in
excess, is what this constraint solver is for. The name implies no
endorsement by Laman or his estate.

## Licence

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option. Unless you explicitly state
otherwise, any contribution intentionally submitted for inclusion in this
work by you, as defined in the Apache-2.0 license, shall be dual licensed
as above, without any additional terms or conditions.
