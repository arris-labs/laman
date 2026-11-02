# A flipping step, bisected

A line from the fixed (0, 1) to a free tip, held parallel to a fixed
horizontal line: the tip slides along y = 1, and crossing x = 0 reverses
the line, the constraint's other root (`dot` of the unit directions from 1
to -1). The tip starts at x = 3.

By construction: the first target (-1, 1) lies past the boundary, so the
step flips; its first halving, x = 3 + (-1 - 3) / 2 = 1, does not, and is
the frame. The second target (2, 1) is on the same side: no bisection.
