"""The builder: fixtures 1 to 3 built by hand in Python have the loaded
fixture's run digest, every entity and constraint method takes its keywords,
and each ModelError subclass is raised once."""

import json
import math
from typing import Any

import pytest

from laman import (
    Arc,
    Circle,
    Coordinates,
    Line,
    ModelError,
    Point,
    Sense,
    Sketch,
    Status,
)
from laman import _errors as errors
from laman.check import check
from laman.debug import digest_run
from test_c1 import DIGESTS, load


def replay(text: str) -> Sketch:
    """Rebuilds a sketch through the Python builder, entity by entity and
    constraint by constraint, with the file's ids, tags and flags."""
    form = json.loads(text)
    sk = Sketch()
    ent: dict[int, Any] = {}
    for e in form["entities"]:
        kw: dict[str, Any] = {
            "tag": e.get("tag"),
            "construction": e.get("construction", False),
            "id": e["id"],
        }
        fixed = set(e.get("fixed", []))
        kind = e["kind"]
        if kind == "point":
            ent[e["id"]] = sk.point(e["x"], e["y"], fixed=fixed >= {"x", "y"}, **kw)
        elif kind == "line":
            ent[e["id"]] = sk.line(ent[e["a"]], ent[e["b"]], **kw)
        elif kind == "circle":
            ent[e["id"]] = sk.circle(ent[e["c"]], e["r"], fixed="r" in fixed, **kw)
        else:
            ent[e["id"]] = sk.arc(ent[e["c"]], ent[e["s"]], ent[e["e"]], **kw)
        if kind == "point":
            for c in fixed - {"x", "y"} if fixed >= {"x", "y"} else fixed:
                sk.set_fixed(e["id"], c, True)
    for c in form["constraints"]:
        kw = {"tag": c.get("tag"), "id": c["id"]}
        kind = c["kind"]
        sense = {1: Sense.Positive, -1: Sense.Negative, None: None}[c.get("sense")]
        if kind == "coincident":
            sk.coincident(ent[c["a"]], ent[c["b"]], **kw)
        elif kind == "point_on_line":
            sk.point_on_line(ent[c["point"]], ent[c["line"]], **kw)
        elif kind == "point_on_circle":
            sk.point_on_circle(ent[c["point"]], ent[c["circle"]], **kw)
        elif kind in ("horizontal", "vertical"):
            span = (ent[c["line"]],) if "line" in c else (ent[c["a"]], ent[c["b"]])
            getattr(sk, kind)(*span, **kw)
        elif kind in ("parallel", "perpendicular", "equal_length", "equal_radius"):
            getattr(sk, kind)(ent[c["a"]], ent[c["b"]], **kw)
        elif kind == "tangent" and "line" in c:
            sk.tangent_line_circle(ent[c["line"]], ent[c["circle"]], sense=sense, **kw)
        elif kind == "tangent":
            external = c.get("contact") == "external"
            sk.tangent_circles(ent[c["a"]], ent[c["b"]], external=external, sense=sense, **kw)
        elif kind == "midpoint":
            sk.midpoint(ent[c["point"]], ent[c["line"]], **kw)
        elif kind == "symmetric":
            sk.symmetric(ent[c["a"]], ent[c["b"]], ent[c["axis"]], **kw)
        elif kind == "fix":
            sk.fix(ent[c["point"]], c["x"], c["y"], **kw)
        elif kind == "distance":
            sk.distance(ent[c["a"]], ent[c["b"]], c["value"], **kw)
        elif kind == "point_line_distance":
            sk.point_line_distance(ent[c["point"]], ent[c["line"]], c["value"], **kw)
        elif kind == "radius":
            sk.radius(ent[c["circle"]], c["value"], **kw)
        elif kind == "angle":
            sk.angle(ent[c["a"]], ent[c["b"]], c["value"], **kw)
        else:  # pragma: no cover
            raise AssertionError(kind)
    return sk


@pytest.mark.parametrize("name", ["sss-triangle", "belt", "slotted-plate"])
def test_fixtures_one_to_three_built_by_hand_have_the_loaded_run_digest(name: str) -> None:
    fx = load(name)
    built = replay(fx.text)
    assert built.to_json() == fx.sketch.to_json()
    assert built.coordinates() == fx.sketch.coordinates()
    values = dict(built.coordinates().items())
    values.update(fx.expected_coordinates())
    report = check(built, Coordinates(values))
    assert report.is_clean()
    assert digest_run(built, report) == DIGESTS[name]
    assert built.solve().status is Status.FullyConstrained


def test_a_hand_built_sketch_solves() -> None:
    sk = Sketch()
    a = sk.point(0.0, 0.0, fixed=True, tag="a")
    b = sk.point(4.0, 0.5, tag="b")
    c = sk.point(1.0, 2.0, tag="c")
    ab = sk.line(a, b, tag="ab")
    sk.horizontal(ab, tag="level")
    sk.distance(a, b, 4.0, tag="ab-length")
    sk.distance(b, c, 3.0, tag="bc-length")
    sk.distance(a, c, 5.0, tag="ac-length")
    solution = sk.solve()
    assert solution.status is Status.FullyConstrained
    assert check(sk, solution.coordinates).is_clean()
    assert abs(solution.coordinates[(b.id, "x")] - 4.0) <= 1e-9
    assert abs(solution.coordinates[(b.id, "y")]) <= 1e-9
    assert abs(abs(solution.coordinates[(c.id, "y")]) - 3.0) <= 1e-9


def test_every_entity_keyword() -> None:
    sk = Sketch()
    p = sk.point(1.0, 2.0, fixed=True, construction=True, tag="p", payload=b"\x00\xff", id=10)
    q = sk.point(3.0, 2.0)
    assert (p.id, q.id) == (10, 11)  # minted ids continue past a given one
    assert p == p and hash(p) == hash(p) and p != q
    assert {p: 1}[p] == 1
    assert sk.is_fixed(10, "x") and sk.is_fixed(10, "y")
    assert sk.is_construction(10) and not sk.is_construction(11)
    assert sk.tag_of(10) == "p" and sk.payload_of(10) == b"\x00\xff"
    ln = sk.line(p, q, construction=True, tag="l", payload=b"l", id=20)
    c = sk.circle(p, 1.5, fixed=True, tag="c", id=30)
    r = sk.point(3.0, 4.0)
    arc = sk.arc(q, p, r, tag="a", id=40)
    assert arc.id == 40 and sk.tag_of(40) == "a"
    assert sk.is_fixed(c.id, "r") and not sk.is_fixed(c.id, "x") and ln.id == 20
    sk.set_fixed(11, "y", True)
    sk.set_fixed(11, "y", False)
    assert not sk.is_fixed(11, "y")
    sk.set_construction(11, True)
    assert sk.is_construction(11)
    sk.tag(11, "q")
    sk.payload(11, b"q")
    assert sk.tag_of(11) == "q" and sk.payload_of(11) == b"q"
    before = sk.revision
    sk.remove(c.id)
    assert sk.revision > before


def test_every_constraint_method_and_removal() -> None:
    sk = Sketch()
    o = sk.point(0.0, 0.0, fixed=True)
    a = sk.point(4.0, 0.2)
    b = sk.point(0.1, 3.0)
    c = sk.point(2.0, 1.0)
    la = sk.line(o, a)
    lb = sk.line(o, b)
    lc = sk.line(a, b)
    ci = sk.circle(c, 1.0)
    cj = sk.circle(a, 2.0)
    arc = sk.arc(o, a, b)
    made = [
        sk.coincident(c, b, tag="t0", payload=b"x"),
        sk.point_on_line(c, la),
        sk.point_on_circle(a, ci),
        sk.horizontal(la),
        sk.vertical(o, b),
        sk.parallel(la, lc),
        sk.perpendicular(la, lb),
        sk.equal_length(la, lb),
        sk.equal_radius(ci, cj),
        sk.tangent_line_circle(la, ci),
        sk.tangent_line_circle(lb, ci, sense=Sense.Negative),
        sk.tangent_circles(ci, cj),
        sk.tangent_circles(ci, cj, external=True),
        sk.tangent_circles(ci, arc, sense=Sense.Positive),
        sk.midpoint(c, lc),
        sk.symmetric(a, b, lc),
        sk.fix(c, 2.0, 1.0),
        sk.distance(o, a, 4.0),
        sk.point_line_distance(c, la, 1.0),
        sk.radius(ci, 1.0),
        sk.radius(arc, 4.0),
        sk.angle(la, lb, 1.2),
        sk.horizontal(o, a, id=900, tag="h"),
    ]
    assert made[-1] == 900
    assert len(set(made)) == len(made)
    assert sk.constraint_tag_of(made[0]) == "t0"
    assert sk.constraint_payload_of(made[0]) == b"x"
    assert sk.constraint_tag_of(900) == "h"
    sk.tag_constraint(made[1], "on")
    sk.payload_constraint(made[1], b"p")
    assert sk.constraint_tag_of(made[1]) == "on" and sk.constraint_payload_of(made[1]) == b"p"
    sk.remove_constraint(made[1])
    with pytest.raises(errors.UnknownConstraint):
        sk.remove_constraint(made[1])
    with pytest.raises(errors.InUse):
        sk.remove(o.id)
    gone = sk.remove_with_constraints(ci.id)
    assert gone.entities == [ci.id]
    assert gone.constraints == sorted(gone.constraints) and gone.constraints
    with pytest.raises(TypeError):
        sk.horizontal(la, o)
    with pytest.raises(TypeError):
        sk.horizontal(o)
    with pytest.raises(TypeError):
        sk.tangent_circles(cj, arc, external=True, sense=Sense.Positive)
    with pytest.raises(TypeError):
        sk.tangent_line_circle(la, cj, sense="negative")  # type: ignore[arg-type]
    # A refused builder leaves no half-built constraint behind.
    count = len(sk.to_json())
    with pytest.raises(errors.DuplicateConstraintId):
        sk.distance(o, a, 4.0, id=900, tag="again")
    assert len(sk.to_json()) == count


def test_each_model_error_subclass_is_raised_once() -> None:
    sk = Sketch()
    p = sk.point(0.0, 0.0, id=5)
    q = sk.point(1.0, 0.0, id=6, fixed=True)
    ln = sk.line(p, q, id=7)
    other = Sketch()
    stranger = other.point(0.0, 0.0, id=7)  # id 7 is a line in `sk`

    with pytest.raises(errors.UnknownId) as e1:
        sk.tag(999, "x")
    assert e1.value.id == 999
    with pytest.raises(errors.WrongKind) as e2:
        sk.circle(stranger, 1.0)
    assert (e2.value.id, e2.value.expected, e2.value.found) == (7, "point", "line")
    with pytest.raises(errors.FixedPoint) as e3:
        sk.drag(q.id)
    assert e3.value.id == 6
    with pytest.raises(errors.NonFinite) as e4:
        sk.point(math.nan, 0.0)
    assert e4.value.what and math.isnan(e4.value.value)
    with pytest.raises(errors.DimensionOutOfRange) as e5:
        sk.circle(p, -1.0)
    assert e5.value.value == -1.0 and e5.value.wanted
    with pytest.raises(errors.DuplicateId) as e6:
        sk.point(2.0, 0.0, id=5)
    assert e6.value.id == 5
    with pytest.raises(errors.RepeatedEntity) as e7:
        sk.line(p, p)
    assert e7.value.id == 5
    with pytest.raises(errors.UnknownConstraint) as e8:
        sk.remove_constraint(77)
    assert e8.value.id == 77
    sk.distance(p, q, 1.0, id=1)
    with pytest.raises(errors.DuplicateConstraintId) as e9:
        sk.distance(p, q, 1.0, id=1)
    assert e9.value.id == 1
    with pytest.raises(errors.NoSuchParameter) as e10:
        sk.set_fixed(ln.id, "x", True)
    assert (e10.value.id, e10.value.coordinate) == (7, "x")
    with pytest.raises(errors.InUse) as e11:
        sk.remove(p.id)
    assert e11.value.id == 5 and 7 in e11.value.entities and 1 in e11.value.constraints

    solution = sk.solve()
    sk.tag(5, "moved")
    with pytest.raises(errors.Stale) as e12:
        sk.apply(solution)
    assert e12.value.solved < e12.value.current
    # Two sketches at one revision with other contents: ForeignSolution.
    a = Sketch()
    a.point(0.0, 0.0)
    b = Sketch()
    b.point(5.0, 5.0)
    assert a.revision == b.revision
    with pytest.raises(errors.ForeignSolution):
        a.apply(b.solve())

    # An incomplete or fixed-moving solution is not constructible from
    # Python (a Solution is only ever made by solving), so those two variants
    # are held by the Rust doctests; here they are only importable.
    assert issubclass(errors.IncompleteSolution, ModelError)
    assert issubclass(errors.FixedMoved, ModelError)

    ids = Sketch()
    ids.point(0.0, 0.0, id=2**32 - 1)
    with pytest.raises(errors.IdsExhausted):
        ids.point(1.0, 1.0)


def test_loading_refusals_are_model_errors() -> None:
    def load_text(**form: Any) -> Sketch:
        base: dict[str, Any] = {"format": "laman.sketch", "version": 1, "entities": [], "constraints": []}
        return Sketch.from_json(json.dumps({**base, **form}))

    with pytest.raises(errors.FormatVersion) as v:
        load_text(version=99)
    assert v.value.found == 99 and v.value.supported >= 1
    with pytest.raises(errors.WrongFormat) as f:
        load_text(format="other")
    assert f.value.found == "other"
    point = {"id": 0, "kind": "point", "x": 0.0, "y": 0.0}
    with pytest.raises(errors.EntityError) as ent:
        load_text(entities=[{**point, "payload": "!!"}])
    assert ent.value.id == 0
    assert isinstance(ent.value.source, errors.InvalidPayload)
    with pytest.raises(errors.ConstraintError) as con:
        load_text(entities=[point], constraints=[{"id": 3, "kind": "distance", "a": 0, "b": 9, "value": 1.0}])
    assert con.value.id == 3
    assert isinstance(con.value.source, errors.UnknownId)
    with pytest.raises(ValueError) as plain:
        Sketch.from_json("{")
    assert not isinstance(plain.value, ModelError)


def test_the_subclasses_hang_off_model_error() -> None:
    for name in errors.__all__:
        cls = getattr(errors, name)
        if name.startswith("Profile"):  # a family of its own: tests/test_profile.py
            assert issubclass(cls, errors.ProfileError) and issubclass(cls, ValueError)
            continue
        assert issubclass(cls, ModelError) and issubclass(cls, ValueError)
    assert getattr(ModelError, "Stale") is errors.Stale
    assert getattr(ModelError, "Entity") is errors.EntityError
    assert getattr(ModelError, "Constraint") is errors.ConstraintError
