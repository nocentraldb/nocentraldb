# 2026-11-08 · NOC-14 · Key 0 is "must understand"

- Decided by: pm
- Why: RFC 0002 names keys 1 to 63 "must understand" and 64 up "may ignore", and leaves key 0 out. A key the RFC does not place cannot be safe to ignore, so a header, payload or operation with key 0 is kept and not folded.
- Instead of: treating key 0 as "may ignore", as the first implementation did.
- Topics: change-format, log
