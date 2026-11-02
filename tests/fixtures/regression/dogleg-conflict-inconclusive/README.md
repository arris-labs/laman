# DogLeg leaves a conflict inconclusive

Six points, two fixed, eight distances: the distances of a rigid framework
built from true positions, plus one stretched by a factor of 1.2 (shrunk from
a property test, `an_extra_distance_is_redundant_when_true_and_a_conflict_when_false`).
The distances cannot all hold. Levenberg-Marquardt proves it
(`Unsatisfiable`, a conflict named); DogLeg ends `NotConverged`, its
analysis inconclusive. The verdict about a sketch should not depend on the
algorithm. Desired: `Unsatisfiable` with a conflict under both. The expected
conflict set is not known from the construction, so only the status is held.
