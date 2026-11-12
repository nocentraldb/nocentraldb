# 2026-11-12 · NOC-271 · On Lambda, live entries go out from the push that appended them, read as each subscriber

- Decided by: coding agent (NOC-271)
- Why: RFC 0007, "Delivery to subscribers": the invocation that appended posts the new entries. The handler reads the range from the push's first to its last accepted position through the relay core, once per subscriber and as that subscriber's member, in pages under 128 KiB. A member that is no longer current reads nothing, and its subscription ends with `removed_member`, as in the WebSocket server. A range may include entries that an interleaved push appended; the member skips any position it already holds, and pulls any gap it sees. Subscribe stores the subscription before it posts the catch-up pages, so an append in between is not missed. `added` goes to every connection of each member that the push made current, found by member.
- Instead of: keeping a cursor per subscription, which would cost a write per delivery; or posting the pushed frames without the core's read, which would skip the membership check at delivery.
- Topics: relay, aws, lambda, subscribe
