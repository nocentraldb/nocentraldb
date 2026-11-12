# 2026-11-12 · NOC-234 · After a take-back, a later shred of the same person key changes nothing, as RFC 0008 reads

- Decided by: pm, 2026-11-12, on NOC-234 (option A; the gap is NOC-242 for the owner, B recommended)
- Why: RFC 0008, step 5 says that of two accepted shreds of one person-key id the first in the order takes effect and the second changes nothing; the core builds the text as it stands, so `PersonKey` keeps the first shred, and an erasure decision on any other shred of the id changes nothing. The gap: once a provisional shred is taken back, the person cannot be erased again under the same person key.
- Instead of: B, the first shred in the order that is not taken back takes effect, so a later shred erases again; it changes RFC 0008's text, so it waits for the owner on NOC-242.
- Topics: membership, erasure, shred, rfc-0008
