# 2026-11-14 · NOC-315 · The bridge names of the remaining space and database calls

- Decided by: pm, NOC-315, on the coding agent's proposal
- Why: the worker protocol between the TypeScript SDK and its core is internal and ships in one build with both sides, but its names stay once written. They follow the names the bridge already uses: a call per SDK call, in the SDK's own dotted form, and an event per report, named after it. The calls are `spaces.follow` (answers the space list and from then on sends a `spaces` event whenever it changes), `space.leave` (answers `{notice}`), `erasures.list`, `durability` and `view`; the events are `spaces`, `durability` and `view`. A space list entry gains `model`, `"ours"` or `"another"`, as RFC 0009 shows it. `space.leave` is a change, so a tab that reaches another tab's database never sends it twice (NOC-177).
- Instead of: a generic `follow {report}` call with one event kind for every report, which would hide in one name what each report carries; or sending the space list after every fold, which most apps never follow.
- Topics: sdk-ts, bridge, spaces
