# 2026-11-09 · NOC-35 · RFC 0004 Amendment 2: a create that carries key 5 is malformed

- Decided by: Carsten
- Why: a create (operation 0) has no arguments, so one that carries key 5 is parked with the reason "malformed arguments", as a delete that carries key 3 already is: one encoding per create, decided from the bytes alone on every member. NOC-39 (#66) built it that way; the owner made it part of RFC 0004 (in chat with pm, 2026-11-09).
- Instead of: ignoring key 5 on a create, which gives one create two encodings.
- Topics: rfc-0004, wire-format, parking
