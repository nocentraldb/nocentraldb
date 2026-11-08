# 2026-11-08 · NOC-21 · The contributor-kit check holds the README's task table to the moon config

- Decided by: coding agent (NOC-21)
- Why: the ticket asks to extend the check only if it already covered the task table; it did not, so a new task could again go unlisted, as the three wasm32 tasks did. The check now reads every project's `gates` deps from `.moon/tasks/<language>.yml` and its own `moon.yml`, plus the tasks of another project those deps need (`repo:wasm-runner`), and fails when the project's row of the table, or the README's block of uncached `moon run` commands, does not name one (the block was added to the ticket by pm). `repo:test` takes the moon config files as inputs for it.
- Instead of: a test that names only the three wasm32 tasks, which would not catch the next gate task the README forgets.
- Topics: readme, gates, testing
