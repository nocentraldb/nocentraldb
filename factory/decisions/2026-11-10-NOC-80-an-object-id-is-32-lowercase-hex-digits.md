# 2026-11-10 · NOC-80 · An object id is a string of 32 lowercase hex digits in TypeScript

- Decided by: Carsten
- Why: the owner followed the recommendation on PR #91. The hex string is the form the lock already writes `model.id` in, an id is never typed by a person, and a string can be a map key, compared with `===` and logged. RFC 0009 Amendment 3 records it.
- Instead of: a base64url string (shorter, a second encoding), a `Uint8Array` (no map key, no `===`), or a branded string type (a new public name).
- Topics: sdk, api, types, rfc-0009
