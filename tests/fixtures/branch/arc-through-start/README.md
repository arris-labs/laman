# An arc dragged through its start

An arc about the origin from `s` (10, 0) counter-clockwise to `e` (6, 8):
sweep `atan(4/3)`. Moving `e` to (6, −8) carries it clockwise past `s`.
Read in `[0, 2π)` the new sweep is `2π − atan(4/3)`, the long way round;
tracked from the last one it is `−atan(4/3)`, outside `(0, 2π)`: the arc
flipped (ADR-0006).
