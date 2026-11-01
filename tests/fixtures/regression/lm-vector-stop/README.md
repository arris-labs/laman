# Vector stopping error

Each initial coordinate misses the origin by 0.9 times the absolute floor,
but their Euclidean distance is sqrt(2) times that miss, above the floor.
A solver that tests the two scalar equations separately returns zero-step
success while the independent checker rejects `origin`. The fixed point's
known solution is (0, 0); geometric vector acceptance must trigger a step.
