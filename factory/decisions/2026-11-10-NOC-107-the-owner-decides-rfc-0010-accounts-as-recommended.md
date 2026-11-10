# 2026-11-10 · NOC-107 · The owner decides RFC 0010, accounts, Q1 to Q12 as recommended

- Decided by: Carsten, 2026-11-10, on NOC-115 ("Die Vorschläge passen alle")
- Why: a recovery key that only the recovery code holds is the account's identity and signs an account key statement, so a lost device is answered with a new account key; the code is twelve BIP-39 words and restores the account, not the spaces; an account space holds the account's records, never a readable key; a device pairs with two codes and no relay, confirmed by a scanned code or a full fingerprint; a new device joins the spaces its account administers; each app's database keeps its own account key; linking leaves written memberships as they are; the SDK names, the redacted recovery code object and a 60-digit fingerprint follow.
- Instead of: the alternatives each decision Q1 to Q12 of RFC 0010 lists, among them the account key as the identity (Q1 A), a recovery member or a key backup at the relay (Q3 B, C), a relay mailbox or a PAKE for pairing (Q5 B, C), and re-adding or re-certifying linked members (Q9 B, C).
- Topics: rfc-0010, accounts, keys, crypto, api
