# 2026-11-09 · NOC-40 · The writer's model is payload key 64, one map of model id and model version

- Decided by: coding agent (NOC-40), accepted by pm in its review of #67; the owner signs off the format by merging RFC 0002 Amendment 5
- Why: RFC 0005's D10 A and D13 B ask for one "may ignore" payload key (64 and up) holding the writer's `model.id` and model version; 64 is the first key of that range, and a map `{1: 16-byte model id, 2: uint version}` keeps both together, has the shape of the format's other maps (the sealed personal value) and leaves room for a later field. The model id is the 16 raw bytes, as in the genesis's key 13 (RFC 0008, M26).
- Instead of: two payload keys (one could arrive without the other), a two-element array (no room for a later field), or the 32 hex digits as text (twice the bytes).
- Topics: rfc-0002, rfc-0005, wire-format, model
