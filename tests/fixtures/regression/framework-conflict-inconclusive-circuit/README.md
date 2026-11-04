# A tiny stretched framework the solve leaves inconclusive

Two fixed points, two free points each tied to both fixed ones (four
distances), and a fifth distance between the free points stretched by a
part in fifty thousand over the length the construction gives (97.6901 true,
97.6901468 set). The four fix both free points, so the fifth cannot hold:
the one conflict is all five distances, known from the construction alone.
Levenberg-Marquardt ends `NotConverged` with no conflict named (the
stretched solve of `solved.rs`'s circuit property, found at
`PROPTEST_CASES=20000`, seed `5994c596…`); its `solved.proptest-regressions`
seed is not kept while this is ignored. Same family as
`framework-conflict-inconclusive`, whose stretch is large; here it is a
hair. Desired: `Unsatisfiable` with the five as the set, under both
algorithms.
