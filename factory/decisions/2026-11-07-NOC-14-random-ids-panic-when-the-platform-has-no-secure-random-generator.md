# 2026-11-07 · NOC-14 · Random ids panic when the platform has no secure random generator

- Decided by: coding agent (NOC-14)
- Why: an id from a weaker source could collide, so `ObjectId::random` and `SpaceId::random` panic when `getrandom` fails rather than fall back; on wasm32 getrandom's `wasm_js` feature uses `crypto.getRandomValues`. `Clock::now` takes the wall clock as a closure, so the core reads no time itself, the tests drive the counter overflow, and the waiting loop asks it again; a wall clock beyond 48 bits of milliseconds reads as the largest the clock holds.
- Instead of: a fallible id API every caller must handle for a failure the platform does not have, or a clock that reads `SystemTime` itself, which does not exist on wasm32-unknown-unknown.
- Topics: ids, clock, crypto
