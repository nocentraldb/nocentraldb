# 2026-11-08 · no ticket · No code or test file grows past 700 lines

- Decided by: Carsten
- Why: a code or test file longer than that is a sign that the design mixes concerns and tasks, so it is split along them. The limit lives in CONFIG.md, the rule for coding agents in FACTORY.md, and pm checks it in every review.
- Instead of: no limit, which lets one file collect several concerns.
- Topics: factory, code-size
