# 2026-11-08 · NOC-23 · Compaction keeps every control change of a space forever

- Decided by: Carsten (RFC 0008 decision M16)
- Why: the membership state is a function of the whole set of control changes, and a snapshot that every member can check is a design of its own. The cost is small: a membership entry is about 300 bytes and an envelope about 150, so a space with 100 removals keeps under 1 MB of control changes on every member and on the relay.
- Instead of: compaction replacing old control changes by a signed snapshot of the control state.
- Topics: rfc-0008, membership, log
