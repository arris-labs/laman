"""`f64`'s extremes from Python: a square of side 1e300 or 1e-300 gives
profiles that reach Python bit for bit, the check is clean on them, and a
reversed twin still raises `orientation` (ADR-0013 D3)."""

import pytest

import laman
from laman.check import check_profiles
from laman.profile import Loop, Profile, Profiles, Traversal, profiles

F64_MAX = 1.7976931348623157e308
SMALLEST_SUBNORMAL = 5e-324


def square(side: float) -> laman.Sketch:
    sk = laman.Sketch()
    corners = [sk.point(x, y) for x, y in [(0.0, 0.0), (side, 0.0), (side, side), (0.0, side)]]
    for i in range(4):
        sk.line(corners[i], corners[(i + 1) % 4])
    return sk


def reversed_twin(outer: Loop) -> Profiles:
    turned = [Traversal(t.entity, not t.reversed) for t in reversed(outer.edges)]
    return Profiles(profiles=[Profile(Loop(turned, -outer.signed_area, outer.joins), [], -outer.signed_area)])


def test_a_square_of_side_1e300_saturates_to_f64_max() -> None:
    sk = square(1e300)
    values = sk.coordinates()
    found = profiles(sk, values)
    (p,) = found.profiles
    assert p.outer.signed_area == F64_MAX
    assert p.net_area == F64_MAX
    assert p.holes == []
    report = check_profiles(sk, values, found)
    assert report.is_clean(), report.violations
    kinds = [v.kind for v in check_profiles(sk, values, reversed_twin(p.outer)).violations]
    assert "orientation" in kinds


def test_a_square_of_side_1e_300_underflows_to_zero_or_a_subnormal() -> None:
    sk = square(1e-300)
    values = sk.coordinates()
    found = profiles(sk, values)
    (p,) = found.profiles
    assert 0.0 <= p.outer.signed_area <= SMALLEST_SUBNORMAL
    assert 0.0 <= p.net_area <= SMALLEST_SUBNORMAL
    report = check_profiles(sk, values, found)
    assert report.is_clean(), report.violations
    # The measure underflows to 0: its sign is undecided (ADR-0013 D2, "within
    # the allowance of 0"), so a reversed twin is not an orientation
    # violation here. The plan's claim that it is was wrong (Settled, step 7).
    assert check_profiles(sk, values, reversed_twin(p.outer)).violations == []


def test_a_reversed_square_of_side_1e_10_is_still_an_orientation_violation() -> None:
    # Where the sign is decided again: at 1e-50 and 1e-100 the area is still
    # inside the allowance of 0 and the twin passes.
    sk = square(1e-10)
    values = sk.coordinates()
    found = profiles(sk, values)
    (p,) = found.profiles
    assert p.outer.signed_area == pytest.approx(1e-20, rel=1e-9)
    assert check_profiles(sk, values, found).is_clean()
    kinds = [v.kind for v in check_profiles(sk, values, reversed_twin(p.outer)).violations]
    assert "orientation" in kinds


@pytest.mark.parametrize("side", [1e300, 1e-300])
def test_the_found_profile_is_repeatable(side: float) -> None:
    sk = square(side)
    values = sk.coordinates()
    assert profiles(sk, values) == profiles(sk, values)
