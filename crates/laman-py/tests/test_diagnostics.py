"""Every diagnostic from Python: redundant and conflict sets by tag, per-entity
DoF and free directions, degenerate entities, tags and payloads."""

import json
import math

import pytest

from laman import ConstraintRef, Sketch, SolverOptions, Status
from test_c1 import ALGORITHMS, load

TWINS = {
    "sss-triangle-redundant": ["right-angle"],
    "belt-redundant": ["same-belt"],
    "slotted-plate-redundant": ["level"],
}
CONFLICTING = {
    "sss-triangle-conflicting": [["base", "right", "apex"]],
    "belt-conflicting": [
        ["r1", "r2", "span", "top-on-c1", "top-on-c2", "top-tangent-c1", "top-tangent-c2", "top-length"]
    ],
    "slotted-plate-conflicting": [["same-ends", "slot-r", "slot-r2"]],
}


@pytest.mark.parametrize("label", ALGORITHMS)
@pytest.mark.parametrize("name", [*TWINS, *CONFLICTING, "sss-triangle", "belt", "slotted-plate"])
def test_redundant_and_conflict_sets_by_tag(name: str, label: str) -> None:
    fx = load(name)
    d = fx.sketch.solve(SolverOptions(algorithm=ALGORITHMS[label])).diagnostics
    assert [c.tag for c in d.redundant] == fx.expected["redundant"] == TWINS.get(name, [])
    # The listed sets are in the closed form's order; the solver's own order
    # within a set is by source, so compare as sets.
    got = [sorted(c.tag or "" for c in s) for s in d.conflicts]
    assert got == [sorted(s) for s in CONFLICTING.get(name, [])] == [
        sorted(s) for s in fx.expected["conflicts"]
    ]
    assert not d.degenerate
    assert not d.ill_conditioned
    assert not d.flips
    for c in [*d.redundant, *(c for s in d.conflicts for c in s)]:
        assert isinstance(c, ConstraintRef)
        assert fx.sketch.constraint_tag_of(c.id) == c.tag


def test_per_entity_freedom_and_free_directions_of_the_drag_fixture() -> None:
    fx = load("slotted-plate-drag")
    d = fx.sketch.solve().diagnostics
    assert d.dof == 3
    by_id = {f.entity.id: f for f in d.entities}
    assert [f.entity.id for f in d.entities] == sorted(by_id)
    assert by_id[21].dof == 2  # Q, which no constraint names
    assert by_id[21].entity.kind == "point"
    assert by_id[0].dof == 0 and by_id[0].entity.tag == "origin"
    assert by_id[3].dof == 0  # P3 is held
    plate = [f for i, f in by_id.items() if i not in (0, 3, 7, 21)]
    assert plate and all(f.dof == 1 for f in plate)
    assert by_id[10].entity.tag == "axis" and by_id[10].entity.kind == "line"

    # An orthonormal basis of 3 motions: Q's two, and the plate's width.
    dirs = d.free_directions
    assert len(dirs) == 3
    for i, u in enumerate(dirs):
        for j, v in enumerate(dirs):
            dot = sum(a.amount * b.amount for a in u for b in v if (a.entity, a.coordinate) == (b.entity, b.coordinate))
            assert abs(dot - (1.0 if i == j else 0.0)) <= 1e-9
    q = [u for u in dirs if all(m.entity == 21 for m in u)]
    assert len(q) == 2
    assert {m.coordinate for u in q for m in u} == {"x", "y"}
    assert all(m.amount != 0.0 and math.isfinite(m.amount) for u in dirs for m in u)


def test_a_collapsed_line_is_degenerate() -> None:
    text = json.dumps(
        {
            "format": "laman.sketch",
            "version": 1,
            "entities": [
                {"id": 0, "kind": "point", "x": 0.0, "y": 0.0, "fixed": ["x", "y"]},
                {"id": 1, "kind": "point", "x": 0.0, "y": 0.0, "fixed": ["x", "y"]},
                {"id": 2, "kind": "line", "a": 0, "b": 1, "tag": "stub"},
            ],
            "constraints": [{"id": 0, "kind": "horizontal", "line": 2, "tag": "flat"}],
        }
    )
    d = Sketch.from_json(text).solve().diagnostics
    assert [g.entity.id for g in d.degenerate] == [2]
    g = d.degenerate[0]
    assert g.collapse == "zero_length_line"
    assert g.entity.tag == "stub" and g.entity.kind == "line"
    assert g.size <= 1e-9
    assert [e.tag for e in g.equations if isinstance(e, ConstraintRef)] == ["flat"]


def test_tags_and_payloads_round_trip() -> None:
    text = json.dumps(
        {
            "format": "laman.sketch",
            "version": 1,
            "entities": [
                {"id": 0, "kind": "point", "x": 0.0, "y": 0.0, "fixed": ["x", "y"], "tag": "o", "payload": "AQID"},
                {"id": 1, "kind": "point", "x": 3.0, "y": 4.0},
            ],
            "constraints": [
                {"id": 7, "kind": "distance", "a": 0, "b": 1, "value": 5.0, "tag": "d", "payload": "/w=="}
            ],
        }
    )
    sk = Sketch.from_json(text)
    assert sk.tag_of(0) == "o" and sk.tag_of(1) is None
    assert sk.payload_of(0) == b"\x01\x02\x03" and sk.payload_of(1) is None
    assert sk.constraint_tag_of(7) == "d"
    assert sk.constraint_payload_of(7) == b"\xff"
    again = Sketch.from_json(sk.to_json())
    assert again.payload_of(0) == b"\x01\x02\x03"
    assert again.constraint_payload_of(7) == b"\xff"
    d = again.solve().diagnostics
    assert d.entities[0].entity.tag == "o"
    assert again.solve().status is Status.UnderConstrained
