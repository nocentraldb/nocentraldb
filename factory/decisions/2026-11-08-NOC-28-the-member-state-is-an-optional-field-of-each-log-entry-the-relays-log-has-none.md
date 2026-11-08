# 2026-11-08 · NOC-28 · The member state is an optional field of each log entry; the relay's log has none

- Decided by: coding agent (NOC-28)
- Why: pm's criteria put the state in NOC-14's in-memory log, beside the frame bytes. The same `Log` also models the relay's log, which holds no states, so `Log::append` keeps its contract and leaves the state empty, and only `Log::receive` sets one.
- Instead of: a second log type for members, or a required state on `append` that would give the relay's log a state it must not have.
- Topics: rfc-0002, log, relay
