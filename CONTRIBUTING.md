# Contributing

## Reports are the most useful contribution

A sketch that makes laman panic, return a wrong solution or wrong
diagnostics is worth more than any patch. Open an issue with the shortest
input that shows it, the full output, and what you expected. A report that
reproduces becomes a fixture in the test corpus and stays there once fixed.

## Code

Laman is developed under written rules that every change follows. Read
them before a pull request:

- [AGENTS.md](AGENTS.md): the project in one page, its setup, and where the
  rest lives;
- [.agents/rules/](.agents/rules/): the solver rules (no panics, statuses
  not errors, determinism, the independent checker), git, and the docs
  lifecycle;
- [PROVENANCE.md](PROVENANCE.md): code from other solvers is read, never
  copied.

Set up the hook once per clone (`git config core.hooksPath .githooks`). For
anything larger than a fix, open a discussion first: the design may already
be settled in `docs/adr/`.

## Licence

Unless you say otherwise, any contribution you submit for inclusion in
laman is dual-licensed as MIT or Apache-2.0, as the project is, without
any additional terms or conditions.
