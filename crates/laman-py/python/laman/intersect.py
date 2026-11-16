"""Intersections: where the curves of a sketch meet without a shared point.

The value classes mirror `laman::model::intersection` field for field, ids as
`int`. An `Intersections` holds the Rust value, so a check and a digest read
the same bits `intersections` wrote; one built by hand is a value like any
other. A negative or oversized id raises `OverflowError`, and a float crosses
unchanged. `intersections` raises an `IntersectError` naming the entity whose
coordinate is missing or not finite. Nothing is changed or joined: an
intersection is a measurement.

`intersections(sketch, coordinates)` reads them from any coordinates, the
sketch's own or a solution's. Two lines across a square cross once, at its
centre, halfway along each:

>>> import laman
>>> from laman.intersect import intersections
>>> sk = laman.Sketch()
>>> p = [sk.point(x, y) for x, y in [(0.0, 0.0), (4.0, 4.0), (0.0, 4.0), (4.0, 0.0)]]
>>> first, second = sk.line(p[0], p[1]), sk.line(p[2], p[3])
>>> found = intersections(sk, sk.coordinates())
>>> (pair,) = found.pairs
>>> (pair.first, pair.second) == (first.id, second.id)
True
>>> (m,) = pair.meetings
>>> m.kind.name, m.at, round(m.on_first, 12), round(m.on_second, 12)
('crossing', (2.0, 2.0), 0.5, 0.5)

`Intersections.along` reads them from one curve, ordered along it:

>>> [(a.other, round(a.meeting.on_first, 12)) for a in found.along(second.id)] == [(first.id, 0.5)]
True

Lines that share a point meet there by construction, and that is not
reported; a pair of collinear lines overlaps:

>>> sk = laman.Sketch()
>>> q = [sk.point(x, 0.0) for x in (0.0, 3.0, 1.0, 5.0)]
>>> _ = sk.line(q[0], q[1]), sk.line(q[2], q[3])
>>> (shared,) = intersections(sk, sk.coordinates()).pairs[0].meetings
>>> shared.kind.name, shared.kind.to, shared.at
('overlap', (3.0, 0.0), (1.0, 0.0))
"""

from ._errors import IntersectError, IntersectMissing, IntersectNonFinite
from ._laman import (
    Along,
    Intersection,
    Intersections,
    Meeting,
    MeetingKind,
    intersections,
)

__all__ = [
    "Along",
    "IntersectError",
    "IntersectMissing",
    "IntersectNonFinite",
    "Intersection",
    "Intersections",
    "Meeting",
    "MeetingKind",
    "intersections",
]
