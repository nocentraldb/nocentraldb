# 2026-11-12 · NOC-258 · Of two shreds of one person key, the first not taken back takes effect

- Decided by: Carsten, 2026-11-12, on NOC-242 (B)
- Why: after a provisional erasure is taken back, the person must still be erasable in that space; a later shred of the same person key therefore takes effect, provisionally, with its own 7 days, and the shred taken back stays in the state as an earlier shred of the key, which the erasure audit lists as taken back.
- Instead of: the literal rule of RFC 0008 (the first shred in the order takes effect, the second changes nothing), under which a person could never be erased again in that space after a take-back.
- Topics: membership, erasure, shred, rfc-0008
- Supersedes: 2026-11-12-NOC-234-a-decision-on-a-later-shred-of-a-taken-back-key-changes-nothing.md
