# 2026-11-15 · NOC-329 · The owner reviews the texts from git diff, and no entry records the release it first appeared in

- Decided by: Carsten, 2026-11-15, on NOC-329 (review from git diff, no since field)
- Why: git already knows when each text appeared and changed. `pnpm texts`, with no arguments, finds the last release tag by itself, the newest beta or stable tag, and with no such tag counts every text as new; it runs `git diff` on `texts/texts.json` against that tag and prints, for each changed entry, its id, kind, code and `when`, and the old and the new text, with new and removed entries marked. So that the diff reads well, the gate `repo:texts` keeps the entries sorted by id and the file written one field per line, as `JSON.stringify(file, null, 2)` writes it. Slice T4 is that script and its step in docs/release-management.md, with no change to the release workflow.
- Instead of: a `since` field in each entry, which the release workflow's commit-back would stamp with the tag's version, which needed a workflow change and repeated what git records.
- Topics: texts, release, gates
