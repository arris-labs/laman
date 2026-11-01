# Provenance

Laman is written from scratch under MIT OR Apache-2.0.

- **SolveSpace** (`slvs`, GPLv3) and **FreeCAD's PlaneGCS** (LGPL) are read
  for their constraint vocabularies, their DoF and redundancy analyses and
  their known failure modes, and reimplemented against Laman's own model:
  no code, comments or identifiers carried over, no line-by-line
  translation. An ADR whose decision one of them informed names the source
  file or module read.
- Either may be **run** as a test oracle, out of process under
  `tools/oracle/`, only when analytic expectations run out (SEED.md §7). It
  is never a build, runtime or dev dependency of any crate here.
- **arrix**'s sketcher contributed experience only (SEED.md §8); its code
  is not read for copying.
- Published algorithms are implemented from the papers and cited where
  they are used: the (2,3) pebble game in `crates/laman/src/rigidity.rs`
  from Jacobs & Hendrickson (1997).
