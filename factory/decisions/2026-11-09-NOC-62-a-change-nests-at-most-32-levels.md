# 2026-11-09 · NOC-62 · A change nests at most 32 levels, checked by every member and the relay

- Decided by: Carsten, 2026-11-09, on NOC-24 (option 1, as pm recommended)
- Why: without a limit, about 1 to 10 KB of nested bytes overflowed the stack of every member and relay that decoded a change. RFC 0002 Amendment 6 writes the rule into "The CBOR profile": levels count from the frame (frame 1, header 2, payload 3), a byte string that is not the header or the payload is one item, so rich text and stream chunks do not count, and every decoder refuses a deeper value before it recurses. The relay checks it in the frame, the header and a control body. `cbor::MAX_DEPTH` stays 32 and now cites the amendment.
- Instead of: a limit of 128 levels, which needs more stack in the browser, or no limit with iterative decoders in every language.
- Topics: cbor, security, rfc-0002, wire-format
- Supersedes: 2026-11-08-NOC-14-cbor-values-nest-at-most-32-deep-counted-from-the-frame.md
