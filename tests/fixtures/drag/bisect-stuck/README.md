# A step that flips at every halving

The sketch of `bisect-keeps`, the tip pulled again and again to (-5, 1),
past the boundary x = 0. Each frame takes the first halving that stays on
the near side, with x(n+1) = x(n) + (-5 - x(n)) / 2^k for the smallest k
that keeps it positive:

| step | k | x |
|---|---|---|
| 1 | 2 | 1 |
| 2 | 3 | 1/4 |
| 3 | 5 | 11/128 |
| 4 | 6 | 53/8192 |
| 5 | none | 53/8192 |

At step 5 the tip is 53/8192 from the boundary: even the 8th halving
(5.0065 / 256 = 0.0196) crosses it. With the default eight bisections the
frame is the last solution unchanged and names the flip (`same-way`); the
session stays on its branch.
