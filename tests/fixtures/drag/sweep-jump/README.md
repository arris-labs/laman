# A frame that jumps an arc's sweep

An arc about the fixed origin from the fixed (1, 0) to a free end, drawn
at sweep 0.3 rad, the end pulled to (-1, 0.05) in one frame (ADR-0006's
sweep 0.3 to -3.2 frame: that target lies at -3.19 rad, or 3.09 rad the
other way round).

Solved at the target itself the end lands on the unit circle at the
nearest point to it, sweep 3.09. Tracking reads that as a move of +2.79 rad
(the nearest representative), not the -3.49 it is, so nothing flips; the
move exceeds `MAX_SWEEP_STEP` (π/4), so the frame is bisected without
naming a flip. With the end at e0 = (cos 0.3, sin 0.3) and the target t, a
try at `k` halvings pulls toward p = e0 + (t - e0) / 2^k and lands at
p / |p|:

| k | p | sweep | moves |
|---|---|---|---|
| 0 | t | 3.09 | 2.79: bisect |
| 1 | (-0.0223, 0.1728) | 1.70 | 1.40: bisect |
| 2 | (0.4665, 0.2341) | 0.465 | 0.165: the frame |

The frame is p / |p| = (0.89374490, 0.44857558) at sweep 0.465, inside
(0, 2π), on the short side of the target, no flip.
