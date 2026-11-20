# 2026-11-13 · NOC-288 · A device waiting to be paired shows a member fingerprint, SHA-256 over "nocentraldb/member-fingerprint" and its member id

- Decided by: Carsten (2026-11-13, on NOC-288, option A)
- Why: RFC 0010's pairing step 2 confirms a device by a scanned code or a full fingerprint compared on both screens, but a new device has no account, so the account fingerprint over the recovery key does not exist for it. The member fingerprint is shown like the account fingerprint, 60 digits in twelve blocks of five; its own context string means it can never be confused with an account fingerprint over the same bytes. The new device shows it under its invitation, the account's device computes it from the scanned or pasted invitation, and `Invitation::fingerprint()` returns it for a version 2 invitation.
- Instead of: reusing the account fingerprint's context string over the member's signing key (B), so that two different things look the same to a person comparing digits; or no fingerprint for a waiting invitation, pairing by scanning only (C), which rules out two devices that cannot scan each other.
- Topics: pairing, fingerprint, accounts, crypto, rfc-0010
