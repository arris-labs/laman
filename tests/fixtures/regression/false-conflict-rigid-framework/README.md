# A conflict that excludes the contradiction

Six points, two fixed, eight distances read off true positions (the guesses
are those positions), plus `stretched`, a second distance between the
points `stated` already fixes, at a different length. The only inclusion-
minimal conflict is {`stated`, `stretched`}. Levenberg-Marquardt names it
and also names a second set of the eight true distances, all satisfiable
together at the guesses: a false conflict. Found by the property
`an_extra_distance_is_redundant_when_true_and_a_conflict_when_false`.
Desired: exactly one conflict, {`stated`, `stretched`}, under both
algorithms.
