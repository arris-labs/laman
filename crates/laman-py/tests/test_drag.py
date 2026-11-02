"""Fixture 4 from Python: apply, and the drag session of ten steps under both
algorithms, against drag.json's listed coordinates and digests.txt."""

import json
import struct

import pytest

from laman import Coordinates, ModelError, Sketch, SolverOptions, Status
from laman._errors import FixedPoint, Stale
from laman.check import check
from laman.debug import digest_drag_step
from test_c1 import ALGORITHMS, C1, DIGESTS, Fixture, load

DRAG = json.loads((C1 / "slotted-plate-drag" / "drag.json").read_text())
TARGETS: list[tuple[float, float]] = [(t[0], t[1]) for t in DRAG["targets"]]
Q = 21  # the stray free point no constraint names


def bits(x: float) -> bytes:
    return struct.pack("<d", x)


def listed(step: int) -> dict[tuple[int, str], float]:
    after = DRAG["after"][step]
    assert after["step"] == step + 1
    return {(c["id"], c["coordinate"]): float(c["value"]) for c in after["coordinates"]}


@pytest.mark.parametrize("label", ALGORITHMS)
def test_the_ten_drag_steps(label: str) -> None:
    fx = load("slotted-plate-drag")
    options = SolverOptions(algorithm=ALGORITHMS[label])
    before = fx.sketch.coordinates()
    session = fx.sketch.drag(DRAG["dragged"], options)
    assert len(TARGETS) == 10
    for k, (x, y) in enumerate(TARGETS):
        step = session.to(x, y)
        assert step.status is Status.UnderConstrained
        assert step.diagnostics.dof == 3
        for key, want in listed(k).items():
            assert abs(step.coordinates[key] - want) <= fx.tolerance, (k + 1, key)
        # P3 stays; Q keeps its bits; no flip anywhere along the drag.
        assert abs(step.coordinates[(3, "x")] - 0.0) <= fx.tolerance
        assert abs(step.coordinates[(3, "y")] - 60.0) <= fx.tolerance
        for c in ("x", "y"):
            assert bits(step.coordinates[(Q, c)]) == bits(before[(Q, c)])
        assert not step.diagnostics.flips
        report = check(fx.sketch, step.coordinates)
        assert report.is_clean(), report
        digest = digest_drag_step(fx.sketch, k + 1, (x, y), step, report)
        assert digest == DIGESTS[f"drag-{label}-step-{k + 1:02}"]
    # The session worked on a copy.
    assert fx.sketch.coordinates() == before


def test_finish_then_apply_and_a_second_apply_is_stale() -> None:
    fx = load("slotted-plate-drag")
    session = fx.sketch.drag(DRAG["dragged"])
    for x, y in TARGETS:
        last = session.to(x, y)
    done = session.finish()
    assert done.coordinates == last.coordinates
    revision = fx.sketch.revision
    fx.sketch.apply(done)
    assert fx.sketch.revision > revision
    assert fx.sketch.coordinates() == done.coordinates
    assert abs(fx.sketch.coordinates()[(2, "x")] - 150.0) <= fx.tolerance
    with pytest.raises(ModelError.Stale) as caught:  # type: ignore[attr-defined]
        fx.sketch.apply(done)
    assert isinstance(caught.value, Stale)
    assert isinstance(caught.value, ValueError)
    assert caught.value.solved == revision
    assert caught.value.current == fx.sketch.revision
    # Solving after applying warm-starts from the applied coordinates.
    again = fx.sketch.solve()
    assert again.status is Status.UnderConstrained
    assert abs(again.coordinates[(2, "x")] - 150.0) <= fx.tolerance


def test_apply_a_static_solution() -> None:
    fx = load("belt")
    solution = fx.sketch.solve()
    fx.sketch.apply(solution)
    assert fx.sketch.coordinates() == solution.coordinates
    with pytest.raises(Stale):
        fx.sketch.apply(solution)


def test_a_finished_session_refuses() -> None:
    fx = load("slotted-plate-drag")
    session = fx.sketch.drag(DRAG["dragged"])
    session.to(*TARGETS[0])
    session.finish()
    with pytest.raises(ValueError, match="finished"):
        session.to(*TARGETS[1])
    with pytest.raises(ValueError, match="finished"):
        session.finish()


def test_a_drag_that_cannot_open_is_a_model_error() -> None:
    fx: Fixture = load("slotted-plate-drag")
    with pytest.raises(FixedPoint) as caught:
        fx.sketch.drag(0)  # the origin, fixed in both coordinates
    assert caught.value.id == 0
    with pytest.raises(ModelError):
        fx.sketch.drag(10_000)
    # A non-finite target is a frame with status NotConverged, not an error.
    session = fx.sketch.drag(DRAG["dragged"])
    bad = session.to(float("nan"), 0.0)
    assert bad.status is Status.NotConverged


def test_the_dragged_sketch_is_untouched_until_apply() -> None:
    sketch = Sketch.from_json(load("slotted-plate-drag").text)
    revision = sketch.revision
    sketch.drag(DRAG["dragged"]).to(*TARGETS[0])
    assert sketch.revision == revision
    assert isinstance(sketch.coordinates(), Coordinates)
