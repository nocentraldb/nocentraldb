# 2026-11-07 · no ticket · "Late" in principle 3 means lost to a change its author had not seen

- Decided by: Carsten
- Why: the time a change arrives differs per member, so storing it in the shared state would break principle 2; what every member can identify the same way is that a change lost to a change its author had not seen (RFC 0002, RFC 0004). GENESIS.md principle 3 and its no-go now say so.
- Instead of: "arrives late", which no converged state can record.
- Topics: principles, convergence
