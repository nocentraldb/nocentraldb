# 2026-11-15 · NOC-320 · A sub-agent's database has no account of its own

- Decided by: Carsten, 2026-11-15, on NOC-320 (Q11)
- Why: a sub-agent never holds the account key (RFC 0010). Its database has no account key, no recovery code and no account space, and cannot pair or recover. It learns its certificate from the add that admits it, as a paired device does from the fold, and keeps it. A sub-agent may add sub-agents of its own: the core accepts a parent of kind 3.
- Instead of: an account drawn for the sub-agent, which would attribute it to an account of its own and not to its parent's.
- Topics: sub-agents, accounts
