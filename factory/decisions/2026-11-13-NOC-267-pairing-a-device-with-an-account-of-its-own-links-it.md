# 2026-11-13 · NOC-267 · Pairing a device that has an account of its own links that account

- Decided by: coding agent (NOC-267), carrying out RFC 0010 and NOC-291
- Why: with the link statement and its store built, `Account::pair` accepts a certified (version 1) invitation from a device: the answer carries the account's half of the link, and `complete_pairing` gives the `AccountLinked` notice besides `DevicePaired`. This lifts the refusal of 2026-11-12, "Pairing refuses a certified invitation until linking is built", for devices. An agent that has an account of its own is still refused with `notAllowed`, as RFC 0010 says for v1 (NOC-291 point 2, deferred).
- Instead of: keeping the refusal for devices, which would leave a database drawn with its own account unable to join the person's account.
- Topics: sdk-rust, accounts, pairing, linking
