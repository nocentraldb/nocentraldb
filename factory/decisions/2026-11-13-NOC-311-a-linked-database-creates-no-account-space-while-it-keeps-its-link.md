# 2026-11-13 · NOC-311 · A linked database creates no account space while it keeps its link statement

- Decided by: pm, NOC-311, on the coding agent's proposal
- Why: `account::create` makes an account space when the database holds none of its current account and does not know it joined one; `joins_account` says so only until the database is closed. A linked database keeps its link statement beside its log (RFC 0010, "The link statement") until the relay holds it in the person's account space, and by then it holds that space. So while `pending_links` holds a statement, the database creates no account space, also after a reopen. This needs no new field or table and changes no format.
- What it leaves: a device paired without a link of its own keeps nothing that survives a reopen, so its window stays until the owner decides how a database remembers that it joined an account (the needs-owner ticket, options C1 to C3).
- Instead of: waiting for the owner's answer for the linked case too, which the store already beside the log answers.
- Topics: sdk-rust, accounts, linking
