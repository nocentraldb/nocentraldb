# 2026-11-23 · NOC-181 · The Node SDK ships by 2027-01-15, after the hardening work

- Decided by: Carsten, 2026-11-23, in the pm session
- Why: a consumer's AI service waits for the TypeScript SDK on Node instead of building a Rust bridge, so the Node SDK gets a date. It takes a few days of work and is ranked after the work to complete, check and harden NoCentralDB, which stays first.
- Instead of: leaving the Node SDK without a date until the hardening phase ends around April 2027.
- Topics: sdk-node, roadmap, priorities
