---
type: fix
tickets: [NOC-421]
introduced: 0.1.0
components: [sdk-rust, sdk-ts, core]
---
## Symptom

A write whose change was larger than the relay's limit of 16 MiB, such as about 43,000 creates of small records in one `space.write`, succeeded on the writing device but never reached another device: the relay refused it with `too_large`, and the only sign was `refused` in the sync status.

## When

On every write whose change exceeded 16 MiB.

## Cause

The SDK wrote a change of any size, and the relay accepts none above 16 MiB.

## Fix

The SDK splits such a write, between objects only, into several changes, each within the limit, so every part reaches the other devices. On the writing device the write stays atomic. **On other devices a split write is not atomic**: its parts arrive one by one, so another member can see part of the write before the rest, and a part the relay refuses leaves the others standing. One operation, or one object's operations together, above the limit are refused with the new error `tooLarge` (`ErrorCode::TooLarge` in Rust) before anything is written. A write of a few hundred objects is one change, as before.
