# A solve returns a negative radius that `apply` refuses

A circle of radius 0.001 at the origin and one of radius 3.03e91 centred
764.76 above it, `tangent` externally. The centres are 764.76 apart, so
the radii would have to sum to that. Both algorithms end `UnderConstrained`
with the small circle's radius at about -7.57e90 and no flip named, and
`Sketch::apply` refuses the solution (`DimensionOutOfRange`, `r >= 0`).
Found by the property `any_point_of_any_sketch_drags_to_any_targets` (its
drag session opens with this solve); the static solve alone shows it.
Desired: a returned solution applies back, the small radius held at 0
(ADR-0010), `UnderConstrained` with DoF 5 (six parameters, one tangency;
`expected.json`'s first count of 3 was wrong), the checker clean but for
the zero radius it names degenerate. Fixed by the radius bound
(plans/m7-drag-conflicts step 1).
