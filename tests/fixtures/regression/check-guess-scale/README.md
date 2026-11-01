# Checker tolerance follows the guess

`laman_check::check(sketch, values)` takes its length tolerance from
`Tolerances::new(sketch)`, and so from the sketch's stored coordinates (the
solve's guesses), not from the coordinates it is checking. A point guessed
at (1e6, 0) gives the component a scale of 1e6 and a length tolerance of
1e-5. Checked at its known solution (1, 0) with 1e-6 added to x, the
length-1 `unit` distance therefore passes, about 1000 times looser than
the old absolute 1e-9 and 1e5 times looser than the tolerance at scale 1.

The solution comes from the construction: `origin` is fixed at (0, 0) and
`unit` is 1 along x. Desired: the checker's verdict at given coordinates
does not depend on the guesses. The same check of the sketch reseeded at
(1, 0) flags `unit`, and so must this one. ADR-0003 freezes a scale from
the input for the *solver*. Whatever the checker uses instead is decided
by the fix. Taking the larger of the guess scale and the solution scale
would not fix this case.
