# 2026-11-08 · NOC-43 · The file-size check knows code by extension and skips only node_modules/ and target/

- Decided by: coding agent (NOC-43)
- Why: the check reads the files git tracks and keeps those ending in `.rs`, `.ts`, `.tsx`, `.js`, `.mjs`, `.cjs`, `.sh` or `.py`; lock files and docs have none of these extensions, so they drop out without a rule of their own. It skips `node_modules/` and `target/` at any depth, the only places generated code lands today. A generated file outside them, such as `packages/sdk/etc/api-report.d.ts` (2 lines today), is checked like any other; if one ever grows past the limit, pm decides about it, as the ticket says.
- Instead of: skipping files that carry a `@generated` marker, which would let a hand-written file opt out of the limit, an allowlist the ticket rules out.
- Topics: gates, code-size
