# 2026-11-20 · NOC-378 · An RFC is decided when its Status line says so and names the owner with a date

- Decided by: pm, on NOC-378, after the owner asked that batch entries be split
- Why: the decision log holds single decisions under titles that state them, so no log heading marks a whole RFC as decided any more. The RFC's own status paragraph already says "Decided by the owner (Carsten, date)" or "Decided. … (Carsten, date)", and `scripts/rfc-status.test.mjs` reads it: such an RFC carries no draft number and no sentence that waits for its own merge.
- Instead of: keeping a batch heading "RFC <n>: …" or "The owner decides RFC <n>, …" in the log only so the test can find it, which lets tooling dictate the log's titles.
- Topics: rfc, tooling, gate, decision-log
- Supersedes: 2026-11-09-NOC-35-an-rfc-counts-as-decided-once-an-owner-entry-names-it.md
