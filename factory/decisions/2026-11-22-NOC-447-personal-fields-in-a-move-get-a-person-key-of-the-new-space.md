# 2026-11-22 · NOC-447 · A move seals personal fields again under a person key of the new space, for the same person id, built after the rest

- Decided by: Carsten, 2026-11-22, on RFC 0016's questions, through pm (NOC-449)
- Why: one key per person per space holds, and the person id carries over, so one erasure still reaches both spaces; until it is built the SDK refuses to move an object that holds a personal value.
- Instead of: leaving personal fields behind, or never moving an object that holds one.
- Topics: move, personal-fields, crypto, rfc-0016
