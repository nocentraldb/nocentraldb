# 2026-11-09 · NOC-64 · A set or many-link field changes only through nc.add and nc.remove, never by writing a whole array

- Decided by: Carsten, 2026-11-09, in chat with pm, on #78 (RFC 0009 decision S4)
- Why: Basic values and links take plain values, and `null` clears them; a set takes `nc.add` and `nc.remove`. A whole array, which the SDK would turn into additions and removals, removes concurrent additions its author never saw, which breaks RFC 0004's add wins for the developer who expects "exactly these".
- Instead of: accepting a whole array for a set as well, with the SDK computing the difference (S4 B).
- Topics: rfc-0009, sdk, api, sets, merge-rule
