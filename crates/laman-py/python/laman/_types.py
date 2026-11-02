"""The enumerations of the API, plain `enum.Enum` classes.

Their members carry Rust's names; the extension module reads and returns
them by name, so the values are never compared with the Rust ones.
"""

import enum

__all__ = ["Algorithm", "Status"]


class Algorithm(enum.Enum):
    """The step an iteration proposes."""

    LevenbergMarquardt = "LevenbergMarquardt"
    """Levenberg-Marquardt: identity-damped normal equations. The default."""
    DogLeg = "DogLeg"
    """Powell's dog leg between the steepest-descent and Gauss-Newton steps."""


class Status(enum.Enum):
    """The verdict of a solve."""

    FullyConstrained = "FullyConstrained"
    """Every equation holds and no degree of freedom remains."""
    UnderConstrained = "UnderConstrained"
    """Every equation holds and the sketch still has freedom."""
    Unsatisfiable = "Unsatisfiable"
    """A conflict set exists; the coordinates are the best effort."""
    NotConverged = "NotConverged"
    """A cap was hit with equations unsatisfied and no conflict proven."""
