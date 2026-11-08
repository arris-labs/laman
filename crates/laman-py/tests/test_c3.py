"""The C3 corpus from Python: every fixture of `tests/fixtures/c3/`, at its
expected coordinates and solved under both algorithms.

Expected profiles are read from `expected.json` (closed form or by
construction); the blessed digests from `digests.txt`, the same file
`crates/laman-debug/tests/determinism.rs` reads.
"""

import json
import math
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import pytest

from laman import Algorithm, Coordinates, Sketch, SolverOptions, Status
from laman.check import check, check_profiles
from laman.debug import digest_profiles, digest_solved_profiles, slot_grid
from laman.profile import Loop, Profiles, profiles

C3 = Path(__file__).resolve().parents[3] / "tests" / "fixtures" / "c3"
NAMES = [
    "rectangle",
    "slotted-plate",
    "slotted-plate-coincident",
    "plate-island",
    "two-loops",
    "circle-alone",
    "open-chain",
    "dangling-edge",
    "shared-edge",
    "unjoined",
]
ALGORITHMS = {"lm": Algorithm.LevenbergMarquardt, "dogleg": Algorithm.DogLeg}
# Its solved coordinates are not compared: the closed-form ones are held at
# the default tolerance by `regression/` (`coincident-contact-drift`).
UNCOMPARED = ["slotted-plate-coincident"]
# An area expectation that names no tolerance, relative to max(1, |value|)
# (laman_debug::fixture::AREA_TOLERANCE; a copy, as test_c1's is).
AREA_TOLERANCE = 1e-9
# The coordinate tolerance of a fixture that names none
# (laman::model::tolerance::LENGTH; test_c1 copies it too).
DEFAULT_TOLERANCE = 1e-9


@dataclass(frozen=True)
class Fixture:
    name: str
    sketch: Sketch
    expected: dict[str, Any]

    def expected_coordinates(self) -> Coordinates:
        values = dict(self.sketch.coordinates().items())
        for c in self.expected["coordinates"]:
            values[(c["id"], c["coordinate"])] = float(c["value"])
        return Coordinates(values)


def load(name: str) -> Fixture:
    text = (C3 / name / "sketch.json").read_text()
    expected = json.loads((C3 / name / "expected.json").read_text())
    return Fixture(name, Sketch.from_json(text), expected)


def blessed() -> dict[str, str]:
    lines = (C3 / "digests.txt").read_text().splitlines()
    return dict(line.split(" ", 1) for line in lines)


DIGESTS = blessed()


def area_holds(want: dict[str, float], got: float) -> bool:
    tolerance = want.get("tolerance", AREA_TOLERANCE)
    return abs(got - want["value"]) <= tolerance * max(1.0, abs(want["value"]))


def assert_loop(label: str, want: dict[str, Any], got: Loop) -> None:
    assert [(t.entity, t.reversed) for t in got.edges] == [
        (t["entity"], t["reversed"]) for t in want["edges"]
    ], label
    assert area_holds(want["signed_area"], got.signed_area), (label, got.signed_area)
    assert got.joins == want["joins"], label


def assert_profiles(name: str, want: dict[str, Any], got: Profiles) -> None:
    assert len(got.profiles) == len(want["profiles"]), name
    for k, (w, g) in enumerate(zip(want["profiles"], got.profiles)):
        assert_loop(f"{name} profile {k} outer", w["outer"], g.outer)
        assert len(g.holes) == len(w["holes"]), (name, k)
        for h, (wh, gh) in enumerate(zip(w["holes"], g.holes)):
            assert_loop(f"{name} profile {k} hole {h}", wh, gh)
        assert area_holds(w["net_area"], g.net_area), (name, k, g.net_area)
    assert got.open_chains == want["open_chains"], name
    assert got.dangling == want["dangling"], name
    assert [(b.edges, b.joins) for b in got.branched] == [
        (b["edges"], b["joins"]) for b in want["branched"]
    ], name
    assert got.degenerate == want["degenerate"], name
    assert [[u.first, u.second] for u in got.unjoined] == want["unjoined"], name


def test_every_directory_is_listed() -> None:
    on_disk = sorted(p.name for p in C3.iterdir() if p.name != "digests.txt")
    assert on_disk == sorted(NAMES)


def test_the_digest_file_covers_the_corpus() -> None:
    # 10 run, 20 solved, the 3 by 3 slot grid.
    lines = (C3 / "digests.txt").read_text().splitlines()
    assert len(lines) == 31
    assert len(DIGESTS) == 31
    for name in NAMES:
        assert f"c3-{name}" in DIGESTS
        for label in ALGORITHMS:
            assert f"c3-solved-{label}-{name}" in DIGESTS
    assert "c3-slot-grid-3x3" in DIGESTS


@pytest.mark.parametrize("name", NAMES)
def test_the_expected_coordinates_give_the_expected_profiles(name: str) -> None:
    fx = load(name)
    values = fx.expected_coordinates()
    found = profiles(fx.sketch, values)
    assert_profiles(name, fx.expected["profiles"], found)
    report = check_profiles(fx.sketch, values, found)
    assert report.is_clean(), report.violations
    assert digest_profiles(fx.sketch, values, found, report) == DIGESTS[f"c3-{name}"]


@pytest.mark.parametrize("label", ALGORITHMS)
@pytest.mark.parametrize("name", NAMES)
def test_solving_gives_the_expected_profiles(name: str, label: str) -> None:
    fx = load(name)
    solution = fx.sketch.solve(SolverOptions(algorithm=ALGORITHMS[label]))
    assert solution.status is Status[fx.expected["status"]]
    assert solution.diagnostics.dof == fx.expected["dof"]
    if name not in UNCOMPARED:
        for key, want in fx.expected_coordinates().items():
            got = solution.coordinates[key]
            assert abs(got - want) <= float(fx.expected.get("tolerance", DEFAULT_TOLERANCE)), (name, key)

    run = check(fx.sketch, solution.coordinates)
    assert run.is_clean(), run
    found = profiles(fx.sketch, solution.coordinates)
    assert_profiles(name, fx.expected["profiles"], found)
    report = check_profiles(fx.sketch, solution.coordinates, found)
    assert report.is_clean(), report.violations
    digest = digest_solved_profiles(fx.sketch, solution, run, found, report)
    assert digest == DIGESTS[f"c3-solved-{label}-{name}"]


def test_the_3x3_slot_grid_equals_its_blessed_digest() -> None:
    sketch, values = slot_grid(3, 3)
    found = profiles(sketch, values)
    report = check_profiles(sketch, values, found)
    assert report.is_clean(), report.violations
    assert digest_profiles(sketch, values, found, report) == DIGESTS["c3-slot-grid-3x3"]


def test_the_acceptance_numbers() -> None:
    fx = load("rectangle")
    (plate,) = profiles(fx.sketch, fx.expected_coordinates()).profiles
    assert abs(plate.outer.signed_area - 7200.0) <= AREA_TOLERANCE * 7200.0
    assert plate.outer.signed_area > 0.0
    assert not any(t.reversed for t in plate.outer.edges)

    fx = load("slotted-plate")
    (plate,) = profiles(fx.sketch, fx.expected_coordinates()).profiles
    want = 7200.0 - (400.0 + 25.0 * math.pi)
    assert abs(plate.net_area - want) <= AREA_TOLERANCE * 7200.0
    (hole,) = plate.holes
    assert hole.signed_area < 0.0
