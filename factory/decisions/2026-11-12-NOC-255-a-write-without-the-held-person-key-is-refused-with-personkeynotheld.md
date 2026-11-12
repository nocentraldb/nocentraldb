# 2026-11-12 · NOC-255 · A write without the held person key is refused with personKeyNotHeld

- Decided by: Carsten, 2026-11-12, on NOC-255 (A)
- Why: a write of a personal field whose person already has a person key that this device does not hold yet, as after a reconnect before the person keys are fetched, cannot be sealed. The SDK refuses it with the new error code `personKeyNotHeld` (`ErrorCode::PersonKeyNotHeld` in Rust): the person has a key this device does not hold yet, and the app retries once syncing has fetched it.
- Instead of: refusing with an existing code, which would name another cause; waiting for the key inside the write, which would make a write wait on the network; and drawing a fresh key for the person, which a second key for one person would lose to the first under the first-key rule, leaving the value unreadable for everyone.
- Topics: personal-fields, person-keys, api, errors
