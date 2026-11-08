# 2026-11-08 · NOC-26 · The clock warning is a return value of `Clock::receive`, measured in whole milliseconds

- Decided by: coding agent (NOC-26)
- Why: `receive(seen, author, wall)` raises the highest clock exactly as `observe` does and returns `Option<ClockAhead>`, so the caller that folds a change gets the warning to hand to the SDK without the core logging anything; the wall clock is passed in, as for `now`, so the code is the same natively and in wasm32. "More than a minute ahead" compares the clock's milliseconds with the wall clock and ignores the counter, so a clock exactly 60 000 ms ahead with any counter raises none. The warning compares with the wall clock, not the highest clock seen, so every change that far ahead warns again.
- Instead of: changing `observe`'s signature (it stays the plain clock rule), a callback or log line from inside the core, or comparing full 64-bit clock values, which would warn at 60 000 ms with a counter above zero.
- Topics: clock, rfc-0002, logging
