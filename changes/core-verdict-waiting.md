---
type: breaking
tickets: [NOC-391]
components: [core]
---
## Problem

`ncdb-core`'s `membership::resolve` gave a shred its verdict from the part of its causal past the holder held. A shred accepted over a gap could be rejected once the gap filled, after the person key was already destroyed. RFC 0008 gives a shred its verdict only once the holder holds its whole causal past, so the verdict needs a value for the time before.

## Use

`membership::Verdict` has a new variant, `Waiting`: a shred, or an erasure decision that finalizes, whose causal past names a change the holder does not hold. A waiting change is not applied, so `PersonKey::destroyed()` stays false for its key, and it gets its verdict once the missing changes arrive.

```rust
match resolved.verdict(shred) {
    Some(Verdict::Waiting) => { /* keep the key; ask for resolved.gaps() */ }
    _ => {}
}
```

## Migration

A `match` on `membership::Verdict` needs an arm for `Verdict::Waiting`. Treat it as not accepted: it changes nothing yet.
