# 2026-11-13 · NOC-291 · An agent that has an account of its own is not linked in v1, and `pair(Agent)` refuses it with notAllowed

- Decided by: pm (2026-11-13, on NOC-291, point 2): a safe subset until NOC-294 designs it; the owner answered only point 1
- Why: RFC 0010 has an agent keep its link statement and its invitations carry it, but neither invitation version nor the key record has a field for it. Most agents are new and have no account to link, so refusing is a safe subset; the design is a separate ticket, NOC-294. Agents that act for a person are paired directly with `PairKind::Agent`, and their certificate carries `kind = agent`, so they need no link.
- Instead of: designing the fields now, an optional link statement after the statement in invitation version 1 and in the key record (B), which means two format changes before beta.
- Topics: accounts, linking, pairing, safe-subset, rfc-0010
