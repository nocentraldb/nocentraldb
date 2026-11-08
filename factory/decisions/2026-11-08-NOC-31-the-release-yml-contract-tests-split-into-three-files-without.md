# 2026-11-08 · NOC-31 · The release.yml contract tests split into three files without a new red test

- Decided by: coding agent (NOC-31)
- Why: the split is a pure refactor, as FACTORY.md asks for a file past the limit: the 24 existing checks and their 110 mutants are the tests, and they ran green before and after. The check bodies were cut from the old file by line range, not retyped; the PR compares the test and mutant names before and after. `scripts/release-workflow.test.mjs` keeps the rules for every job, so the references to it in `release.yml` and `moon.yml` stay true and neither file changes; the alpha job and the publish side get their own files, and the helpers move to `scripts/lib/release-workflow.mjs`.
- Instead of: a new test that holds these files below 700 lines, which would be one file's copy of a repository-wide limit check that belongs in its own ticket.
- Topics: release, testing, code-size
