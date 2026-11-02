"""The C1 corpus from Python: every fixture and twin, under both algorithms.

Expected values are read from `tests/fixtures/c1/<name>/expected.json`
(closed form or by construction); the blessed digests from `digests.txt`,
the same file `crates/laman-debug/tests/determinism.rs` reads.
"""

import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import pytest

import laman
from laman import Algorithm, Coordinates, Sketch, SolverOptions, Status
from laman.check import check
from laman.debug import digest_run, digest_solution

C1 = Path(__file__).resolve().parents[3] / "tests" / "fixtures" / "c1"
NAMES = [
    "sss-triangle",
    "sss-triangle-redundant",
    "sss-triangle-conflicting",
    "belt",
    "belt-redundant",
    "belt-conflicting",
    "slotted-plate",
    "slotted-plate-redundant",
    "slotted-plate-conflicting",
    "slotted-plate-drag",
]
ALGORITHMS = {"lm": Algorithm.LevenbergMarquardt, "dogleg": Algorithm.DogLeg}
# The fixtures whose solve lands on the expected coordinates. A conflicting
# twin's best effort is not the closed form, and the drag fixture's listed
# coordinates are the drag's, not the static solve's (they are held in
# test_drag, step 5).
BASES = [n for n in NAMES if not n.endswith("conflicting") and n != "slotted-plate-drag"]
CONFLICTING = [n for n in NAMES if n.endswith("conflicting")]
# The tolerance of a fixture that names none (laman::model::tolerance::LENGTH).
DEFAULT_TOLERANCE = 1e-9


@dataclass(frozen=True)
class Fixture:
    name: str
    text: str
    sketch: Sketch
    expected: dict[str, Any]

    @property
    def tolerance(self) -> float:
        return float(self.expected.get("tolerance", DEFAULT_TOLERANCE))

    def expected_coordinates(self) -> dict[tuple[int, str], float]:
        return {
            (c["id"], c["coordinate"]): float(c["value"])
            for c in self.expected["coordinates"]
        }


def load(name: str) -> Fixture:
    text = (C1 / name / "sketch.json").read_text()
    expected = json.loads((C1 / name / "expected.json").read_text())
    return Fixture(name, text, Sketch.from_json(text), expected)


def blessed() -> dict[str, str]:
    lines = (C1 / "digests.txt").read_text().splitlines()
    return dict(line.split(" ", 1) for line in lines)


DIGESTS = blessed()
# The violations the checker must report at a conflicting twin's best effort.
VIOLATIONS: dict[str, list[str]] = json.loads((C1 / "violations.json").read_text())


def test_the_digest_file_covers_the_corpus() -> None:
    # 10 run, 20 solved, 20 drag steps.
    assert len(DIGESTS) == 50
    for name in NAMES:
        assert name in DIGESTS
        for label in ALGORITHMS:
            assert f"solved-{label}-{name}" in DIGESTS


@pytest.mark.parametrize("name", NAMES)
def test_json_round_trips(name: str) -> None:
    fx = load(name)
    again = Sketch.from_json(fx.sketch.to_json())
    assert again.to_json() == fx.sketch.to_json()
    assert again.coordinates() == fx.sketch.coordinates()


@pytest.mark.parametrize("name", NAMES)
def test_the_expected_coordinates_check_clean_and_digest(name: str) -> None:
    fx = load(name)
    values = dict(fx.sketch.coordinates().items())
    values.update(fx.expected_coordinates())
    report = check(fx.sketch, Coordinates(values))
    # A conflicting twin's closed-form coordinates violate its conflict set.
    assert report.is_clean() == (name not in CONFLICTING), report
    assert digest_run(fx.sketch, report) == DIGESTS[name]


@pytest.mark.parametrize("label", ALGORITHMS)
@pytest.mark.parametrize("name", NAMES)
def test_solving_reaches_the_expected_answer(name: str, label: str) -> None:
    fx = load(name)
    options = SolverOptions(algorithm=ALGORITHMS[label])
    assert options.algorithm is ALGORITHMS[label]
    solution = fx.sketch.solve(options)

    assert solution.status is Status[fx.expected["status"]]
    assert solution.diagnostics.dof == fx.expected["dof"]
    assert solution.source_revision == fx.sketch.revision
    for key, want in fx.expected_coordinates().items() if name in BASES else []:
        assert abs(solution.coordinates[key] - want) <= fx.tolerance, (name, key)

    # The checker's verdict: clean, or exactly the listed violations.
    report = check(fx.sketch, solution.coordinates)
    assert sorted(v.tag or "" for v in report.violations) == VIOLATIONS.get(name, [])
    assert not report.degenerate
    assert not report.missing
    assert digest_solution(fx.sketch, solution, report) == DIGESTS[f"solved-{label}-{name}"]


def test_the_digest_is_stable_across_solves() -> None:
    fx = load("belt")
    one = fx.sketch.solve()
    two = fx.sketch.solve()
    assert one.coordinates == two.coordinates
    first = digest_solution(fx.sketch, one, check(fx.sketch, one.coordinates))
    second = digest_solution(fx.sketch, two, check(fx.sketch, two.coordinates))
    assert first == second


def test_solver_options_default_to_rust_defaults() -> None:
    options = SolverOptions()
    assert options.algorithm is Algorithm.LevenbergMarquardt
    assert options.max_iterations > 0
    assert SolverOptions(max_bisections=3).max_bisections == 3
    assert options == SolverOptions()
    with pytest.raises(TypeError):
        SolverOptions(algorithm="DogLeg")  # type: ignore[arg-type]


def test_a_malformed_sketch_is_a_value_error() -> None:
    with pytest.raises(ValueError):
        Sketch.from_json("not json")
    with pytest.raises(ValueError):
        Coordinates({(0, "z"): 1.0})
    with pytest.raises(KeyError):
        Coordinates()[(0, "x")]


def test_the_package_exports() -> None:
    assert laman.Status.FullyConstrained.name == "FullyConstrained"
