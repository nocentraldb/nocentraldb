# 2026-11-15 · NOC-326 · A device of the account signs off an erasure in a space of the account's agents

- Decided by: Carsten, 2026-11-15, on NOC-326 (Q4)
- Why: a space whose members are only agents of one account, such as an ad-hoc copy, has no second account, and an agent holds no account key. The flow of Q3 serves it: the agent makes the request, a device of the same account signs it off with the account key, and the shred is provisional (RFC 0008, "A provisional erasure in a space with one account"), finalized after 7 days by the SDK as before, or taken back.
- Instead of: a flow of its own for agents.
- Topics: sdk-rust, erasure, agents
