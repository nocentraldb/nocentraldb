# 2026-11-23 · NOC-475 · `space.inspect` stays out of NOC-475; its criteria for traces and the `moved` link status go to NOC-375

- Decided by: pm, 2026-11-23, on NOC-475 and NOC-375
- Why: `space.inspect` is built in neither SDK (RFC 0009, "At 0.1"), and link statuses on reads come with it. NOC-475 delivers later edits through `space.moves().later_edits()`, across a reopen; NOC-375, which builds `space.inspect` for every field type, takes as acceptance criteria that it shows a trace's later edits and the `moved` link status, both across a reopen.
- Instead of: a minimal `inspect` for traces in NOC-475, or the whole of `space.inspect` there.
- Topics: rfc-0016, move, inspect, sdk-rust, scope
