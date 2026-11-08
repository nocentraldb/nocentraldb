# 2026-11-07 · NOC-13 · The RFC 0007 test lives in its own file and needs no moon.yml change

- Decided by: coding agent (NOC-13)
- Why: `scripts/rfc-0007.test.mjs` matches the existing `scripts/*.test.mjs` pattern of `repo:test`, so it runs in `pnpm gates` without touching `moon.yml`, which the RFC 0002 PR also edits.
- Instead of: adding RFC 0007 to a shared RFC test file and conflicting with the other open RFC PRs.
- Topics: testing, rfc-0007
