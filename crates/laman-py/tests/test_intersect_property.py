"""Crossing cutters by construction: a base line along x is cut by vertical
lines at distinct abscissae, each spanning the base. Every cutter crosses the
base exactly once, at a point and with parameters known from the
construction, and cutters do not meet each other. `intersections` reports
exactly those pairs and the checker finds nothing to name; with one cutter
dropped from the report, it names the missed pair.
"""

from hypothesis import given, strategies as st

import laman
from laman.check import check_intersections
from laman.intersect import Intersections, intersections

LENGTH = 1000.0
# Cutters sit on a grid of 10, so no two are nearer than the checker's bands.
SLOTS = st.lists(st.integers(min_value=1, max_value=99), min_size=1, max_size=12, unique=True)
HEIGHT = st.integers(min_value=5, max_value=500)
RELATIVE = 1e-9


@given(slots=SLOTS, height=HEIGHT, y=st.integers(min_value=-200, max_value=200))
def test_cutters_cross_a_base_where_they_were_built_to(slots: list[int], height: int, y: int) -> None:
    sk = laman.Sketch()
    left, right = sk.point(0.0, float(y)), sk.point(LENGTH, float(y))
    base = sk.line(left, right)
    cutters = {}
    for s in slots:
        x = 10.0 * s
        bottom, top = sk.point(x, y - float(height)), sk.point(x, y + float(height))
        cutters[sk.line(bottom, top).id] = x
    values = sk.coordinates()
    found = intersections(sk, values)
    assert {(p.first, p.second) for p in found.pairs} == {(base.id, c) for c in cutters}
    for pair in found.pairs:
        (m,) = pair.meetings
        x = cutters[pair.second]
        assert m.kind.name == "crossing"
        assert abs(m.at[0] - x) <= RELATIVE * LENGTH and abs(m.at[1] - y) <= RELATIVE * LENGTH
        assert abs(m.on_first - x / LENGTH) <= RELATIVE
        assert abs(m.on_second - 0.5) <= RELATIVE
    assert check_intersections(sk, values, found).is_clean()

    # The first pair dropped is named, and only it.
    twin = Intersections(found.pairs[1:])
    missed = [(v.first, v.second) for v in check_intersections(sk, values, twin).violations if v.kind == "missed"]
    assert missed == [(found.pairs[0].first, found.pairs[0].second)]
