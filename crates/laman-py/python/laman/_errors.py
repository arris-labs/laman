"""The refusals of the API: `ModelError` and one subclass per Rust variant.

The extension module builds them by class name; the ids and values the Rust
variant carries are attributes. Under- and over-constrained sketches are
never errors: they are a `Solution`'s status.
"""

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
    """A sketch operation was refused; the subclasses say why.

    A malformed input is an error; an under- or over-constrained sketch never
    is. Each subclass carries the caller's ids and values as attributes.
    """

    def __init__(self, message: str = "", **fields: object) -> None:
        super().__init__(message)
        for name, value in fields.items():
            setattr(self, name, value)


class UnknownId(ModelError):
    """An id names no entity of the sketch (never added, or removed)."""

    id: int


class WrongKind(ModelError):
    """An entity is of another kind than the operation needs."""

    id: int
    expected: str
    found: str


class FixedPoint(ModelError):
    """A drag was asked of a point with both coordinates fixed."""

    id: int


class NonFinite(ModelError):
    """A coordinate or value is NaN or infinite."""

    what: str
    value: float


class DimensionOutOfRange(ModelError):
    """A value is outside what its parameter allows."""

    what: str
    value: float
    wanted: str


class DuplicateId(ModelError):
    """An entity id the caller passed is already in use."""

    id: int


class IdsExhausted(ModelError):
    """Every id the sketch can mint is used."""


class RepeatedEntity(ModelError):
    """An entity or constraint names the same entity twice."""

    id: int
    kind: str


class UnknownConstraint(ModelError):
    """A constraint id names no constraint of the sketch."""

    id: int


class DuplicateConstraintId(ModelError):
    """A constraint id the caller passed is already in use."""

    id: int


class NoSuchParameter(ModelError):
    """The parameter does not exist on that entity."""

    id: int
    coordinate: str


class FormatVersion(ModelError):
    """A file's `version` is newer than this reader loads, or is 0."""

    found: int
    supported: int


class WrongFormat(ModelError):
    """A file's `format` is not `laman.sketch`."""

    found: str


class InvalidPayload(ModelError):
    """A payload in a file is not valid base64."""


class EntityError(ModelError):
    """Loading a file failed at an entity; `source` says why."""

    id: int
    source: ModelError


class ConstraintError(ModelError):
    """Loading a file failed at a constraint; `source` says why."""

    id: int
    source: ModelError


class Stale(ModelError):
    """A solution was solved for another revision of the sketch."""

    solved: int
    current: int


class ForeignSolution(ModelError):
    """A solution has this revision but was solved from other contents."""


class IncompleteSolution(ModelError):
    """A solution holds no value for a parameter the sketch has."""

    id: int
    coordinate: str


class FixedMoved(ModelError):
    """A solution would move a fixed parameter."""

    id: int
    coordinate: str
    fixed: float
    given: float


class InUse(ModelError):
    """An entity is still used and the cascade was not asked for."""

    id: int
    entities: list[int]
    constraints: list[int]


# The variants are reached as `ModelError.Stale` and so on; `Entity` and
# `Constraint` are Rust's names for the load failures.
for _cls in (
    UnknownId,
    WrongKind,
    FixedPoint,
    NonFinite,
    DimensionOutOfRange,
    DuplicateId,
    IdsExhausted,
    RepeatedEntity,
    UnknownConstraint,
    DuplicateConstraintId,
    NoSuchParameter,
    FormatVersion,
    WrongFormat,
    InvalidPayload,
    Stale,
    ForeignSolution,
    IncompleteSolution,
    FixedMoved,
    InUse,
):
    setattr(ModelError, _cls.__name__, _cls)
ModelError.Entity = EntityError  # type: ignore[attr-defined]
ModelError.Constraint = ConstraintError  # type: ignore[attr-defined]
del _cls
