# A drag step returns a negative radius that `apply` refuses

A line `a`-`b`, a circle centred on `a`, and `tangent` between them: the
centre lies on the line, so the only radius that holds is 0 (a degenerate
sketch by construction, found by the property
`any_point_of_any_sketch_drags_to_any_targets`). Dragging `a` to (0, 0)
under DogLeg ends with the circle's radius at -2.55e-19, inside every
tolerance, but `Sketch::apply` refuses a negative radius
(`DimensionOutOfRange`), so `DragSession::finish`'s solution does not apply
to the sketch it was opened on. Desired: every solution a session returns
applies back (a radius that rounds below zero is not returned negative).
Only that is held; `expected.json`'s status and DoF are the sketch's count
(5 parameters, the tangency pins the radius at 0) and are not asserted.
Levenberg-Marquardt happens to land on +3.7e-14 here.
