# 2026-11-07 · NOC-6 · Unknown changes are parked and reported, and principle 2 binds members that understand every change they hold

- Decided by: Carsten (RFC 0002 decision D9, with the reading of principle 2 for parked changes)
- Why: GENESIS.md asks that old clients keep data they do not understand yet, so such a change is stored, forwarded, counted and reported in the SDK, and folded after an upgrade. A client that parks holds the same changes as an upgraded one but shows a different state, so principle 2 is read as "every member that understands all the changes it holds ends in the same state", and the parked changes are made visible.
- Instead of: refusing to sync a space until the client is upgraded.
- Topics: rfc-0002, log, convergence, principles

What parks and why is in `docs/rfc/0002-change-and-log-format.md`, "Versioning: old clients keep what they do not understand".
