# Satisfied solve the checker cannot measure

A point 2e303 from a short line, constrained onto it. The solver moves the
line's far end under the point and reports `UnderConstrained`, but the
independent checker returns NaN for `point_on_line` at the solution
(products of 1e303 coordinates overflow to infinity), so a satisfied
status is not confirmed by the checker. Found by the property
`any_well_formed_sketch_solves_to_a_complete_answer` at magnitudes past
1e154. Desired: the checker measures the solution (finite, within
tolerance). Which side changes (checker scaling, or the solver stopping
where the checker can measure) is for the fix to decide.
