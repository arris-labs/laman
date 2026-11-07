# A goal at 1e300 is called stationary before it moves

A point on a circle of radius 10 about a ground point, dragged toward
(-1e300, 3e299). The goal's residual is about 1e299 in normalized units, so
`norm(&g)` in `pursue` overflows to infinity and the stationary test
`|p| <= acceptance + GOAL_ROUNDING * |g|` passes at once: the frame ends
converged with the point where it started, (10, 0). Desired: the point ends
nearest the target, as it does at 1e30 and 1e150: (-10, 3) / hypot(1, 0.3).
Found by plans/m8-pursuit step 1, while testing the damping at f64's
extremes; the exit comes before the damping, so that change did not cause it.
