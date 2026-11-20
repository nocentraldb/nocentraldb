# 2026-11-10 · NOC-107 · Pairing, linking and recovery use nc.open({ pairing }), db.account.pair, completePairing and nc.open({ recover })

- Decided by: Carsten, 2026-11-10, on NOC-115 ("Die Vorschläge passen alle"), RFC 0010 Q10
- Why: `nc.open({ pairing: "request" })` and `db.invitation()` on the new device, `db.account.pair({ kind, invitation })` on a device of the account, `db.account.completePairing(answer)` back on the new one, and `nc.open({ recover: words })`; linking is pairing a database that already has data, so one flow and one set of names cover both.
- Instead of: the same names plus a separate `db.account.link(answer)` (Q10 B).
- Topics: rfc-0010, rfc-0009, accounts, pairing, public-names

The Rust forms and later names stand in `docs/rfc/0009-sdk-api.md`, "Accounts and devices".
