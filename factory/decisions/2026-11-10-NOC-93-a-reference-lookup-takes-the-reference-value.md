# 2026-11-10 · NOC-93 · A lookup on a reference index takes the reference's value, and the view writes it as 32 bytes

- Decided by: coding agent (NOC-93)
- Why: RFC 0006 Amendment 7 stores a reference in the view as one 32-byte `BLOB`, the space id followed by the object id, and `NULL` when it is unset or does not fit. The core's `View::page` and `View::page_in` take equality values as `cbor::Value`, as a field holds them; for a key on a reference, the lookup writes the value `[space id, object id]` as those 32 bytes itself, so a caller passes the same value it writes and reads, and cannot pass a byte layout of its own. The view's model hash names a reference with its own tag (4), so a lock that turns a link into a reference is another model.
- Instead of: taking the 32 bytes from the caller, a layout every caller would have to build.
- Topics: rfc-0006, view, indexes, query, references
