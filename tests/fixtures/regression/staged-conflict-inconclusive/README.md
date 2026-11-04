# Staged DogLeg leaves a conflict inconclusive

Seven points, two fixed, nine distances: a rigid distance framework built
from true positions with one distance (`span`, between points 4 and 7)
stretched (shrunk from the seed `4ca64a2b…` of the property
`an_extra_distance_is_redundant_when_true_and_a_conflict_when_false`).
`leg-a` and `leg-b` hold points 4 and 7 at 70.015 and 127.357 from the fixed
point 1; `span` asks them 384.92 apart, past the sum 197.37 of the two legs
(the triangle inequality), so the three cannot hold together, and every two
of them can. That set is the answer by hand, not by laman's output.

Under `Algorithm::LevenbergMarquardt`, staged and whole
(`laman::reference::solve`) both prove it. Under `Algorithm::DogLeg` the
whole path proves it and the staged solve ends `NotConverged` with no set
named. The verdict should not depend on the path.

Desired: `Unsatisfiable` with the conflict {leg-a, leg-b, span} under both
algorithms, staged and whole.
