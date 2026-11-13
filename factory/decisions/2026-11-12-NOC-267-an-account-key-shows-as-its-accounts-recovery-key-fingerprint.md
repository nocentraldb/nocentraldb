# 2026-11-12 · NOC-267 · A bare account key shows as the fingerprint of its account's recovery key

- Decided by: pm, NOC-267, on the coding agent's proposal
- Why: the fingerprint of an account is over its recovery key once its certificates carry a statement (RFC 0003, "Account fingerprint"), but an erasure's sign-off and a bad person-key record's writer name only an account key. The SDK shows them as the fingerprint over the recovery key that a current member's certificate of that account states, and over the account key only when no certificate states one, so that one account never shows as two fingerprints.
- Instead of: showing the fingerprint over the bare account key, which differs from the fingerprint the same account's invitations show.
- Topics: sdk-rust, accounts, fingerprint
