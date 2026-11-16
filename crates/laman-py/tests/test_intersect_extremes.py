"""`f64`'s extremes from Python: two lines crossing at the centre of a square
of side 1e300 give the meeting at exactly half the side, parameters 0.5, and
the check is clean; at 1e-300 the sketch is below the length floor, so
nothing is reported and nothing missed (ADR-0003, ADR-0018)."""

import pytest

import laman
from laman.check import check_intersections
from laman.intersect import intersections


def crossed(side: float) -> laman.Sketch:
    sk = laman.Sketch()
    p = [sk.point(x, y) for x, y in [(0.0, 0.0), (side, side), (0.0, side), (side, 0.0)]]
    sk.line(p[0], p[1])
    sk.line(p[2], p[3])
    return sk


def test_a_square_of_side_1e300_crosses_at_its_centre_bit_for_bit() -> None:
    sk = crossed(1e300)
    values = sk.coordinates()
    found = intersections(sk, values)
    (pair,) = found.pairs
    (m,) = pair.meetings
    assert m.kind.name == "crossing"
    assert m.at == (5e299, 5e299)
    assert (m.on_first, m.on_second) == (0.5, 0.5)
    report = check_intersections(sk, values, found)
    assert report.is_clean(), report.violations
    assert found == intersections(sk, values)


def test_a_square_of_side_1e_300_reports_nothing_and_misses_nothing() -> None:
    sk = crossed(1e-300)
    values = sk.coordinates()
    found = intersections(sk, values)
    assert found.pairs == []
    assert check_intersections(sk, values, found).is_clean()
