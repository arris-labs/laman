"""Profiles: the closed loops of a sketch's edges, and the holes inside them.

The value classes mirror `laman::model::profile` field for field, ids as
`int`. A `Profiles` holds the Rust value, so a check and a digest read the
same bits extraction wrote; one built by hand is a value like any other. A
negative or oversized id raises `OverflowError`, and a float crosses
unchanged. `profiles` raises a `ProfileError` naming the entity whose
coordinate is missing or not finite.

`profiles(sketch, coordinates)` reads them from any coordinates, the sketch's
own or a solution's. A 4 by 3 rectangle is one profile of area 12, counter-
clockwise:

>>> import laman
>>> from laman.profile import profiles
>>> sk = laman.Sketch()
>>> corners = [sk.point(x, y) for x, y in [(0.0, 0.0), (4.0, 0.0), (4.0, 3.0), (0.0, 3.0)]]
>>> _ = [sk.line(corners[i], corners[(i + 1) % 4]) for i in range(4)]
>>> (found,) = profiles(sk, sk.coordinates()).profiles
>>> found.net_area, found.holes
(12.0, [])

A value can be built by hand, for `laman.check.check_profiles` to re-measure:

>>> from laman.profile import Loop, Profile, Profiles, Traversal
>>> outer = Loop([Traversal(0, False)], 12.0)
>>> by_hand = Profiles(profiles=[Profile(outer, [], 12.0)])
>>> by_hand.profiles[0].net_area
12.0
>>> Profiles() == Profiles(dangling=[])
True
"""

from ._errors import ProfileError
from ._laman import Branched, Loop, Profile, Profiles, Traversal, Unjoined, profiles

__all__ = [
    "Branched",
    "Loop",
    "Profile",
    "ProfileError",
    "Profiles",
    "Traversal",
    "Unjoined",
    "profiles",
]
