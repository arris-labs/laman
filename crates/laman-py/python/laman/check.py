"""The independent checker: every constraint re-evaluated from coordinates.

It shares no code with the solver's residuals, so a solve is judged by a
second reading of the sketch.

>>> import laman
>>> from laman.check import check
>>> sk = laman.Sketch()
>>> a = sk.point(0.0, 0.0, fixed=True)
>>> b = sk.point(3.0, 0.0)
>>> _ = sk.distance(a, b, 5.0)
>>> check(sk, sk.coordinates()).is_clean()
False
>>> check(sk, sk.solve().coordinates).is_clean()
True

`check_profiles` does the same for a `Profiles` value, the one extraction
returned or one built by hand; the 4 by 3 rectangle's area is 12:

>>> from laman.check import check_profiles
>>> from laman.profile import Loop, Profile, Profiles, profiles
>>> rect = laman.Sketch()
>>> corners = [rect.point(x, y) for x, y in [(0.0, 0.0), (4.0, 0.0), (4.0, 3.0), (0.0, 3.0)]]
>>> _ = [rect.line(corners[i], corners[(i + 1) % 4]) for i in range(4)]
>>> found = profiles(rect, rect.coordinates())
>>> check_profiles(rect, rect.coordinates(), found).is_clean()
True
>>> outer = found.profiles[0].outer
>>> wrong = Profiles(profiles=[Profile(Loop(outer.edges, 13.0, outer.joins), [], 12.0)])
>>> [(v.kind, v.measured, v.reported) for v in check_profiles(rect, rect.coordinates(), wrong).violations]
[('area', 12.0, 13.0)]
"""

from ._laman import (
    CheckDegenerate as Degenerate,
    CheckMissing as Missing,
    CheckProfileReport as ProfileReport,
    CheckProfileViolation as ProfileViolation,
    CheckReport as Report,
    CheckViolation as Violation,
    check_profiles,
    check_sketch as check,
)

__all__ = [
    "Degenerate",
    "Missing",
    "ProfileReport",
    "ProfileViolation",
    "Report",
    "Violation",
    "check",
    "check_profiles",
]
