"""Nested shapes built by construction, read back as profiles.

Cells sit 100 apart along x. A cell is a plate (an 80 x 80 square) or a disc.
A plate has four quadrants (40 x 40, centres 20 and 60), each empty, a square
hole, a round hole, or a larger square hole holding an island square. The
counts and every area come from that construction, never from `profiles`.
"""

import math
from dataclasses import dataclass, field

from hypothesis import given, strategies as st

from laman import Coordinates, Sketch
from laman.check import check_profiles
from laman.profile import Loop, Profile, Profiles, profiles

PLATE = 80.0
RELATIVE = 1e-9
NUDGE = 1e-3


@dataclass
class Built:
    sketch: Sketch
    plates: list[float] = field(default_factory=list)  # net areas expected
    plate_holes: list[int] = field(default_factory=list)  # hole counts, per plate profile
    outers: list[float] = field(default_factory=list)  # outer areas expected
    holes: list[float] = field(default_factory=list)  # |hole| areas expected


def square(sk: Sketch, x: float, y: float, side: float) -> None:
    corners = [sk.point(px, py) for px, py in [(x, y), (x + side, y), (x + side, y + side), (x, y + side)]]
    for i in range(4):
        sk.line(corners[i], corners[(i + 1) % 4])


KINDS = st.sampled_from(["empty", "square", "round", "island"])
SIDES = st.integers(min_value=4, max_value=24)
RADII = st.integers(min_value=2, max_value=19)
CELLS = st.lists(
    st.one_of(
        st.tuples(st.just("disc"), st.integers(min_value=5, max_value=45)),
        st.tuples(st.just("plate"), st.lists(st.tuples(KINDS, SIDES, RADII), min_size=4, max_size=4)),
    ),
    min_size=1,
    max_size=4,
)


def build(cells: list[tuple[str, object]]) -> Built:
    b = Built(Sketch())
    sk = b.sketch
    for index, (kind, spec) in enumerate(cells):
        x0 = 100.0 * index
        if kind == "disc":
            assert isinstance(spec, int)
            r = float(spec)
            sk.circle(sk.point(x0 + 50.0, 50.0), r)
            b.plates.append(math.pi * r * r)
            b.plate_holes.append(0)
            b.outers.append(math.pi * r * r)
            continue
        assert isinstance(spec, list)
        square(sk, x0, 0.0, PLATE)
        net = PLATE * PLATE
        count = 0
        for q, (hole, side, radius) in enumerate(spec):
            cx = x0 + (20.0 if q % 2 == 0 else 60.0)
            cy = 20.0 if q < 2 else 60.0
            if hole == "square":
                s = float(side)
                square(sk, cx - s / 2, cy - s / 2, s)
                net -= s * s
                b.holes.append(s * s)
            elif hole == "round":
                r = float(radius)
                sk.circle(sk.point(cx, cy), r)
                net -= math.pi * r * r
                b.holes.append(math.pi * r * r)
            elif hole == "island":
                # A 24 hole holds an island of side 3..13.
                island = float(side // 2 + 1)
                square(sk, cx - 12.0, cy - 12.0, 24.0)
                square(sk, cx - island / 2, cy - island / 2, island)
                net -= 24.0 * 24.0
                b.holes.append(24.0 * 24.0)
                b.plates.append(island * island)
                b.plate_holes.append(0)
                b.outers.append(island * island)
            else:
                continue
            count += 1
        b.plates.append(net)
        b.plate_holes.append(count)
        b.outers.append(PLATE * PLATE)
    return b


def close(got: list[float], want: list[float]) -> bool:
    return len(got) == len(want) and all(
        abs(g - w) <= RELATIVE * max(1.0, abs(w)) for g, w in zip(sorted(got), sorted(want), strict=True)
    )


@given(CELLS)
def test_the_profiles_are_the_construction(cells: list[tuple[str, object]]) -> None:
    built = build(cells)
    values: Coordinates = built.sketch.coordinates()
    found = profiles(built.sketch, values)
    assert not found.open_chains and not found.dangling and not found.branched
    assert not found.degenerate and not found.unjoined
    assert len(found.profiles) == len(built.plates)
    assert sorted(len(p.holes) for p in found.profiles) == sorted(built.plate_holes)
    assert close([p.net_area for p in found.profiles], built.plates)
    assert close([p.outer.signed_area for p in found.profiles], built.outers)
    assert all(p.outer.signed_area > 0.0 for p in found.profiles)
    assert close([-h.signed_area for p in found.profiles for h in p.holes], built.holes)
    assert all(h.signed_area < 0.0 for p in found.profiles for h in p.holes)
    report = check_profiles(built.sketch, values, found)
    assert report.is_clean(), report.violations


@given(CELLS, st.data())
def test_a_nudged_loop_is_one_area_and_one_net_area(
    cells: list[tuple[str, object]], data: st.DataObject
) -> None:
    built = build(cells)
    values = built.sketch.coordinates()
    found = profiles(built.sketch, values)
    slots: list[tuple[int, int | None]] = [(pi, None) for pi in range(len(found.profiles))]
    slots += [(pi, hi) for pi, p in enumerate(found.profiles) for hi in range(len(p.holes))]
    pi, hi = data.draw(st.sampled_from(slots))

    def nudged(loop: Loop) -> Loop:
        return Loop(loop.edges, loop.signed_area * (1.0 + NUDGE), loop.joins)

    twin_profiles: list[Profile] = []
    for i, p in enumerate(found.profiles):
        outer = nudged(p.outer) if (i, hi) == (pi, None) else p.outer
        holes = [nudged(h) if (i, j) == (pi, hi) else h for j, h in enumerate(p.holes)]
        # The twin is consistent with its own loops: its net is its outer less
        # its holes, so the nudge shows in the net too, whose reported value
        # the check compares with its own measure.
        net = outer.signed_area + sum(h.signed_area for h in holes)
        twin_profiles.append(Profile(outer, holes, net))
    twin = Profiles(profiles=twin_profiles)

    target = found.profiles[pi]
    loop = target.outer if hi is None else target.holes[hi]
    violations = check_profiles(built.sketch, values, twin).violations
    assert sorted(v.kind for v in violations) == ["area", "net_area"], violations
    by_kind = {v.kind: v for v in violations}
    assert by_kind["area"].first == loop.edges[0].entity
    assert by_kind["area"].reported == loop.signed_area * (1.0 + NUDGE)
    assert by_kind["net_area"].first == target.outer.edges[0].entity
