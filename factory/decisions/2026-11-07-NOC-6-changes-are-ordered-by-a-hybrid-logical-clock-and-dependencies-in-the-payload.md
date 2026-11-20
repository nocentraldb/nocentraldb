# 2026-11-07 · NOC-6 · Changes are ordered by a 64-bit hybrid logical clock and dependencies, both inside the encrypted payload

- Decided by: Carsten (RFC 0002 decisions D1 and D6)
- Why: with a hybrid logical clock "the last writer wins" matches what users expect in time, and the dependencies on the author's heads give the causal order at a size that does not grow with the number of keys. The clock sits inside the payload because in the header it would show the relay when every change was written, offline work included, and a relay check of far-ahead clocks would protect only changes that pass through the relay, not those exchanged in island mode.
- Instead of: a Lamport clock plus dependencies, or a hybrid logical clock plus a version vector; and the clock in the header, so that the relay could refuse changes whose clock is far ahead of its own.
- Topics: rfc-0002, clock, convergence, change-format, relay

The clock layout (48 bits of milliseconds, a 16-bit counter), how it advances and the dependency list are in `docs/rfc/0002-change-and-log-format.md`, "Ordering metadata".
