"""A sketch loaded from JSON can be edited: handles come from the sketch,
and the read side shows what it holds."""

import pytest

from laman import Circle, Line, ModelError, Point, Sketch, Status
from laman._errors import UnknownId, WrongKind
from laman.check import check
from test_c1 import load

Q = 21  # slotted-plate-drag's stray free point


def test_a_loaded_sketch_takes_a_new_distance() -> None:
    sk = load("slotted-plate-drag").sketch
    before = len(sk.constraints())
    p3, q = sk.point_handle(3), sk.point_handle(Q)
    assert isinstance(p3, Point) and p3.id == 3
    cid = sk.distance(p3, q, 25.0)
    assert len(sk.constraints()) == before + 1
    assert sk.constraint_value(cid) == 25.0
    assert sk.constraint_entities(cid) == [3, Q]
    solution = sk.solve()
    assert solution.status is Status.UnderConstrained
    assert check(sk, solution.coordinates).is_clean()
    sk.apply(solution)
    dx = sk.coordinates()[(3, "x")] - sk.coordinates()[(Q, "x")]
    dy = sk.coordinates()[(3, "y")] - sk.coordinates()[(Q, "y")]
    assert abs((dx * dx + dy * dy) ** 0.5 - 25.0) < 1e-6


def test_the_read_side_lists_entities_and_constraints() -> None:
    sk = load("sss-triangle").sketch
    kinds = {e.id: e.kind for e in sk.entities()}
    assert set(kinds.values()) <= {"point", "line", "circle", "arc"}
    assert [e.id for e in sk.entities()] == sorted(kinds)
    cs = sk.constraints()
    assert [c.id for c in cs] == sorted(c.id for c in cs)
    dims = [c for c in cs if c.kind == "distance"]
    assert dims and all(sk.constraint_value(c.id) is not None for c in dims)
    other = next((c for c in cs if c.kind != "distance"), None)
    if other is not None:
        assert sk.constraint_value(other.id) is None
    assert sk.constraint_value(10**6) is None
    assert sk.constraint_entities(10**6) is None


def test_a_handle_of_the_wrong_kind_or_id_is_a_typed_error() -> None:
    sk = Sketch()
    p, q = sk.point(0.0, 0.0), sk.point(1.0, 0.0)
    line = sk.line(p, q)
    assert sk.line_handle(line.id) == line
    assert isinstance(sk.line_handle(line.id), Line)
    with pytest.raises(WrongKind):
        sk.point_handle(line.id)
    with pytest.raises(UnknownId):
        sk.circle_handle(999)
    with pytest.raises(ModelError):
        sk.arc_handle(p.id)
    c = sk.circle(p, 2.0)
    assert isinstance(sk.circle_handle(c.id), Circle)


def test_drag_takes_a_handle_or_an_id() -> None:
    sk = load("slotted-plate-drag").sketch
    by_handle = sk.drag(sk.point_handle(Q)).finish()
    by_id = sk.drag(Q).finish()
    assert by_handle.coordinates == by_id.coordinates
