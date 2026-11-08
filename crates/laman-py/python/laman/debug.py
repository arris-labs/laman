"""Tooling: the digests of a run and the text and SVG dumps. Outside the API's
stability promise.

A digest is the SHA-256 (lower-case hex) over the bits of a run; equal runs
have equal digests on every target. `text`, `svg` and `solution_text` print a
sketch (and a solution's diagnostics) for reading. `digest_profiles` and
`digest_solved_profiles` add the profiles and their check; `slot_grid(rows,
cols)` is the corpus's slotted plate and its expected coordinates.

>>> from laman import Sketch
>>> from laman.check import check
>>> from laman.debug import text
>>> sk = Sketch()
>>> p = sk.point(0.0, 0.0, fixed=True)
>>> q = sk.point(3.0, 4.0)
>>> _ = sk.distance(p, q, 5.0)
>>> out = text(sk, sk.coordinates(), check(sk, sk.coordinates()))
>>> out.endswith("report: clean\\n")
True
"""

from ._laman import (
    digest_drag_step,
    digest_profiles,
    digest_run,
    digest_solution,
    digest_solved_profiles,
    slot_grid,
    solution_text,
    svg,
    text,
)

__all__ = [
    "digest_drag_step",
    "digest_profiles",
    "digest_run",
    "digest_solution",
    "digest_solved_profiles",
    "slot_grid",
    "solution_text",
    "svg",
    "text",
]
