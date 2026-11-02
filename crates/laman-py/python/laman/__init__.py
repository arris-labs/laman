"""Laman: a deterministic 2D geometric constraint solver. A thin layer over the Rust crate.

A `Sketch` is loaded from its JSON form and solved; the answer is a
`Solution` with its status and diagnostics. `laman.check` re-evaluates every
constraint independently of the solver; `laman.debug` holds tooling (digests)
outside the API's stability promise. The generic rigidity count of bar
frameworks (Laman's theorem) is `rigidity`.
"""

from ._laman import (
    Coordinates,
    Diagnostics,
    Rigidity,
    RigidityError,
    Sketch,
    Solution,
    SolverOptions,
    rigidity,
)
from ._types import Algorithm, Status

__all__ = [
    "Algorithm",
    "Coordinates",
    "Diagnostics",
    "Rigidity",
    "RigidityError",
    "Sketch",
    "Solution",
    "SolverOptions",
    "Status",
    "rigidity",
]
