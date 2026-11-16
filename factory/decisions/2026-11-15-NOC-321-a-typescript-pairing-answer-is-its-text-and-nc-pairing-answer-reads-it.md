# 2026-11-15 · NOC-321 · A TypeScript pairing answer is its text, and nc.pairingAnswer reads it

- Decided by: Carsten, 2026-11-15, on NOC-321 (A)
- Why: RFC 0009 makes `PairingAnswer` public data with a text form, as `Invitation` is, but names no TypeScript way to read one back. `PairingAnswer` is an exported type with `toString()`, and `nc.pairingAnswer(text)` reads it synchronously: it checks only the prefix `ncdb-pair-1:` and unpadded base64url, failing with `invalidValue`, as `nc.invitation` does (NOC-305); `completePairing` checks the rest. RFC 0009's "Accounts and devices" says so.
- Instead of: `completePairing` taking the text directly, with no `nc.pairingAnswer` (option B).
- Topics: sdk-ts, accounts, pairing
