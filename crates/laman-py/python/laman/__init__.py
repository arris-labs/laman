"""Laman: a deterministic 2D geometric constraint solver. A thin layer over the Rust crate.

A `Sketch` is built by hand (`point`, `line`, `distance`, `horizontal`, ...) or
loaded from its JSON form, then solved, applied or dragged; the answer is a
`Solution` with its status and diagnostics. A refused operation raises a
`ModelError`. `laman.check` re-evaluates every constraint independently of the
solver; `laman.debug` holds tooling (digests, text and SVG dumps) outside the
API's stability promise. The generic rigidity count of bar frameworks (Laman's
theorem) is `rigidity`.

Build a sketch by hand, solve it, apply the answer:

>>> import laman
>>> sk = laman.Sketch()
>>> a = sk.point(0.0, 0.0, fixed=True)
>>> b = sk.point(3.0, 0.5)
>>> _ = sk.distance(a, b, 5.0)
>>> solution = sk.solve()
>>> solution.status
<Status.UnderConstrained: 'UnderConstrained'>
>>> sk.apply(solution)

A drag moves a point (a `Point` handle or its id) through targets, one
frame per `to`; the sketch itself is untouched until `apply`:

>>> session = sk.drag(b)
>>> frame = session.to(0.0, 5.0)
>>> round(frame.coordinates[(b.id, "x")], 9), round(frame.coordinates[(b.id, "y")], 9)
(0.0, 5.0)
>>> _ = session.finish()

A sketch loaded from JSON hands out its handles, and `laman.check` re-reads
a solution independently of the solver:

>>> from laman.check import check
>>> loaded = laman.Sketch.from_json(sk.to_json())
>>> loaded.point_handle(b.id) == b
True
>>> check(loaded, loaded.solve().coordinates).is_clean()
True
"""

from ._errors import ModelError
from ._laman import (
    Arc,
    Circle,
    ConstraintRef,
    Coordinates,
    Degenerate,
    Diagnostics,
    DragSession,
    EntityFreedom,
    EntityRef,
    Flip,
    FlipSubject,
    Line,
    Motion,
    Point,
    Removed,
    Rigidity,
    RigidityError,
    Sketch,
    Solution,
    SolverOptions,
    rigidity,
)
from ._types import Algorithm, Sense, Status

__all__ = [
    "Algorithm",
    "Arc",
    "Circle",
    "ConstraintRef",
    "Coordinates",
    "Degenerate",
    "Diagnostics",
    "DragSession",
    "EntityFreedom",
    "EntityRef",
    "Flip",
    "FlipSubject",
    "Line",
    "ModelError",
    "Motion",
    "Point",
    "Removed",
    "Rigidity",
    "RigidityError",
    "Sense",
    "Sketch",
    "Solution",
    "SolverOptions",
    "Status",
    "rigidity",
]
