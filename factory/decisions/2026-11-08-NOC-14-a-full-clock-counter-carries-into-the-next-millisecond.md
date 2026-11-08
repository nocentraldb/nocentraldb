# 2026-11-08 · NOC-14 · A full clock counter carries into the next millisecond

- Decided by: pm, pending the owner (NOC-25)
- Why: RFC 0002 has the writer wait for its wall clock when the counter is full. After a clock from the future was seen, that wait lasts as long as the other member's clock is ahead, and forever after `observe(Hlc(u64::MAX))`, which the first implementation did as a busy loop. `Clock::now` now takes one more than the highest clock seen, which carries into the next millisecond, never waits and never runs backwards, and returns `ClockExhausted` at `u64::MAX` rather than wrap. It takes the wall clock as a value, since it no longer asks again. Reverses part of "Random ids panic without a secure generator; the clock takes the wall clock as a function".
- Instead of: waiting for the wall clock, as RFC 0002 says.
- Topics: clock
