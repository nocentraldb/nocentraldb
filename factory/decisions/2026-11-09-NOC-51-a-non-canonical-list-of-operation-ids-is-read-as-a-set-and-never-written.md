# 2026-11-09 · NOC-51 · A non-canonical list of operation ids is read as a set and never written

- Decided by: coding agent (NOC-51), confirmed by pm, who corrected the ticket's acceptance criterion
- Why: the ticket first asked for a test that "a non-canonical list is refused"; RFC 0004 Amendment 1, decision 10 (decided by the owner) says a reader accepts an unsorted or repeating list and only the writer is held to the sorted form. So the decoder accepts and normalises such a list, and the writer refuses it by construction: `replaces` and `observed` are `BTreeSet<OperationId>`, which the encoder writes sorted by bytes, without repeats, `[]` when empty. The test decodes an unsorted list with a repeat and checks that encoding gives E2's canonical bytes, never the non-canonical ones.
- Instead of: a decoder that rejects a non-canonical list, which contradicts the decided RFC.
- Topics: rfc-0004, encoding
