# Every conflict excludes the contradiction

Seven points, two fixed, ten distances. All but `stretched` are read off one
set of true positions (the guesses are near them); `stretched` asks for 255
between a point held within 15.4 of the origin and one 118 from it, which
cannot be. Every genuine conflict contains `stretched`, since the other
nine hold together. Levenberg-Marquardt reports at least one conflict and
none of them contains it: false conflicts. Found by the property
`an_extra_distance_is_redundant_when_true_and_a_conflict_when_false`
(shrunk by deleting constraints and entities while it failed).
Desired: every reported conflict contains `stretched`.
