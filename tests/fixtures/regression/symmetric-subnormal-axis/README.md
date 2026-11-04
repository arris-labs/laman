# Satisfied solve the checker cannot measure (subnormal axis)

Two points symmetric about the line joining them, one fixed in y at a
subnormal height (7.7e-309). The solve reports `UnderConstrained`, but the
independent checker returns NaN for `symmetric` and flags the axis as a
zero-length line. Found by `any_well_formed_sketch_solves_to_a_complete_answer`
at `PROPTEST_CASES=20000`. Same family as `extreme-point-on-line`.
Desired: the checker measures the solution (finite, within tolerance).
