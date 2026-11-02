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
"""

from ._laman import (
    CheckDegenerate as Degenerate,
    CheckMissing as Missing,
    CheckReport as Report,
    CheckViolation as Violation,
    check_sketch as check,
)

__all__ = ["Degenerate", "Missing", "Report", "Violation", "check"]
