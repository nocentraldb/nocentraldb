# 2026-11-12 · NOC-267 · Pairing refuses a certified invitation until linking is built

- Decided by: pm, NOC-267, on the coding agent's proposal
- Why: pairing a database that has an account of its own links that account (RFC 0010, "Linking accounts drawn per database"), and the link statement it makes belongs in the account space, which the second slice of NOC-267 writes to, with the automatic adds of a new device. Until then `Account::pair` refuses a version 1 invitation with `notAllowed`, and `complete_pairing` refuses an answer of another kind than the database's with `invalidCertificate`, so no account is linked halfway.
- Instead of: linking now and dropping the link statement, which would leave the old account's memberships without the statement that shows them as linked.
- Topics: sdk-rust, accounts, pairing
