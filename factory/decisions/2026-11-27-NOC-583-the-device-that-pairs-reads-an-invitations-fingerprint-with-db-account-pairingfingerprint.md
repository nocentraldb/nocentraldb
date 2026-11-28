# 2026-11-27 · NOC-583 · The device that pairs reads an invitation's fingerprint with `db.account.pairingFingerprint(invitation)`

- Decided by: Carsten (2026-11-27, on NOC-583, option B)
- Why: RFC 0010's pairing step 2 lets a person confirm a new device by comparing its full fingerprint on both screens, and `nc.invitation(text)` stays synchronous without a core (NOC-305), so it cannot compute one. The device that pairs always has an open database: `await db.account.pairingFingerprint(invitation)` returns the core's `Invitation::fingerprint()` through the bridge, the member's own for a version 2 invitation and the account's for a certified one, so both SDKs show the same digits from one implementation.
- Instead of: computing the digits in TypeScript inside `nc.invitation`, which needs an SHA-256 library as the SDK's first runtime dependency (A); or pairing by scanned code only in TypeScript (C).
- Topics: pairing, fingerprint, sdk-ts, accounts, rfc-0009, rfc-0010
