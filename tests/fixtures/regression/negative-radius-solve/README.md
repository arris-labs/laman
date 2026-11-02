# A solve returns a negative radius that `apply` refuses

A circle of radius 0.001 at the origin and one of radius 3.03e91 centred
764.76 above it, `tangent` externally. The centres are 764.76 apart, so
the radii would have to sum to that. Both algorithms end `UnderConstrained`
with the small circle's radius at about -7.57e90 and no flip named, and
`Sketch::apply` refuses the solution (`DimensionOutOfRange`, `r >= 0`).
Found by the property `any_point_of_any_sketch_drags_to_any_targets` (its
drag session opens with this solve); the static solve alone shows it.
Desired: a returned solution applies back; whether that means a radius
held at 0 or a named flip and status is the fix's to decide, so only
`apply` is held. `expected.json`'s status and DoF are not asserted.
