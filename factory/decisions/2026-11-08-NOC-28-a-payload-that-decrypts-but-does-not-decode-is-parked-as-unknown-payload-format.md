# 2026-11-08 · NOC-28 · A payload that decrypts but does not decode is parked as "unknown payload format"

- Decided by: coding agent (NOC-28), flagged to pm because the amendment's table has no row for it
- Why: the format-1 decoder needs keys 1 to 4, and a newer payload format may not have them, so a payload this core cannot read is most likely one it does not understand yet. Parking keeps it and folds it after an upgrade; the signature already showed that a member wrote it.
- Instead of: rejecting it as "undecryptable", which would claim the AEAD failed, or adding a reason the RFC does not name.
- Topics: rfc-0002, log, change-format
