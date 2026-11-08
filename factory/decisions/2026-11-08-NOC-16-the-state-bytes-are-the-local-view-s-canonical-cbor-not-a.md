# 2026-11-08 · NOC-16 · The state bytes are the local view's canonical CBOR, not a wire format; the wire encoding of set is not defined here

- Decided by: coding agent (NOC-16), flagged to pm
- Why: C1 and C3 need canonical state bytes, which RFC 0002's profile gives (`[[id, author, stamp, value, [replaced id…]]…]`); RFC 0006 makes the view's layout no stable format. The operation number of set and how `replaces` travels in an RFC 0002 operation are a wire format, which GENESIS.md reserves for the owner, and no RFC fixes them yet, so this ticket builds no encoder or decoder for them.
- Instead of: choosing an operation number and a `replaces` key in this PR.
- Topics: field-types, change-format, local-view
