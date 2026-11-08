# 2026-11-08 · NOC-43 · The gate rejects a code or test file longer than 700 lines

- Decided by: Carsten
- Why: a check by hand in every review is easy to miss and reaches a coding agent only after its PR is open; as the gate task `repo:file-size`, CI rejects a file past the limit and the agent sees it when it runs `pnpm gates`. The limit stays in CONFIG.md, which the check reads, and there is no allowlist: a longer file is split by concern. This changes how "No code or test file grows past 700 lines" is enforced; the limit itself still holds.
- Instead of: keeping the limit a review rule that pm checks by hand.
- Topics: factory, code-size, gates
