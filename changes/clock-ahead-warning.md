---
type: internal
tickets: [NOC-26]
components: [core]
---
`ncdb-core`'s clock receives a change's clock with `Clock::receive`, which moves the clock exactly as before and returns a `ClockAhead` warning, naming only the author and the difference, when the clock is more than a minute ahead of the local wall clock (RFC 0002, "What the clock does not do"); the change is never rejected.
