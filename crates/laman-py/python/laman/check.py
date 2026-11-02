"""The independent checker: every constraint re-evaluated from coordinates.

It shares no code with the solver's residuals, so a solve is judged by a
second reading of the sketch.
"""

from ._laman import (
    CheckDegenerate as Degenerate,
    CheckMissing as Missing,
    CheckReport as Report,
    CheckViolation as Violation,
    check_sketch as check,
)

__all__ = ["Degenerate", "Missing", "Report", "Violation", "check"]
