---
type: feature
tickets: [NOC-198]
components: [core, sdk-rust]
---
## Problem
A Rust app could not tell that an erasure had not fully happened yet. A shred destroys its person key for good only once it is stable (RFC 0008, "Effects that wait until a change is stable"), and an admin device that stays away keeps it waiting.

## Use
`space.status().current().shreds_waiting` lists each shred that is not stable yet: its change, its age, and, on an admin device once it waited 7 days, the fingerprint of each account whose admin devices block it. The age counts from when this device first held the shred, because control changes carry no clock.

```rust
for shred in project.status().current().shreds_waiting {
    if !shred.blocking.is_empty() {
        alarm(shred.change, shred.age, &shred.blocking);
    }
}
```
