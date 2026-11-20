# 2026-11-10 · NOC-107 · The account's identity is a recovery key that only the recovery code holds, shown as a 60-digit fingerprint

- Decided by: Carsten, 2026-11-10, on NOC-115 ("Die Vorschläge passen alle"), RFC 0010 Q1 (B) and Q12
- Why: The account key lives on every device, so as the identity a lost device would sign in the person's name for good; a recovery key no device keeps signs an account key statement naming the current account key and a number, so a lost device is answered with a new account key and the fingerprint stays. The fingerprint is SHA-256 of the recovery key under its own context, 60 digits in twelve blocks of five, which read aloud the same in every language.
- Instead of: the account key as the identity (Q1 A); 16 bytes of the hash in hex, or words from the recovery code's list, which invites confusing the two (Q12 B, C).
- Topics: rfc-0010, accounts, keys, crypto, fingerprint
