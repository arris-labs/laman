# A satisfied status at a tangent the checker cannot measure

A circle of radius 0.001 centred on `a`, a line from `a` to a point
1.04e-209 away along x, and `tangent` between them: the line runs through
the centre and has collapsed to a point. Levenberg-Marquardt ends
`UnderConstrained` with the radius at 3.7e-14; the checker reports both
entities degenerate and the tangency violated with a NaN measure. Found by
the property `any_well_formed_sketch_solves_to_a_complete_answer` at 20000
cases (plans/m6-stages step 2); the static solve alone shows it, and the
staged and whole analyses agree on it. Desired: a satisfied status
(`FullyConstrained`, `UnderConstrained`) has no violation the checker
reports. Only that is held; `expected.json`'s status and DoF (5 parameters,
one equation) are not asserted. Same family as `drag-negative-radius`.
