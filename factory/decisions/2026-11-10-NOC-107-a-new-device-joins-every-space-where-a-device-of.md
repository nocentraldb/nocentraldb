# 2026-11-10 · NOC-107 · A new device joins every space where a device of its account is admin; the app asks other admins for the rest

- Decided by: Carsten, 2026-11-10, on NOC-115 ("Die Vorschläge passen alle"), RFC 0010 Q7
- Why: A person expects their new phone to show their data, so the SDK adds it with the role the account's devices hold and returns one notice per space; it lists the other spaces, whose admins add the device by its invitation. Other admins' SDKs never admit it on their own, because a certificate must not admit a member without an admin deciding.
- Instead of: no space automatically (Q7 B); every space, admitted automatically by other admins' SDKs (Q7 C).
- Topics: rfc-0010, accounts, membership, pairing
