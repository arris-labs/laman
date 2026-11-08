from collections.abc import Iterator, Mapping, Sequence
from typing import Self, final

from ._types import Algorithm, Sense, Status

__all__ = [
    "Arc",
    "Bisection",
    "Circle",
    "Line",
    "Point",
    "Removed",
    "ConstraintRef",
    "Degenerate",
    "DragSession",
    "EntityFreedom",
    "EntityRef",
    "Flip",
    "FlipSubject",
    "Motion",
    "CheckDegenerate",
    "CheckMissing",
    "CheckReport",
    "CheckViolation",
    "Coordinates",
    "Diagnostics",
    "Branched",
    "Loop",
    "Profile",
    "Profiles",
    "Traversal",
    "Unjoined",
    "Rigidity",
    "RigidityError",
    "Sketch",
    "Solution",
    "SolverOptions",
    "check_sketch",
    "digest_drag_step",
    "digest_run",
    "digest_solution",
    "rigidity",
    "solution_text",
    "svg",
    "text",
]

class RigidityError(ValueError):
    """A bar names a point the framework does not have, or a point twice."""

@final
class Rigidity:
    """The generic rigidity of a bar framework."""

    @property
    def points(self) -> int: ...
    @property
    def independent(self) -> list[int]: ...
    @property
    def redundant(self) -> list[int]: ...
    @property
    def dof(self) -> int: ...
    def is_rigid(self) -> bool: ...
    def is_minimally_rigid(self) -> bool: ...

def rigidity(points: int, bars: Sequence[tuple[int, int]]) -> Rigidity:
    """The generic rigidity of `points` points joined by `bars`."""

@final
class Coordinates:
    """The value of every parameter, keyed by `(entity id, "x" | "y" | "r")`."""

    def __new__(
        cls,
        items: Mapping[tuple[int, str], float] | Sequence[tuple[tuple[int, str], float]] | None = None,
    ) -> Coordinates: ...
    def __len__(self) -> int: ...
    def __getitem__(self, key: tuple[int, str], /) -> float: ...
    def __contains__(self, key: tuple[int, str], /) -> bool: ...
    def __iter__(self) -> Iterator[tuple[int, str]]: ...
    def __eq__(self, other: object, /) -> bool: ...
    def keys(self) -> list[tuple[int, str]]: ...
    def values(self) -> list[float]: ...
    def items(self) -> list[tuple[tuple[int, str], float]]: ...

@final
class SolverOptions:
    """How a solve runs: the algorithm and its hard caps."""

    def __new__(
        cls,
        *,
        algorithm: Algorithm | None = None,
        max_iterations: int | None = None,
        max_evaluations: int | None = None,
        max_analysis_solves: int | None = None,
        max_bisections: int | None = None,
    ) -> SolverOptions: ...
    @property
    def algorithm(self) -> Algorithm: ...
    @property
    def max_iterations(self) -> int: ...
    @property
    def max_evaluations(self) -> int: ...
    @property
    def max_analysis_solves(self) -> int: ...
    @property
    def max_bisections(self) -> int: ...
    def __eq__(self, other: object, /) -> bool: ...

@final
class EntityRef:
    """An entity as a diagnostic names it."""

    @property
    def id(self) -> int: ...
    @property
    def kind(self) -> str: ...
    @property
    def tag(self) -> str | None: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...

@final
class ConstraintRef:
    """A constraint as a diagnostic names it."""

    @property
    def id(self) -> int: ...
    @property
    def kind(self) -> str: ...
    @property
    def tag(self) -> str | None: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...

@final
class EntityFreedom:
    """An entity's remaining freedom."""

    @property
    def entity(self) -> EntityRef: ...
    @property
    def dof(self) -> int: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...

@final
class Motion:
    """One parameter's share of a free direction."""

    @property
    def entity(self) -> int: ...
    @property
    def coordinate(self) -> str: ...
    @property
    def amount(self) -> float: ...
    def __eq__(self, other: object, /) -> bool: ...

@final
class Degenerate:
    """An entity that collapsed at the solution."""

    @property
    def entity(self) -> EntityRef: ...
    @property
    def collapse(self) -> str: ...
    @property
    def size(self) -> float: ...
    @property
    def equations(self) -> list[ConstraintRef | EntityRef]: ...

@final
class FlipSubject:
    """What changed branch: an entity or a constraint (the other is None)."""

    @property
    def entity(self) -> EntityRef | None: ...
    @property
    def constraint(self) -> ConstraintRef | None: ...
    def __eq__(self, other: object, /) -> bool: ...

@final
class Flip:
    """A change of branch."""

    @property
    def subject(self) -> FlipSubject: ...
    @property
    def was(self) -> float: ...
    @property
    def now(self) -> float: ...
    def __eq__(self, other: object, /) -> bool: ...

@final
class Bisection:
    """A drag frame that was halved toward its target: the full step flipped
    a branch or moved an arc's sweep too far. A frame that stayed put because
    every halving flipped or jumped is marked too, with the last halving's
    target (never solved at) and halvings 0 if max_bisections is 0."""

    @property
    def reached(self) -> tuple[float, float]: ...
    @property
    def halvings(self) -> int: ...
    def __eq__(self, other: object, /) -> bool: ...

@final
class Diagnostics:
    """What a solve established about the sketch."""

    @property
    def dof(self) -> int: ...
    @property
    def entities(self) -> list[EntityFreedom]: ...
    @property
    def free_directions(self) -> list[list[Motion]]: ...
    @property
    def redundant(self) -> list[ConstraintRef]: ...
    @property
    def conflicts(self) -> list[list[ConstraintRef]]: ...
    @property
    def degenerate(self) -> list[Degenerate]: ...
    @property
    def ill_conditioned(self) -> list[ConstraintRef | EntityRef]: ...
    @property
    def flips(self) -> list[Flip]: ...
    @property
    def bisection(self) -> Bisection | None:
        """Set on a drag frame halved toward its target or one that stayed
        put; None for a static solve and a frame solved at its own target."""
        ...

@final
class Solution:
    """The result of a solve."""

    @property
    def coordinates(self) -> Coordinates: ...
    @property
    def status(self) -> Status: ...
    @property
    def diagnostics(self) -> Diagnostics: ...
    @property
    def source_revision(self) -> int: ...

@final
class Point:
    """A point of a sketch, by id."""

    @property
    def id(self) -> int: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...

@final
class Line:
    """A line segment of a sketch, by id."""

    @property
    def id(self) -> int: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...

@final
class Circle:
    """A circle of a sketch, by id."""

    @property
    def id(self) -> int: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...

@final
class Arc:
    """A circular arc of a sketch, by id."""

    @property
    def id(self) -> int: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...

@final
class Removed:
    """What a cascading removal took out of a sketch, in id order."""

    @property
    def entities(self) -> list[int]: ...
    @property
    def constraints(self) -> list[int]: ...
    def __eq__(self, other: object, /) -> bool: ...

@final
class Sketch:
    """A sketch: entities and the constraints between them."""

    def __new__(cls) -> Sketch: ...
    @staticmethod
    def from_json(text: str) -> Sketch: ...
    def to_json(self) -> str: ...
    @property
    def revision(self) -> int: ...
    def coordinates(self) -> Coordinates: ...
    def solve(self, options: SolverOptions | None = None) -> Solution: ...
    def point(
        self,
        x: float,
        y: float,
        *,
        fixed: bool = False,
        construction: bool = False,
        tag: str | None = None,
        payload: bytes | None = None,
        id: int | None = None,
    ) -> Point: ...
    def line(
        self,
        a: Point,
        b: Point,
        *,
        construction: bool = False,
        tag: str | None = None,
        payload: bytes | None = None,
        id: int | None = None,
    ) -> Line: ...
    def circle(
        self,
        centre: Point,
        r: float,
        *,
        fixed: bool = False,
        construction: bool = False,
        tag: str | None = None,
        payload: bytes | None = None,
        id: int | None = None,
    ) -> Circle: ...
    def arc(
        self,
        centre: Point,
        start: Point,
        end: Point,
        *,
        construction: bool = False,
        tag: str | None = None,
        payload: bytes | None = None,
        id: int | None = None,
    ) -> Arc: ...
    def set_fixed(self, id: int, coordinate: str, fixed: bool) -> None: ...
    def is_fixed(self, id: int, coordinate: str) -> bool: ...
    def set_construction(self, id: int, construction: bool) -> None: ...
    def is_construction(self, id: int) -> bool: ...
    def tag(self, id: int, tag: str) -> None: ...
    def payload(self, id: int, payload: bytes) -> None: ...
    def remove(self, id: int) -> None: ...
    def remove_with_constraints(self, id: int) -> Removed: ...
    def remove_constraint(self, id: int) -> None: ...
    def tag_constraint(self, id: int, tag: str) -> None: ...
    def payload_constraint(self, id: int, payload: bytes) -> None: ...
    def coincident(
        self,
        p: Point,
        q: Point,
        *,
        tag: str | None = None,
        payload: bytes | None = None,
        id: int | None = None,
    ) -> int: ...
    def point_on_line(
        self,
        p: Point,
        line: Line,
        *,
        tag: str | None = None,
        payload: bytes | None = None,
        id: int | None = None,
    ) -> int: ...
    def point_on_circle(
        self,
        p: Point,
        on: Circle | Arc,
        *,
        tag: str | None = None,
        payload: bytes | None = None,
        id: int | None = None,
    ) -> int: ...
    def horizontal(
        self,
        a: Line | Point,
        b: Point | None = None,
        *,
        tag: str | None = None,
        payload: bytes | None = None,
        id: int | None = None,
    ) -> int: ...
    def vertical(
        self,
        a: Line | Point,
        b: Point | None = None,
        *,
        tag: str | None = None,
        payload: bytes | None = None,
        id: int | None = None,
    ) -> int: ...
    def parallel(
        self,
        a: Line,
        b: Line,
        *,
        tag: str | None = None,
        payload: bytes | None = None,
        id: int | None = None,
    ) -> int: ...
    def perpendicular(
        self,
        a: Line,
        b: Line,
        *,
        tag: str | None = None,
        payload: bytes | None = None,
        id: int | None = None,
    ) -> int: ...
    def equal_length(
        self,
        a: Line,
        b: Line,
        *,
        tag: str | None = None,
        payload: bytes | None = None,
        id: int | None = None,
    ) -> int: ...
    def equal_radius(
        self,
        a: Circle | Arc,
        b: Circle | Arc,
        *,
        tag: str | None = None,
        payload: bytes | None = None,
        id: int | None = None,
    ) -> int: ...
    def tangent_line_circle(
        self,
        line: Line,
        round: Circle | Arc,
        *,
        sense: Sense | None = None,
        tag: str | None = None,
        payload: bytes | None = None,
        id: int | None = None,
    ) -> int: ...
    def tangent_circles(
        self,
        a: Circle | Arc,
        b: Circle | Arc,
        *,
        external: bool = False,
        sense: Sense | None = None,
        tag: str | None = None,
        payload: bytes | None = None,
        id: int | None = None,
    ) -> int: ...
    def midpoint(
        self,
        p: Point,
        line: Line,
        *,
        tag: str | None = None,
        payload: bytes | None = None,
        id: int | None = None,
    ) -> int: ...
    def symmetric(
        self,
        p: Point,
        q: Point,
        axis: Line,
        *,
        tag: str | None = None,
        payload: bytes | None = None,
        id: int | None = None,
    ) -> int: ...
    def fix(
        self,
        p: Point,
        x: float,
        y: float,
        *,
        tag: str | None = None,
        payload: bytes | None = None,
        id: int | None = None,
    ) -> int: ...
    def distance(
        self,
        p: Point,
        q: Point,
        value: float,
        *,
        tag: str | None = None,
        payload: bytes | None = None,
        id: int | None = None,
    ) -> int: ...
    def point_line_distance(
        self,
        p: Point,
        line: Line,
        value: float,
        *,
        tag: str | None = None,
        payload: bytes | None = None,
        id: int | None = None,
    ) -> int: ...
    def radius(
        self,
        round: Circle | Arc,
        value: float,
        *,
        tag: str | None = None,
        payload: bytes | None = None,
        id: int | None = None,
    ) -> int: ...
    def angle(
        self,
        a: Line,
        b: Line,
        value: float,
        *,
        tag: str | None = None,
        payload: bytes | None = None,
        id: int | None = None,
    ) -> int: ...
    def apply(self, solution: Solution) -> None: ...
    def drag(self, point: Point | int, options: SolverOptions | None = None) -> DragSession: ...
    def point_handle(self, id: int) -> Point: ...
    def line_handle(self, id: int) -> Line: ...
    def circle_handle(self, id: int) -> Circle: ...
    def arc_handle(self, id: int) -> Arc: ...
    def entities(self) -> list[EntityRef]: ...
    def constraints(self) -> list[ConstraintRef]: ...
    def constraint_entities(self, id: int) -> list[int] | None: ...
    def constraint_value(self, id: int) -> float | None: ...
    def tag_of(self, id: int) -> str | None: ...
    def payload_of(self, id: int) -> bytes | None: ...
    def constraint_tag_of(self, id: int) -> str | None: ...
    def constraint_payload_of(self, id: int) -> bytes | None: ...

@final
class DragSession:
    """A point being dragged through targets, one frame per `to`."""

    def to(self, x: float, y: float) -> Solution: ...
    def finish(self) -> Solution: ...

@final
class CheckViolation:
    @property
    def constraint(self) -> int | None: ...
    @property
    def arc(self) -> int | None: ...
    @property
    def tag(self) -> str | None: ...
    @property
    def kind(self) -> str: ...
    @property
    def measured(self) -> float: ...
    @property
    def wanted(self) -> float: ...

@final
class CheckDegenerate:
    @property
    def id(self) -> int: ...
    @property
    def tag(self) -> str | None: ...
    @property
    def kind(self) -> str: ...
    @property
    def what(self) -> str: ...
    @property
    def size(self) -> float: ...

@final
class CheckMissing:
    @property
    def id(self) -> int: ...
    @property
    def coordinate(self) -> str | None: ...

@final
class CheckReport:
    @property
    def violations(self) -> list[CheckViolation]: ...
    @property
    def degenerate(self) -> list[CheckDegenerate]: ...
    @property
    def missing(self) -> list[CheckMissing]: ...
    def is_clean(self) -> bool: ...

def check_sketch(sketch: Sketch, coordinates: Coordinates) -> CheckReport: ...
def digest_run(sketch: Sketch, report: CheckReport) -> str: ...
def digest_solution(sketch: Sketch, solution: Solution, report: CheckReport) -> str: ...
def digest_drag_step(
    sketch: Sketch,
    index: int,
    target: tuple[float, float],
    solution: Solution,
    report: CheckReport,
) -> str: ...
def text(sketch: Sketch, coordinates: Coordinates, report: CheckReport) -> str: ...
def svg(sketch: Sketch, coordinates: Coordinates, report: CheckReport) -> str: ...
def solution_text(sketch: Sketch, solution: Solution) -> str: ...

@final
class Traversal:
    """An edge as a loop traverses it."""

    def __new__(cls, entity: int, reversed: bool) -> Self: ...
    @property
    def entity(self) -> int: ...
    @property
    def reversed(self) -> bool: ...

@final
class Loop:
    """A closed boundary: edges end to end, or one circle."""

    def __new__(
        cls, edges: Sequence[Traversal], signed_area: float, joins: Sequence[int] = ...
    ) -> Self: ...
    @property
    def edges(self) -> list[Traversal]: ...
    @property
    def signed_area(self) -> float: ...
    @property
    def joins(self) -> list[int]: ...

@final
class Profile:
    """An outer loop, counter-clockwise, with its holes, clockwise."""

    def __new__(cls, outer: Loop, holes: Sequence[Loop], net_area: float) -> Self: ...
    @property
    def outer(self) -> Loop: ...
    @property
    def holes(self) -> list[Loop]: ...
    @property
    def net_area(self) -> float: ...

@final
class Branched:
    """A core of edges where some join has three or more of them."""

    def __new__(cls, edges: Sequence[int], joins: Sequence[int]) -> Self: ...
    @property
    def edges(self) -> list[int]: ...
    @property
    def joins(self) -> list[int]: ...

@final
class Unjoined:
    """Two free ends that coincide but are not joined."""

    def __new__(cls, first: int, second: int) -> Self: ...
    @property
    def first(self) -> int: ...
    @property
    def second(self) -> int: ...

@final
class Profiles:
    """What profile extraction finds in a sketch at some coordinates."""

    def __new__(
        cls,
        profiles: Sequence[Profile] = ...,
        open_chains: Sequence[Sequence[int]] = ...,
        dangling: Sequence[int] = ...,
        branched: Sequence[Branched] = ...,
        degenerate: Sequence[int] = ...,
        unjoined: Sequence[Unjoined] = ...,
    ) -> Self: ...
    @property
    def profiles(self) -> list[Profile]: ...
    @property
    def open_chains(self) -> list[list[int]]: ...
    @property
    def dangling(self) -> list[int]: ...
    @property
    def branched(self) -> list[Branched]: ...
    @property
    def degenerate(self) -> list[int]: ...
    @property
    def unjoined(self) -> list[Unjoined]: ...
