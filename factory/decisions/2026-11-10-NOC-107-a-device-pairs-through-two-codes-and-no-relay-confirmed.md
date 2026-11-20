# 2026-11-10 · NOC-107 · A device pairs through two codes and no relay, confirmed by a scanned code or a full fingerprint

- Decided by: Carsten, 2026-11-10, on NOC-115 ("Die Vorschläge passen alle"), RFC 0010 Q5 and Q6
- Why: The new device shows its invitation, a device of the account answers with the certificate and, for a device, the account key sealed with HPKE; nothing passes the relay and it works in island mode. A device is confirmed only by a scanned code or a full fingerprint on both screens, because the answer carries the account key; an agent receives only a certificate, so any channel the person trusts will do. A short code is not enough, since an attacker can grind keys to match it.
- Instead of: one code plus a relay mailbox, or a short code with a PAKE through the relay (Q5 B, C); any channel for a device too (Q6 B).
- Topics: rfc-0010, accounts, pairing, crypto
