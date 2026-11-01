from collections.abc import Sequence
from typing import final

__all__ = ["Rigidity", "RigidityError", "rigidity"]

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
