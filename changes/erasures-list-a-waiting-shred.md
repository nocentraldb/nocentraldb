---
type: breaking
tickets: [NOC-401]
components: [core, sdk-rust, sdk-ts]
---
## Problem

A shred that arrives before part of its causal past waits for its verdict and has no effect until sync fills the gap. The erasure audit did not list it, so a user who erased a person saw no sign of the erasure on a copy that still lacked changes it depends on.

## Use

`space.erasures().list()` in Rust and `space.erasures.list()` in TypeScript list such a shred with the new state `waiting` (`ErasureState::Waiting` in Rust), with its performer and its sign-off, and no members. When the gap fills, it moves on to `final`, `provisional` or `takenBack`, or leaves the list if it is rejected.

```ts
for (const erasure of await space.erasures.list()) {
  if (erasure.state === "waiting") showSyncing(erasure.shred);
}
```

`ncdb-core`'s `Resolved::waiting_shreds()` gives each waiting shred as a `WaitingShred` with its person key, performer and sign-off.

## Migration

In TypeScript, a `switch` on an erasure's `state` that checks it is exhaustive needs a case for `"waiting"`. In Rust, `ErasureState` is `#[non_exhaustive]`, so a `match` on it already has a wildcard arm.
