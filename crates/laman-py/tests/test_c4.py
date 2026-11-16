"""The C4 corpus from Python: the six profile fixtures and the eight
intersection fixtures of `tests/fixtures/c4/`, at their expected coordinates
and solved under both algorithms, every number of `expected.json`; the
blessed digests of `digests.txt` (the file `determinism.rs` reads); the
checker's broken twins, each naming its variant; and the intersection corpus
scaled by 1e-6 and 1e6 and shifted (`crates/laman-debug/tests/c4_twins.rs`).
"""

import json
import math
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import pytest

from laman import Algorithm, Coordinates, Sketch, SolverOptions, Status
from laman.check import check, check_intersections, check_profiles
from laman.debug import (
    cell_grid,
    digest_intersections,
    digest_profiles,
    digest_solved_intersections,
    digest_solved_profiles,
)
from laman.intersect import Intersection, Intersections, Meeting, MeetingKind, intersections
from laman.profile import Loop, Profile, Profiles, Traversal, profiles

C4 = Path(__file__).resolve().parents[3] / "tests" / "fixtures" / "c4"
C3 = C4.parent / "c3"
INTERSECTION_NAMES = [
    "crossing-lines",
    "line-through-circle",
    "two-circles",
    "tangent-touch",
    "collinear-overlap",
    "arc-outside-sweep",
    "t-end-on-line",
    "arc-crosses-line",
]
PROFILE_NAMES = [
    "shared-wall",
    "window",
    "tangent-branch",
    "figure-eight",
    "bridged-plate",
    "branched-in-plate",
]
ALGORITHMS = {"lm": Algorithm.LevenbergMarquardt, "dogleg": Algorithm.DogLeg}
# laman_debug::fixture::AREA_TOLERANCE and laman::model::tolerance::LENGTH, as
# test_c3 copies them.
AREA_TOLERANCE = 1e-9
DEFAULT_TOLERANCE = 1e-9
# The expected values of `intersections` are closed forms; a fixture that names
# no tolerance is held to this, relative to max(1, |value|).
MEETING_TOLERANCE = 1e-9


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


def load(name: str, group: Path = C4) -> Fixture:
    text = (group / name / "sketch.json").read_text()
    expected = json.loads((group / name / "expected.json").read_text())
    return Fixture(name, Sketch.from_json(text), expected)


def blessed() -> dict[str, str]:
    lines = (C4 / "digests.txt").read_text().splitlines()
    return dict(line.split(" ", 1) for line in lines)


DIGESTS = blessed()


def near(got: float, want: float, tolerance: float = MEETING_TOLERANCE) -> bool:
    return abs(got - want) <= tolerance * max(1.0, abs(want))


def area_holds(want: dict[str, float], got: float) -> bool:
    return near(got, want["value"], want.get("tolerance", AREA_TOLERANCE))


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
    assert got.bridges == want["bridges"], name
    assert got.degenerate == want["degenerate"], name
    assert [[u.first, u.second] for u in got.unjoined] == want["unjoined"], name


def assert_intersections(name: str, want: dict[str, Any], got: Intersections) -> None:
    assert len(got.pairs) == len(want["pairs"]), name
    for w, g in zip(want["pairs"], got.pairs):
        assert (g.first, g.second) == (w["first"], w["second"]), name
        assert len(g.meetings) == len(w["meetings"]), (name, g.first, g.second)
        for wm, gm in zip(w["meetings"], g.meetings):
            label = (name, g.first, g.second)
            assert gm.kind.name == wm["kind"], label
            assert near(gm.at[0], wm["at"][0]) and near(gm.at[1], wm["at"][1]), (label, gm.at)
            assert near(gm.on_first, wm["on_first"]), (label, gm.on_first)
            assert near(gm.on_second, wm["on_second"]), (label, gm.on_second)
            if wm["kind"] == "overlap":
                assert gm.kind.to is not None
                assert near(gm.kind.to[0], wm["to"][0]) and near(gm.kind.to[1], wm["to"][1])
                assert gm.kind.to_first is not None and near(gm.kind.to_first, wm["to_first"])
                assert gm.kind.to_second is not None and near(gm.kind.to_second, wm["to_second"])


def solve(fx: Fixture, label: str) -> Any:
    solution = fx.sketch.solve(SolverOptions(algorithm=ALGORITHMS[label]))
    assert solution.status is Status[fx.expected["status"]], fx.name
    assert solution.diagnostics.dof == fx.expected["dof"], fx.name
    for key, want in fx.expected_coordinates().items():
        got = solution.coordinates[key]
        assert abs(got - want) <= float(fx.expected.get("tolerance", DEFAULT_TOLERANCE)), (fx.name, key)
    return solution


def test_every_directory_is_listed() -> None:
    on_disk = sorted(p.name for p in C4.iterdir() if p.name != "digests.txt")
    assert on_disk == sorted(INTERSECTION_NAMES + PROFILE_NAMES)


def test_the_digest_file_covers_the_corpus() -> None:
    # 14 fixtures, each at its expected coordinates and under both algorithms,
    # and the 2 by 2 cell grid's profiles and intersections.
    lines = (C4 / "digests.txt").read_text().splitlines()
    assert len(lines) == 14 * 3 + 2
    assert len(DIGESTS) == len(lines)
    for name in INTERSECTION_NAMES + PROFILE_NAMES:
        assert f"c4-{name}" in DIGESTS
        for label in ALGORITHMS:
            assert f"c4-solved-{label}-{name}" in DIGESTS
    assert "c4-cell-grid-2x2" in DIGESTS
    assert "c4-cell-grid-2x2-intersections" in DIGESTS


@pytest.mark.parametrize("name", INTERSECTION_NAMES)
def test_the_expected_coordinates_give_the_expected_intersections(name: str) -> None:
    fx = load(name)
    values = fx.expected_coordinates()
    found = intersections(fx.sketch, values)
    assert_intersections(name, fx.expected["intersections"], found)
    report = check_intersections(fx.sketch, values, found)
    assert report.is_clean(), report.violations
    assert digest_intersections(fx.sketch, values, found, report) == DIGESTS[f"c4-{name}"]


@pytest.mark.parametrize("label", ALGORITHMS)
@pytest.mark.parametrize("name", INTERSECTION_NAMES)
def test_solving_gives_the_expected_intersections(name: str, label: str) -> None:
    fx = load(name)
    solution = solve(fx, label)
    run = check(fx.sketch, solution.coordinates)
    assert run.is_clean(), run
    found = intersections(fx.sketch, solution.coordinates)
    assert_intersections(name, fx.expected["intersections"], found)
    checked = check_intersections(fx.sketch, solution.coordinates, found)
    assert checked.is_clean(), checked.violations
    digest = digest_solved_intersections(fx.sketch, solution, run, found, checked)
    assert digest == DIGESTS[f"c4-solved-{label}-{name}"]


@pytest.mark.parametrize("name", PROFILE_NAMES)
def test_the_expected_coordinates_give_the_expected_profiles(name: str) -> None:
    fx = load(name)
    values = fx.expected_coordinates()
    found = profiles(fx.sketch, values)
    assert_profiles(name, fx.expected["profiles"], found)
    report = check_profiles(fx.sketch, values, found)
    assert report.is_clean(), report.violations
    assert digest_profiles(fx.sketch, values, found, report) == DIGESTS[f"c4-{name}"]


@pytest.mark.parametrize("label", ALGORITHMS)
@pytest.mark.parametrize("name", PROFILE_NAMES)
def test_solving_gives_the_expected_profiles(name: str, label: str) -> None:
    fx = load(name)
    solution = solve(fx, label)
    run = check(fx.sketch, solution.coordinates)
    assert run.is_clean(), run
    found = profiles(fx.sketch, solution.coordinates)
    assert_profiles(name, fx.expected["profiles"], found)
    report = check_profiles(fx.sketch, solution.coordinates, found)
    assert report.is_clean(), report.violations
    digest = digest_solved_profiles(fx.sketch, solution, run, found, report)
    assert digest == DIGESTS[f"c4-solved-{label}-{name}"]


def test_the_2x2_cell_grid_equals_its_blessed_digests() -> None:
    sketch, values = cell_grid(2, 2)
    found = profiles(sketch, values)
    report = check_profiles(sketch, values, found)
    assert report.is_clean(), report.violations
    assert digest_profiles(sketch, values, found, report) == DIGESTS["c4-cell-grid-2x2"]
    meetings = intersections(sketch, values)
    checked = check_intersections(sketch, values, meetings)
    assert checked.is_clean(), checked.violations
    assert (
        digest_intersections(sketch, values, meetings, checked)
        == DIGESTS["c4-cell-grid-2x2-intersections"]
    )


@pytest.mark.parametrize("rows", [1, 2, 3])
@pytest.mark.parametrize("cols", [1, 2, 3])
def test_a_cell_grid_is_one_profile_per_cell_with_the_slot_as_its_hole(rows: int, cols: int) -> None:
    # Each cell is 4000 less its slot, 40 by 10 and a disc of radius 5 (the
    # tests/c4.rs generator test).
    sketch, values = cell_grid(rows, cols)
    found = profiles(sketch, values)
    assert len(found.profiles) == rows * cols
    assert not found.bridges
    for p in found.profiles:
        assert len(p.holes) == 1
        want = 4000.0 - (400.0 + 25.0 * math.pi)
        assert abs(p.net_area - want) <= 1e-9 * want
    assert check_profiles(sketch, values, found).is_clean()


def test_the_profile_acceptance_numbers() -> None:
    def areas(name: str) -> list[float]:
        fx = load(name)
        found = profiles(fx.sketch, fx.expected_coordinates())
        return sorted(p.net_area for p in found.profiles)

    def close(got: list[float], want: list[float]) -> None:
        assert len(got) == len(want)
        for g, w in zip(got, want):
            assert abs(g - w) <= 1e-9 * max(1.0, abs(w)), (g, w)

    close(areas("shared-wall"), [3600.0, 3600.0])
    close(areas("window"), [3600.0] * 4)
    close(areas("tangent-branch"), sorted([150.0 - 25.0 * math.pi, 25.0 * math.pi]))
    close(areas("figure-eight"), [200.0, 200.0])
    close(areas("bridged-plate"), [7600.0])
    close(areas("branched-in-plate"), [30000.0])

    fx = load("bridged-plate")
    found = profiles(fx.sketch, fx.expected_coordinates())
    (plate,) = found.profiles
    (bridge,) = found.bridges
    assert len(plate.holes) == 1
    listed = [t.entity for t in [*plate.outer.edges, *plate.holes[0].edges]]
    assert bridge not in listed
    fx = load("branched-in-plate")
    found = profiles(fx.sketch, fx.expected_coordinates())
    assert len(found.profiles[0].holes) == 2 and not found.bridges


def test_the_intersection_acceptance_numbers() -> None:
    def kinds(name: str) -> list[list[str]]:
        fx = load(name)
        found = intersections(fx.sketch, fx.expected_coordinates())
        return [[m.kind.name for m in p.meetings] for p in found.pairs]

    assert kinds("crossing-lines") == [["crossing"]]
    assert kinds("line-through-circle") == [["crossing", "crossing"]]
    assert kinds("two-circles") == [["crossing", "crossing"]]
    assert kinds("tangent-touch") == [["touch"]]
    assert kinds("collinear-overlap") == [["overlap"]]
    assert kinds("arc-outside-sweep") == []
    assert kinds("t-end-on-line") == [["crossing"]]
    assert kinds("arc-crosses-line") == [["crossing"]]


# The broken twins (c4_twins.rs): each fixture's result with one thing broken.


def replace_meeting(found: Intersections, p: int, m: int, meeting: Meeting | None) -> Intersections:
    pairs = []
    for i, pair in enumerate(found.pairs):
        meetings = list(pair.meetings)
        if i == p:
            if meeting is None:
                del meetings[m]
            else:
                meetings[m] = meeting
        if meetings:
            pairs.append(Intersection(pair.first, pair.second, meetings))
    return Intersections(pairs)


def violation_kinds(sk: Sketch, values: Coordinates, found: Intersections) -> list[tuple[str, int | None]]:
    return [(v.kind, v.meeting) for v in check_intersections(sk, values, found).violations]


@pytest.mark.parametrize("name", INTERSECTION_NAMES)
def test_a_broken_intersection_names_its_variant(name: str) -> None:
    fx = load(name)
    values = fx.expected_coordinates()
    found = intersections(fx.sketch, values)
    size = max(1.0, *(abs(v) for _, v in values.items()))
    for p, pair in enumerate(found.pairs):
        # The pair dropped.
        twin = Intersections([q for i, q in enumerate(found.pairs) if i != p])
        report = check_intersections(fx.sketch, values, twin)
        assert any(
            (v.kind, v.first, v.second) == ("missed", pair.first, pair.second)
            for v in report.violations
        ), (name, "dropped pair")
        for m, meeting in enumerate(pair.meetings):

            def at_m(kind: str, got: Intersections, m: int = m) -> bool:
                return (kind, m) in violation_kinds(fx.sketch, values, got)

            if len(pair.meetings) > 1:
                twin = replace_meeting(found, p, m, None)
                assert any(
                    (v.kind, v.first, v.second) == ("missed", pair.first, pair.second)
                    for v in check_intersections(fx.sketch, values, twin).violations
                ), (name, "dropped meeting", m)
            # Its point moved by a tenth of the sketch's size.
            moved = Meeting(meeting.kind, (meeting.at[0] + 0.1 * size, meeting.at[1]), meeting.on_first, meeting.on_second)
            kinds = {k for k, _ in violation_kinds(fx.sketch, values, replace_meeting(found, p, m, moved))}
            assert kinds & {"off_curve", "parameter"}, (name, "moved point")
            # Its parameter on the first curve changed.
            t = meeting.on_first
            shifted = Meeting(meeting.kind, meeting.at, t - 0.2 if t > 0.5 else t + 0.2, meeting.on_second)
            assert at_m("parameter", replace_meeting(found, p, m, shifted)), (name, "parameter")
            # Its kind changed.
            other = MeetingKind.touch() if meeting.kind.name == "crossing" else MeetingKind.crossing()
            recast = Meeting(other, meeting.at, meeting.on_first, meeting.on_second)
            assert at_m("kind", replace_meeting(found, p, m, recast)), (name, "kind", meeting.kind.name)


def test_a_broken_profile_split_names_its_variant() -> None:
    # A bridge put in a loop is misclassified.
    fx = load("bridged-plate")
    values = fx.expected_coordinates()
    found = profiles(fx.sketch, values)
    (bridge,) = found.bridges
    (plate,) = found.profiles
    outer = Loop([*plate.outer.edges, Traversal(bridge, False)], plate.outer.signed_area, plate.outer.joins)
    twin = Profiles(profiles=[Profile(outer, plate.holes, plate.net_area)])
    kinds = [(v.kind, v.entity) for v in check_profiles(fx.sketch, values, twin).violations]
    assert ("misclassified", bridge) in kinds, kinds

    # A bridge listed as dangling instead: only that.
    twin = Profiles(profiles=found.profiles, dangling=[bridge])
    kinds = [(v.kind, v.entity) for v in check_profiles(fx.sketch, values, twin).violations]
    assert kinds == [("misclassified", bridge)]

    # Two bounded areas merged into one loop: the wall between them is inside
    # it and left out.
    fx = load("shared-wall")
    values = fx.expected_coordinates()
    found = profiles(fx.sketch, values)
    a, b = (p.outer for p in found.profiles)
    wall = next(t.entity for t in a.edges if t.entity in {u.entity for u in b.edges})

    def rotated(loop: Loop, at: int) -> list[Traversal]:
        i = next(k for k, t in enumerate(loop.edges) if t.entity == wall)
        n = len(loop.edges)
        shift = (i + n - at) % n
        return [*loop.edges[shift:], *loop.edges[:shift]]

    ra, rb = rotated(a, 3), rotated(b, 0)
    merged_edges = [*ra[:-1], *rb[1:]]
    area = a.signed_area + b.signed_area
    merged = Loop(merged_edges, area, [])
    twin = Profiles(profiles=[Profile(merged, [], area)])
    kinds = [(v.kind, v.first) for v in check_profiles(fx.sketch, values, twin).violations]
    assert ("not_one_area", merged_edges[0].entity) in kinds, kinds

    # The island that lies in the plate's own hole, listed as a second hole.
    fx = load("plate-island", C3)
    values = fx.expected_coordinates()
    found = profiles(fx.sketch, values)
    *rest, island_profile = found.profiles
    island = island_profile.outer
    flipped = Loop(
        [Traversal(t.entity, not t.reversed) for t in reversed(island.edges)],
        -island.signed_area,
        island.joins,
    )
    plate = rest[0]
    twin = Profiles(profiles=[Profile(plate.outer, [*plate.holes, flipped], plate.net_area), *rest[1:]])
    kinds = [(v.kind, v.first) for v in check_profiles(fx.sketch, values, twin).violations]
    assert ("containment", flipped.edges[0].entity) in kinds, kinds


def moved(values: Coordinates, scale: float, shift: float) -> Coordinates:
    out: dict[Any, float] = {}
    for (eid, coordinate), v in values.items():
        if coordinate == "x":
            out[(eid, coordinate)] = (v + shift) * scale
        elif coordinate == "y":
            out[(eid, coordinate)] = (v - shift * 0.7) * scale
        else:
            out[(eid, coordinate)] = v * scale
    return Coordinates(out)


@pytest.mark.parametrize("name", INTERSECTION_NAMES)
def test_the_intersection_corpus_is_clean_scaled_and_shifted(name: str) -> None:
    fx = load(name)
    values = fx.expected_coordinates()
    base = [[m.kind.name for m in p.meetings] for p in intersections(fx.sketch, values).pairs]
    for scale, shift in [(1.0, 0.0), (1e-6, 0.0), (1e6, 0.0), (1e-6, 1e3), (1e6, 1e3)]:
        at = moved(values, scale, shift)
        found = intersections(fx.sketch, at)
        assert [[m.kind.name for m in p.meetings] for p in found.pairs] == base, (name, scale, shift)
        report = check_intersections(fx.sketch, at, found)
        assert report.is_clean(), (name, scale, shift, report.violations)
