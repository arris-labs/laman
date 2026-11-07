# Coincident tangent contacts solved to 3e-5, a shared point to 1e-11

The slotted plate of `c3/slotted-plate` with each arc end tied to a line end
by `coincident` instead of a shared point. The solve is `FullyConstrained`
and the checker finds it clean, but the eight contact points (arc ends where
a tangent line meets a circle) come out up to 3.7e-5 from the closed-form
coordinates, against 1e-11 for the shared-point sketch.

Cause, found with `inspect`: a tangent contact is a double root. The solve
stops when residuals are within the length tolerance (about 1e-10 here), and
a point that lies on a circle and on its tangent moves along the circle by
the square root of that residual times the radius: 7.5e-11 gives 3e-5. The
shared-point sketch reaches the closed form because its path ends nearer
the root, not because the stop is tighter.

Desired: the same accuracy as the shared-point sketch (the default
coordinate tolerance), by a stop or a last step that does not leave a double
root at the square root of the residual. `c3/slotted-plate-coincident` holds
the profiles and DoF of the same sketch and skips the solved coordinates
until this is fixed.
