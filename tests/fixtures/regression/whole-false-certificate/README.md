# A whole-path conflict that holds together

Found by the property
`an_extra_distance_is_redundant_when_true_and_a_conflict_when_false`
(seed `aa67e2f4…`, shrunk): a rigid construction of distances on two fixed
points, plus `stretched`, a distance between two of its points at 1.2 times
their true separation. The conflict filter's first test drops `spared`, a
true distance, and re-solves the rest from the best effort. The whole path
stops at a stationary point there and certifies it (ADR-0005's certificate
is local), so it drops `spared` and names a set without it. The staged path
solves the same subset from the same start to the coordinates in
`expected.json`, which the checker accepts: every constraint but `spared`
holds there, so no conflict set can leave `spared` out. The staged path
names a set with it; staged and whole conflict sets differ.
The witness came from laman's staged solve, but its authority is the
independent checker's: the test holds only that `laman-check` accepts it.

Desired: every conflict set the whole path (`laman::reference::solve`)
names contains `spared` and `stretched`, under both algorithms. The
witness's check and the staged path's sets pass today.
