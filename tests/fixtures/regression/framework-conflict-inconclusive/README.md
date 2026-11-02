# Both algorithms leave a conflict inconclusive

Like `dogleg-conflict-inconclusive`, shrunk from the same property test: a
rigid distance framework with one distance stretched by at least 1.2, so
the distances cannot all hold. Here Levenberg-Marquardt and DogLeg both end
`NotConverged` with no conflict named. Desired: `Unsatisfiable` with a
conflict, under both. Only the status is held (the conflict set is not
known from the construction).
