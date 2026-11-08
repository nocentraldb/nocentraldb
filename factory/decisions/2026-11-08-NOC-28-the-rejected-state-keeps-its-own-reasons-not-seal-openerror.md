# 2026-11-08 · NOC-28 · The rejected state keeps its own reasons, not seal's OpenError

- Decided by: coding agent (NOC-28), at pm's question after NOC-15 merged
- Why: `RejectReason` holds the reasons of RFC 0002 Amendment 1, steps 3 to 5: unknown author, bad signature, not a member, no key, undecryptable. `seal::OpenError` is the error of one `open` call. It spans steps 2, 3, 5 and 6 (unknown suite, signature, decryption, payload), and it has no case for an unknown author, a missing membership or a missing key envelope, because those depend on the member's control state, which `open` never sees. The mapping from `OpenError` to `Verified` and `Opened` belongs in a real implementation of `receive::Checks`, which NOC-28 left for a later ticket: `AuthorMismatch`, `AuthorKey` and `Signature` become a bad signature; `Decryption` becomes undecryptable; `Payload` is the case the payload-parking decision covers.
- Instead of: storing `OpenError` in the log state, which would leave three of the amendment's five reasons without a value and tie an on-disk state to one function's error type.
- Topics: rfc-0002, rfc-0003, log, crypto
