"""Stub: the exceptions are also reached as `ModelError.<Variant>`."""

from typing import ClassVar

__all__ = [
    "ConstraintError",
    "DimensionOutOfRange",
    "DuplicateConstraintId",
    "DuplicateId",
    "EntityError",
    "FixedMoved",
    "FixedPoint",
    "ForeignSolution",
    "FormatVersion",
    "IdsExhausted",
    "InUse",
    "IncompleteSolution",
    "InvalidPayload",
    "ModelError",
    "NoSuchParameter",
    "NonFinite",
    "RepeatedEntity",
    "Stale",
    "UnknownConstraint",
    "UnknownId",
    "WrongFormat",
    "WrongKind",
]

class ModelError(ValueError):
    def __init__(self, message: str = "", **fields: object) -> None: ...
    UnknownId: ClassVar[type[UnknownId]]
    WrongKind: ClassVar[type[WrongKind]]
    FixedPoint: ClassVar[type[FixedPoint]]
    NonFinite: ClassVar[type[NonFinite]]
    DimensionOutOfRange: ClassVar[type[DimensionOutOfRange]]
    DuplicateId: ClassVar[type[DuplicateId]]
    IdsExhausted: ClassVar[type[IdsExhausted]]
    RepeatedEntity: ClassVar[type[RepeatedEntity]]
    UnknownConstraint: ClassVar[type[UnknownConstraint]]
    DuplicateConstraintId: ClassVar[type[DuplicateConstraintId]]
    NoSuchParameter: ClassVar[type[NoSuchParameter]]
    FormatVersion: ClassVar[type[FormatVersion]]
    WrongFormat: ClassVar[type[WrongFormat]]
    InvalidPayload: ClassVar[type[InvalidPayload]]
    Entity: ClassVar[type[EntityError]]
    Constraint: ClassVar[type[ConstraintError]]
    Stale: ClassVar[type[Stale]]
    ForeignSolution: ClassVar[type[ForeignSolution]]
    IncompleteSolution: ClassVar[type[IncompleteSolution]]
    FixedMoved: ClassVar[type[FixedMoved]]
    InUse: ClassVar[type[InUse]]

class UnknownId(ModelError):
    id: int

class WrongKind(ModelError):
    id: int
    expected: str
    found: str

class FixedPoint(ModelError):
    id: int

class NonFinite(ModelError):
    what: str
    value: float

class DimensionOutOfRange(ModelError):
    what: str
    value: float
    wanted: str

class DuplicateId(ModelError):
    id: int

class IdsExhausted(ModelError):
    ...

class RepeatedEntity(ModelError):
    id: int
    kind: str

class UnknownConstraint(ModelError):
    id: int

class DuplicateConstraintId(ModelError):
    id: int

class NoSuchParameter(ModelError):
    id: int
    coordinate: str

class FormatVersion(ModelError):
    found: int
    supported: int

class WrongFormat(ModelError):
    found: str

class InvalidPayload(ModelError):
    ...

class EntityError(ModelError):
    id: int
    source: ModelError

class ConstraintError(ModelError):
    id: int
    source: ModelError

class Stale(ModelError):
    solved: int
    current: int

class ForeignSolution(ModelError):
    ...

class IncompleteSolution(ModelError):
    id: int
    coordinate: str

class FixedMoved(ModelError):
    id: int
    coordinate: str
    fixed: float
    given: float

class InUse(ModelError):
    id: int
    entities: list[int]
    constraints: list[int]
