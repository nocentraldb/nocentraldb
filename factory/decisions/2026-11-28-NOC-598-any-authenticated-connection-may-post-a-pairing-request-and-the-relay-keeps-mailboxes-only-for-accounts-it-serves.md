# 2026-11-28 · NOC-598 · Any authenticated connection may post a pairing request, and the relay keeps a mailbox only for accounts it serves

- Decided by: Carsten, 2026-11-28, on NOC-598 (option A)
- Why: a new device is a member of nothing, so the relay cannot check membership before it takes a request; what keeps a stranger out is the scan, or the typed words and the fingerprint the person compares. The relay keeps a mailbox only for an account on its allowlist, with an admission that holds (RFC 0015), or with a member in a space it holds, so it stores nothing for accounts that have nothing on it. Limits on open requests bound what a poster can fill.
- Instead of: posting only with a token an app's backend signs with an admission key, which keeps out strangers but not members who share a space with the account, and ties the mailbox to an app's backend; it can come later on top of this (B). Or a mailbox for any account, which lets any account anybody draws store records at the relay (C).
- Topics: pairing, relay, admission, rfc-0019, rfc-0007
