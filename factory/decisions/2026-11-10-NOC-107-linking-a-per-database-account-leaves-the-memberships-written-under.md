# 2026-11-10 · NOC-107 · Linking a per-database account leaves the memberships written under it as they are

- Decided by: Carsten, 2026-11-10, on NOC-115 ("Die Vorschläge passen alle"), RFC 0010 Q9
- Why: Both account keys sign a link statement and the old key is deleted; memberships already written keep the old certificate, because RFC 0008's entries are fixed, which costs no rotation and changes no format. The fold and the relay keep counting the old account as its own, no weaker than anybody drawing a second account.
- Instead of: re-adding each such member with one rotation per space (Q9 B); a new membership action that replaces a member's certificate (Q9 C).
- Topics: rfc-0010, accounts, linking, membership
