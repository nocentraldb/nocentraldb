# 2026-11-20 · NOC-362 · Header and payload keys 0 to 63 are must-understand

- Decided by: Carsten, 2026-11-20, on NOC-362 ("Ja zu den Empfehlungen"); the rule pm set for key 0 on 2026-11-08 (2026-11-08-NOC-14-key-0-is-must-understand.md), now in RFC 0002
- Why: RFC 0002 placed keys 1 to 63 and 64 up and left key 0 out. A key the RFC does not place cannot be safe to ignore, and the core already keeps a change with an unknown key 0 and does not fold it.
- Instead of: treating key 0 as may-ignore.
- Topics: wire-format, rfc-0002
