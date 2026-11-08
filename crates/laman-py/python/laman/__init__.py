"""Laman: a deterministic 2D geometric constraint solver. A thin layer over the Rust crate.

A `Sketch` is built by hand (`point`, `line`, `distance`, `horizontal`, ...) or
loaded from its JSON form, then solved, applied or dragged; the answer is a
`Solution` with its status and diagnostics. A refused operation raises a
`ModelError`. `laman.profile` reads the closed loops and holes of a sketch's
edges (a refusal is a `ProfileError`). `laman.check` re-evaluates every constraint independently of the
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
>>> frame.diagnostics.bisection is None
True
>>> _ = session.finish()

A frame that would flip a branch (or carry an arc's sweep too far) is halved
toward the last target, and says so:

>>> o, e, top = sk.point(0.0, 0.0, fixed=True), sk.point(1.0, 0.0, fixed=True), sk.point(0.0, 1.0, fixed=True)
>>> tip = sk.point(3.0, 1.0)
>>> _ = sk.parallel(sk.line(o, e), sk.line(top, tip))
>>> arm = sk.drag(tip)
>>> frame = arm.to(-1.0, 1.0)  # the tip may not cross x = 0
>>> frame.diagnostics.bisection
Bisection(reached=(1, 1), halvings=1)
>>> _ = arm.finish()

A sketch loaded from JSON hands out its handles, and `laman.check` re-reads
a solution independently of the solver:

>>> from laman.check import check
>>> loaded = laman.Sketch.from_json(sk.to_json())
>>> loaded.point_handle(b.id) == b
True
>>> check(loaded, loaded.solve().coordinates).is_clean()
True
"""

from ._errors import ModelError, ProfileError
from ._laman import (
    Arc,
    Bisection,
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
    "Bisection",
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
    "ProfileError",
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
