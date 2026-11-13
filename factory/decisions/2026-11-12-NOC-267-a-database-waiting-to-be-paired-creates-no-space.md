# 2026-11-12 · NOC-267 · A database waiting to be paired creates no space

- Decided by: pm, NOC-267, on the coding agent's proposal
- Why: a genesis carries its creator's certificate (RFC 0008), and a database opened with `OpenOptions::pairing()` has none until its pairing completes, so `spaces().create()` fails with `notAllowed` until then; an admin's `members.add` already refuses its version 2 invitation with `invalidCertificate` (RFC 0003, "Invitation").
- Instead of: creating the space under an account of its own drawn for the purpose, which would make the database an account it then has to link into the one it pairs with.
- Topics: sdk-rust, accounts, pairing
