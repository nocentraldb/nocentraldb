# 2026-11-08 · NOC-23 · An erasure needs the sign-off of another account that holds an admin member in the space

- Decided by: Carsten (RFC 0008 decision M9)
- Why: an erasure cannot be undone, and a reader's account should not approve one. The sign-off's account must hold an admin member in the state of the shred entry's causal past and must not be the performer's account, which narrows RFC 0003's "an account that has a certified member in the space".
- Instead of: any account with a member in the space, other than the performer's (RFC 0003).
- Topics: rfc-0008, membership, erasure, sign-off

Later owner decisions refine this rule; they are in `docs/rfc/0008-membership.md`, "Roles and rights": accounts are compared by account key and every recovery key (`2026-11-11-NOC-196-the-sign-off-reads-every-certificate-for-recovery-keys.md`), a writer of another account may sign off when the performer's account is the only admin account (`2026-11-11-NOC-229-exactly-one-admin-account-is-counted-by-same-account-and-needs-one-admin.md`), and a space with one account gets a provisional erasure (NOC-230).
