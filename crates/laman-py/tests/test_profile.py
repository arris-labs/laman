"""`laman.profile`: the value classes round-trip through the Rust value."""

import math
import struct

import pytest

from laman.profile import Branched, Loop, Profile, Profiles, Traversal, Unjoined

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
