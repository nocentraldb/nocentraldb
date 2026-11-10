---
type: breaking
tickets: [NOC-48]
components: [core]
---
## Problem

`ncdb-core`'s member log took received changes through the steps of RFC 0002 Amendment 1, but nothing verified their signatures or decrypted them: only test doubles implemented `receive::Checks`.

## Use

`checks::SealChecks` implements `Checks` with the real signature and decryption: it verifies with `seal::verify` and decrypts with `seal::open`, and every `seal::OpenError` maps to the step and reason of RFC 0002 Amendment 1. It learns the member keys, the membership and the space keys from the space's control changes (NOC-78):

```rust
let mut checks = SealChecks::new(space, member_key);
log.receive(&frame, &mut checks, &mut fold)?;
```

## Migration

`Checks::verify` now returns `Result<(), Stop>` and `Checks::open` returns `Result<Payload, Stop>`, the decoded payload; the `Verified` and `Opened` enums are removed. In an implementation of `Checks`, return `Ok(())` for `Verified::Valid` and `Err(Stop::Rejected(reason))` for a failed check, and return the decoded `Payload` from `open`, with `Err(Stop::Parked(ParkReason::UnknownPayloadFormat))` for a plaintext that does not decode.
