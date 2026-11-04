# DogLeg leaves a conflict inconclusive

Six points, two fixed, eight distances: the distances of a rigid framework
built from true positions, plus one stretched by a factor of 1.2 (shrunk from
a property test, `an_extra_distance_is_redundant_when_true_and_a_conflict_when_false`).
The distances cannot all hold. Levenberg-Marquardt proves it
(`Unsatisfiable`, a conflict named); DogLeg ends `NotConverged`, its
analysis inconclusive. The verdict about a sketch should not depend on the
algorithm. Desired: `Unsatisfiable` with a conflict under both. The expected
conflict set is not known from the construction, so only the status is held.

Fixed by plans/m7-drag-conflicts step 5 (ADR-0005, third amendment), with
the `*_dogleg` halves of `false-conflict-rigid-framework` and
`false-conflict-excludes-contradiction`: every filter test re-solves by
Levenberg–Marquardt whatever the caller's algorithm, the cause being
`staged-conflict-inconclusive`'s.
