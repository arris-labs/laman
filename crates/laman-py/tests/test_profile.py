"""`laman.profile`: the value classes round-trip through the Rust value, and
`profiles` reads them from coordinates."""

import math
import struct

import pytest

import laman
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
