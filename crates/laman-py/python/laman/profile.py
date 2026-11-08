"""Profiles: the closed loops of a sketch's edges, and the holes inside them.

The value classes mirror `laman::model::profile` field for field, ids as
`int`. A `Profiles` holds the Rust value, so a check and a digest read the
same bits extraction wrote; one built by hand is a value like any other. A
negative or oversized id raises `OverflowError`, and a float crosses
unchanged.

>>> from laman.profile import Loop, Profile, Profiles, Traversal
>>> outer = Loop([Traversal(0, False)], 12.0)
>>> found = Profiles(profiles=[Profile(outer, [], 12.0)])
>>> found.profiles[0].net_area
12.0
>>> Profiles() == Profiles(dangling=[])
True
"""

from ._laman import Branched, Loop, Profile, Profiles, Traversal, Unjoined

__all__ = ["Branched", "Loop", "Profile", "Profiles", "Traversal", "Unjoined"]
