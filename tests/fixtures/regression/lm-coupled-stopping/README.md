# Coupled stopping error

The original LM stop at the geometric acceptance threshold returned E2.y =
35.00000000110096, outside the independently specified coordinate bound
1e-9, although every original constraint passed the checker. The expected
geometry is the roadmap's analytic slotted plate, not solver output.

Shrinking by grounding each nonempty subset of P1, P2, P3, Mb and Mt at its
analytic position (and removing newly fixed-only constraints) eliminated
the failure in all 31 cases. The plate/axis/slot coupling is retained here.
The engine now polishes new iterates to one tenth of geometric acceptance;
already satisfied starts still return without moving any coordinate bits.
