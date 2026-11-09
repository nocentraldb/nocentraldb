# 2026-11-09 · NOC-62 · A full clock counter carries into the next millisecond, and a member never waits

- Decided by: Carsten, 2026-11-09, on NOC-25 (option 1, as pm recommended)
- Why: with far-future clocks never refused, RFC 0002's wait for the wall clock let one change with a far-future clock and a full counter stop every member that saw it from writing. RFC 0002 Amendment 6 has the clock carry into the next millisecond with counter 0, whatever the wall clock says, as `Clock::now` already does. The amendment also states what the core does at the largest value a clock holds: it refuses to write (`ClockExhausted`) rather than wrap, which follows from "never runs backwards" and decides nothing new.
- Instead of: refusing to write until the wall clock catches up, or a capped wait that ends in the same carry.
- Topics: clock, rfc-0002, wire-format
- Supersedes: 2026-11-08-NOC-14-a-full-clock-counter-carries-into-the-next-millisecond.md
