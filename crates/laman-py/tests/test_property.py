"""Sketches built by construction, solved back.

A Henneberg-built distance framework: a ground point, a second point held by
a `horizontal` and a distance, then each new point joined to two earlier,
non-collinear points by two distances. Such a framework is minimally and
infinitesimally rigid, so its distances are independent and dropping `k` of
them leaves exactly `k` degrees of freedom. The start is the construction's
coordinates, nudged, so the solve has work to do.
"""

import math

from hypothesis import assume, given, strategies as st

from laman import Algorithm, Sketch, SolverOptions, Status
from laman.check import check
from laman.debug import digest_solution

COORD = st.floats(min_value=-50.0, max_value=50.0, allow_nan=False, allow_infinity=False)
NUDGE = st.floats(min_value=-0.05, max_value=0.05, allow_nan=False, allow_infinity=False)
# A new point whose parents and it span less area than this (twice the
# triangle's) makes the framework only nearly rigid.
MIN_CROSS = 1.0
MIN_SEPARATION = 1.0


@st.composite
def frameworks(d: st.DrawFn) -> tuple[Sketch, int, int]:
    """(sketch, number of distances, degrees of freedom the construction left)."""
    ground = (d(COORD), d(COORD))
    length = d(st.floats(min_value=1.0, max_value=50.0))
    truth = [ground, (ground[0] + length, ground[1])]
    parents: list[tuple[int, int]] = []
    count = d(st.integers(min_value=0, max_value=6))
    for _ in range(count):
        i = int(d(st.integers(min_value=0, max_value=len(truth) - 1)))
        j = int(d(st.integers(min_value=0, max_value=len(truth) - 1)))
        assume(i != j)
        new = (d(COORD), d(COORD))
        a, b = truth[i], truth[j]
        cross = (b[0] - a[0]) * (new[1] - a[1]) - (b[1] - a[1]) * (new[0] - a[0])
        assume(abs(cross) >= MIN_CROSS)
        assume(all(math.dist(new, p) >= MIN_SEPARATION for p in truth))
        truth.append(new)
        parents.append((i, j))
    sk = Sketch()
    ids = []
    for n, (x, y) in enumerate(truth):
        nudge = 0.0 if n == 0 else d(NUDGE)
        ids.append(sk.point(x + nudge, y - nudge, fixed=(n == 0)))
    sk.horizontal(ids[0], ids[1])
    bars = [(0, 1)]
    for point, (i, j) in enumerate(parents, start=2):
        bars += [(point, i), (point, j)]
    handles = [sk.distance(ids[a], ids[b], math.dist(truth[a], truth[b])) for a, b in bars]
    drops = int(d(st.integers(min_value=0, max_value=len(handles))))
    order = d(st.permutations(range(len(handles))))
    for k in list(order)[:drops]:
        sk.remove_constraint(handles[k])
    return sk, len(handles), drops


@given(frameworks(), st.sampled_from(list(Algorithm)))
def test_a_framework_solves_back_with_the_constructed_freedom(
    built: tuple[Sketch, int, int], algorithm: Algorithm
) -> None:
    sk, _, drops = built
    solution = sk.solve(SolverOptions(algorithm=algorithm))
    expected = Status.FullyConstrained if drops == 0 else Status.UnderConstrained
    assert solution.status is expected
    assert solution.diagnostics.dof == drops
    report = check(sk, solution.coordinates)
    assert report.is_clean(), report
    assert not solution.diagnostics.redundant
    assert not solution.diagnostics.conflicts


@given(frameworks())
def test_two_solves_of_one_sketch_share_a_digest(built: tuple[Sketch, int, int]) -> None:
    sk = built[0]
    one, two = sk.solve(), sk.solve()
    assert digest_solution(sk, one, check(sk, one.coordinates)) == digest_solution(
        sk, two, check(sk, two.coordinates)
    )
