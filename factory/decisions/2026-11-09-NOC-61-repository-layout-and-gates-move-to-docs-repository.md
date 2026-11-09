# 2026-11-09 · NOC-61 · The repository layout, prerequisites, gates and package names move from the README to docs/repository.md

- Decided by: coding agent, within the ticket's choice of CONTRIBUTING.md or a new docs/repository.md
- Why: the README now speaks to a developer deciding whether to care about NoCentralDB. The moved sections are reference for contributors and are long; CONTRIBUTING.md says what a contribution needs and links them, as the README's Contributing section does. The text moved unchanged except for its relative links. The contributor-kit tests that check the task table and the uncached gate commands, and the decisions test that checks the layout's `factory/` line, now read docs/repository.md.
- Instead of: appending the sections to CONTRIBUTING.md, which would bury its short list of what we ask for under about 150 lines of tooling detail.
- Topics: readme, docs, contributing
