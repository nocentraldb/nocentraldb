# 2026-11-24 · NOC-504 · The generic tools wait behind --generic-tools, and list_spaces is always listed

- Decided by: Carsten, 2026-11-24, on RFC 0017's open questions, through pm (NOC-504)
- Why: declared actions need a space id, and how fresh a space is belongs to the member, not to the app; the flag's name says what it adds.
- Instead of: list_spaces behind the flag too, which leaves the model no honest view of its spaces' freshness; or the flag named --dev, which says nothing of what it turns on.
- Topics: mcp, public-api, rfc-0017
