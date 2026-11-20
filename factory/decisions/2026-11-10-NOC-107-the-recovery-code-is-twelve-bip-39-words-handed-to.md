# 2026-11-10 · NOC-107 · The recovery code is twelve BIP-39 words, handed to the app once, and restores the account but not its spaces

- Decided by: Carsten, 2026-11-10, on NOC-115 ("Die Vorschläge passen alle"), RFC 0010 Q2, Q3 and Q11
- Why: Twelve words encode 128 random bits, the strength of Ed25519, and are copied by hand with fewer mistakes. The code restores the account; spaces come back when their other members add the new device, because anything more would let a leaked code read everything. The SDK hands the app the code once as an object whose string, JSON and debug forms are redacted, the app shows it and calls `confirm()`, and the device then forgets it.
- Instead of: 24 words of the seed, or 26 base32 characters (Q2 B, C); a recovery member in every space, or a backup of space keys at the relay (Q3 B, C); a screen the SDK ships, or the SDK storing the code in the password manager (Q11 B, C).
- Topics: rfc-0010, accounts, recovery, keys
