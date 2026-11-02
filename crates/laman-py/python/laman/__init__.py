"""Laman: a deterministic 2D geometric constraint solver. A thin layer over the Rust crate.

A `Sketch` is loaded from its JSON form and solved, applied or dragged; the
answer is a `Solution` with its status and diagnostics. A refused operation
raises a `ModelError`. `laman.check` re-evaluates every
constraint independently of the solver; `laman.debug` holds tooling (digests)
outside the API's stability promise. The generic rigidity count of bar
frameworks (Laman's theorem) is `rigidity`.
"""

from ._errors import ModelError
from ._laman import (
    ConstraintRef,
    Coordinates,
    Degenerate,
    Diagnostics,
    DragSession,
    EntityFreedom,
    EntityRef,
    Flip,
    FlipSubject,
    Motion,
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
    "ConstraintRef",
    "Coordinates",
    "Degenerate",
    "Diagnostics",
    "DragSession",
    "EntityFreedom",
    "EntityRef",
    "Flip",
    "FlipSubject",
    "ModelError",
    "Motion",
    "Rigidity",
    "RigidityError",
    "Sketch",
    "Solution",
    "SolverOptions",
    "Status",
    "rigidity",
]
