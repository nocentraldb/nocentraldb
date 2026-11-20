# 2026-11-10 · NOC-107 · An account space holds the account's devices, spaces and links, and never a key its members can read

- Decided by: Carsten, 2026-11-10, on NOC-115 ("Die Vorschläge passen alle"), RFC 0010 Q4
- Why: Without a shared record each device knows only what it did itself, so `removeMember` could not find a lost device's spaces and a new device could not learn which to ask for. It is an ordinary RFC 0008 space with a model the SDK ships; a new account key reaches each device wrapped to its agreement key, because a readable key would reach every later member through the key chain and never be forgotten.
- Instead of: no shared record (Q4 B).
- Topics: rfc-0010, accounts, spaces
