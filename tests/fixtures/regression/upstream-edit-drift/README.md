# An edit moves upstream coordinate bits

Three collinear points have y fixed at zero, point 0 grounded, and two
consecutive distances of 20. Their positive-x branch gives x = 0, 20, 40.
After solving and applying the result, change only the second distance to
22 with the same constraint id. The closed-form answer is x = 0, 20, 42;
point 1 is upstream of the edit and must keep its previously solved bits.
The ignored tests assert that desired guarantee, comparing before and
after bits directly, independently of the coordinate tolerance.

Found while adding M5's C2 shared runner on 2026-11-04. All three small
families passed edited coordinates, status, DoF and checker checks, then
failed upstream bit equality. Rectangle count 1 passed; count 2 failed.
Removing the rectangle height, sides, and horizontal constraints, fixing
y directly instead, retains the failure here. Removing either distance or
its endpoint removes the upstream/edit relationship, so this is the
smallest point-distance chain for that claim.

Measured on this fixture: LM point 1 x moves from 19.999999999999964 to
19.999999999999083 (248 ULP); DogLeg moves from 19.999999999999996 to
20 (1 ULP). Both solves in each pair are FullyConstrained, DoF 0,
checker clean, with no redundancy, conflict or flip. The evidence supports
whole-component numerical movement, rather than an incorrect closed-form
expectation or edit id: only the second distance changes and the first
still fixes x to 20. The text and SVG of the two-rectangle case and this
shrink were inspected under target/inspect during diagnosis.

Run both algorithms:

```sh
cargo test -p laman-debug --test solve_regression upstream_edit_drift
```

Fixed by the staged solve (plans/m6-stages step 4): the first distance is
a stage of its own, satisfied after the edit, so it takes no step and
point 1 keeps its bits; only the edited distance's stage moves.
