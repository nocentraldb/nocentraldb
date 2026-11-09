# 2026-11-09 · NOC-78 · A key envelope reads as its own control-body entry

- Decided by: coding agent (NOC-78)
- Why: the ticket asks that the envelope's place in the control body follow RFC 0008, so entry type 2 now reads as `Entry::KeyEnvelope`, with keys 8 to 11 through `KeyEnvelope::from_fields` (NOC-68). It follows NOC-67's reading rules: a missing or mistyped field is malformed, and a key from 12 to 63 makes the body not understood, so the change is parked. It still needs the admin right in `resolve`; its own rules (the recipient is a member and the generation exists in its causal past) need the generation graph and come with NOC-71.
- Instead of: keeping entry type 2 as `Entry::Other` and reading the envelope out of its raw fields in the checks, which would leave a key from 12 to 63 silently ignored.
- Topics: rfc-0008, envelope, encoding
