"""The binding answers as the crate does; the counts themselves are tested in Rust."""

import pytest

import laman


def test_triangle_is_minimally_rigid() -> None:
    r = laman.rigidity(3, [(0, 1), (1, 2), (2, 0)])
    assert r.dof == 0
    assert r.is_minimally_rigid()


def test_complete_four_names_its_last_bar() -> None:
    bars = [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)]
    r = laman.rigidity(4, bars)
    assert r.is_rigid()
    assert r.independent == [0, 1, 2, 3, 4]
    assert r.redundant == [5]


def test_square_flexes() -> None:
    assert laman.rigidity(4, [(0, 1), (1, 2), (2, 3), (3, 0)]).dof == 1


def test_bad_bar_raises() -> None:
    with pytest.raises(laman.RigidityError, match="bar 1 names point 3"):
        laman.rigidity(3, [(0, 1), (1, 3)])
    with pytest.raises(ValueError):
        laman.rigidity(3, [(2, 2)])
