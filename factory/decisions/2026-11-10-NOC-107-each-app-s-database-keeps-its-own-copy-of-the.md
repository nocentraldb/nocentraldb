# 2026-11-10 · NOC-107 · Each app's database keeps its own copy of the account key, so a person pairs each app once

- Decided by: Carsten, 2026-11-10, on NOC-115 ("Die Vorschläge passen alle"), RFC 0010 Q8
- Why: One database is one member (RFC 0006), and phones and browsers keep apps apart, so sharing a keystore entry where a desktop platform allows it would make accounts behave differently by platform, which principle 7 rules out.
- Instead of: one account key per device where the platform allows it (Q8 B).
- Topics: rfc-0010, accounts, keys
