# 2026-11-08 · NOC-14 · CBOR values nest at most 32 deep, counted from the frame

- Decided by: pm, pending the owner (NOC-24)
- Why: the decoder recursed without a limit, so a frame of nested one-item arrays overflowed the stack of a member or the relay, after a few hundred levels in a debug build and sooner on wasm32. `cbor::MAX_DEPTH` is 32; the frame is at depth 1, the header map at 2 and the payload map at 3, since a byte string that holds encoded CBOR does not reset the count. The depth is checked before a value is read, so 100,000 levels fail with `Reason::TooDeep`. The encoder still writes any depth; a value too deep for the decoder is refused on read.
- Instead of: no limit (a crash), or a limit per map that a header could multiply by nesting payload inside it.
- Topics: cbor, security
