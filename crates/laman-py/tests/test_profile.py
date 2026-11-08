"""`laman.profile`: the value classes round-trip through the Rust value, and
`profiles` reads them from coordinates."""

import json
import math
import struct
from pathlib import Path

import pytest

import laman
from laman import Coordinates, Sketch
from laman.check import check, check_profiles
from laman.debug import digest_profiles, digest_solved_profiles, slot_grid
from laman.profile import (
    Branched,
    Loop,
    Profile,
    ProfileError,
    Profiles,
    Traversal,
    Unjoined,
    profiles,
)

F64_MAX = 1.7976931348623157e308
SMALLEST_SUBNORMAL = 5e-324
AWKWARD = 7200 - 400 * math.pi


def bits(x: float) -> bytes:
    return struct.pack(">d", x)


def test_empty_profiles_is_the_default() -> None:
    empty = Profiles()
    assert empty.profiles == [] and empty.open_chains == [] and empty.dangling == []
    assert empty.branched == [] and empty.degenerate == [] and empty.unjoined == []
    assert empty == Profiles(profiles=[], unjoined=[])


@pytest.mark.parametrize("x", [F64_MAX, -F64_MAX, -0.0, 0.0, SMALLEST_SUBNORMAL, AWKWARD])
def test_floats_cross_unchanged(x: float) -> None:
    outer = Loop([Traversal(3, True), Traversal(4, False)], x, [9])
    hole = Loop([Traversal(7, False)], -x)
    built = Profiles(
        profiles=[Profile(outer, [hole], x)],
        open_chains=[[1, 2], [5]],
        dangling=[6],
        branched=[Branched([10, 11, 12], [2])],
        degenerate=[8],
        unjoined=[Unjoined(2, 9)],
    )
    (p,) = built.profiles
    assert bits(p.net_area) == bits(x)
    assert bits(p.outer.signed_area) == bits(x)
    assert bits(p.holes[0].signed_area) == bits(-x)
    assert p.outer.edges == [Traversal(3, True), Traversal(4, False)]
    assert p.outer.joins == [9] and p.holes[0].joins == []
    assert built.open_chains == [[1, 2], [5]] and built.dangling == [6]
    assert built.branched == [Branched([10, 11, 12], [2])]
    assert built.degenerate == [8] and built.unjoined == [Unjoined(2, 9)]
    again = Profiles(
        profiles=built.profiles,
        open_chains=built.open_chains,
        dangling=built.dangling,
        branched=built.branched,
        degenerate=built.degenerate,
        unjoined=built.unjoined,
    )
    assert again == built
    assert bits(again.profiles[0].net_area) == bits(x)


def test_a_negative_or_oversized_id_raises_overflow() -> None:
    with pytest.raises(OverflowError):
        Traversal(-1, False)
    with pytest.raises(OverflowError):
        Loop([], 1.0, [-2])
    with pytest.raises(OverflowError):
        Profiles(dangling=[2**32])
    with pytest.raises(OverflowError):
        Profiles(open_chains=[[0, -1]])
    with pytest.raises(OverflowError):
        Unjoined(0, -3)


def test_values_have_a_repr() -> None:
    assert repr(Traversal(4, True)) == "Traversal(entity=4, reversed=True)"
    assert repr(Profiles()).startswith("Profiles(profiles=[]")


def rectangle() -> laman.Sketch:
    sk = laman.Sketch()
    corners = [sk.point(x, y) for x, y in [(0.0, 0.0), (4.0, 0.0), (4.0, 3.0), (0.0, 3.0)]]
    for i in range(4):
        sk.line(corners[i], corners[(i + 1) % 4])
    return sk


def test_the_rectangle_by_hand_is_one_profile() -> None:
    sk = rectangle()
    found = profiles(sk, sk.coordinates())
    (p,) = found.profiles
    assert p.net_area == 12.0 and p.outer.signed_area == 12.0 and p.holes == []
    assert found.open_chains == [] and found.dangling == [] and found.unjoined == []


def test_a_removed_coordinate_raises_missing_with_its_entity() -> None:
    sk = rectangle()
    items = [(k, v) for k, v in sk.coordinates().items() if k != (2, "x")]
    with pytest.raises(ProfileError.Missing) as err:
        profiles(sk, laman.Coordinates(items))
    assert err.value.entity == 2
    assert isinstance(err.value, ProfileError) and isinstance(err.value, ValueError)
    assert "2" in str(err.value)


def test_a_nan_raises_non_finite_with_its_entity() -> None:
    sk = rectangle()
    items = [(k, math.nan if k == (1, "y") else v) for k, v in sk.coordinates().items()]
    with pytest.raises(ProfileError.NonFinite) as err:
        profiles(sk, laman.Coordinates(items))
    assert err.value.entity == 1
    assert isinstance(err.value, ProfileError) and isinstance(err.value, ValueError)


def test_profile_error_is_at_the_top_level_and_in_profile() -> None:
    assert laman.ProfileError is ProfileError
    assert not issubclass(ProfileError, laman.ModelError)


def rect_with_found() -> tuple[laman.Sketch, laman.Coordinates, Profiles, Loop]:
    sk = rectangle()
    values = sk.coordinates()
    found = profiles(sk, values)
    return sk, values, found, found.profiles[0].outer


def test_the_found_rectangle_checks_clean() -> None:
    sk, values, found, _ = rect_with_found()
    report = check_profiles(sk, values, found)
    assert report.is_clean() and report.violations == [] and report.missing == []


def test_a_wrong_area_is_one_area_violation() -> None:
    sk, values, _, outer = rect_with_found()
    twin = Profiles(profiles=[Profile(Loop(outer.edges, 13.0, outer.joins), [], 12.0)])
    (v,) = check_profiles(sk, values, twin).violations
    assert (v.kind, v.measured, v.reported) == ("area", 12.0, 13.0)
    assert v.first == outer.edges[0].entity and v.entity is None


def test_a_reversed_outer_is_an_orientation_violation() -> None:
    sk, values, _, outer = rect_with_found()
    turned = [Traversal(t.entity, not t.reversed) for t in reversed(outer.edges)]
    twin = Profiles(profiles=[Profile(Loop(turned, -12.0, outer.joins), [], 12.0)])
    kinds = [v.kind for v in check_profiles(sk, values, twin).violations]
    assert kinds == ["orientation"]


def test_a_stale_edge_id_is_not_an_edge() -> None:
    sk, values, _, outer = rect_with_found()
    stale = [*outer.edges[:3], Traversal(99, False)]
    twin = Profiles(profiles=[Profile(Loop(stale, 12.0, outer.joins), [], 12.0)])
    report = check_profiles(sk, values, twin)
    assert [v.entity for v in report.violations if v.kind == "not_an_edge"] == [99]


def test_a_removed_coordinate_is_missing() -> None:
    sk, values, found, _ = rect_with_found()
    items = [(k, v) for k, v in values.items() if k != (2, "x")]
    report = check_profiles(sk, laman.Coordinates(items), found)
    assert [(m.id, m.coordinate) for m in report.missing] == [(2, "x")]
    assert not report.is_clean()


FIXTURES = Path(__file__).resolve().parents[3] / "tests" / "fixtures" / "c3"


def blessed_digest(name: str) -> str:
    for line in (FIXTURES / "digests.txt").read_text().splitlines():
        key, digest = line.split(" ", 1)
        if key == name:
            return digest
    raise AssertionError(name)


def expected_run(name: str) -> tuple[Sketch, Coordinates]:
    sketch = Sketch.from_json((FIXTURES / name / "sketch.json").read_text())
    expected = json.loads((FIXTURES / name / "expected.json").read_text())
    values = dict(sketch.coordinates().items())
    for c in expected["coordinates"]:
        values[(c["id"], c["coordinate"])] = float(c["value"])
    return sketch, Coordinates(values)


def test_the_rectangle_profile_run_equals_its_blessed_digest() -> None:
    sketch, values = expected_run("rectangle")
    found = profiles(sketch, values)
    report = check_profiles(sketch, values, found)
    assert report.is_clean()
    assert digest_profiles(sketch, values, found, report) == blessed_digest("c3-rectangle")


def test_the_solved_rectangle_equals_its_blessed_digest() -> None:
    sketch, _ = expected_run("rectangle")
    solution = sketch.solve()
    run = check(sketch, solution.coordinates)
    found = profiles(sketch, solution.coordinates)
    report = check_profiles(sketch, solution.coordinates, found)
    assert digest_solved_profiles(sketch, solution, run, found, report) == blessed_digest(
        "c3-solved-lm-rectangle"
    )


def test_the_3x3_slot_grid_equals_its_blessed_digest() -> None:
    sketch, values = slot_grid(3, 3)
    found = profiles(sketch, values)
    report = check_profiles(sketch, values, found)
    assert report.is_clean()
    assert digest_profiles(sketch, values, found, report) == blessed_digest("c3-slot-grid-3x3")


def test_the_largest_slot_grid_has_498_entities() -> None:
    sketch, _ = slot_grid(7, 7)
    assert len(sketch.entities()) == 498


def test_a_slot_grid_refuses_zero_rows() -> None:
    with pytest.raises(ValueError):
        slot_grid(0, 3)
