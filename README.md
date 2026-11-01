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

Early. The sketch solver is being designed and built in the open
([`SEED.md`](https://github.com/arris-labs/laman/blob/main/SEED.md) is the
charter). This release holds the combinatorial core the project is named
after: the **generic rigidity count of a bar framework** in the plane.

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
