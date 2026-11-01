"""Laman: a deterministic 2D geometric constraint solver. A thin layer over the Rust crate.

This early release holds the generic rigidity count of bar frameworks
(Laman's theorem); the sketch solver is being built.
"""

from ._laman import Rigidity, RigidityError, rigidity

__all__ = ["Rigidity", "RigidityError", "rigidity"]
