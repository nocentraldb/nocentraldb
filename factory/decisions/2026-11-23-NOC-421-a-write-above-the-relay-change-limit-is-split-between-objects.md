# 2026-11-23 · NOC-421 · A write above the relay's change limit is split between objects into several changes, and one operation or object above the limit is refused with `tooLarge`

- Decided by: Carsten, 2026-11-23, on NOC-431 (option B′); the error's name `tooLarge` (Rust `ErrorCode::TooLarge`) is proposed in the PR of NOC-421 and waits for the owner's confirmation there
- Why: a change above 16 MiB was folded on the writer and refused by the relay with `too_large`, so its data lived on one device only, and the app learned it only from `SyncStatus.refused` (GENESIS principles 3 and 4). Bulk imports of tens of thousands of objects in one write are the realistic case, and a split serves them without the app knowing the limit; one large value is the rare case, and chunked file and stream types are meant to carry those.
- Instead of: A, refusing every write above the limit, which keeps a write atomic everywhere but fails a bulk import on a size the app cannot see; C, keeping the behaviour and only documenting it; B, splitting anywhere, including inside one object.
- What this does not do: a split write is not atomic on other devices. Its parts arrive and fold one by one, so another member can see part of it before the rest arrives, and a part the relay refuses leaves the others standing; convergence holds. One object whose operations together exceed the limit is refused as well, since a write is never split inside an object.
- Topics: sdk-rust, sdk-ts, relay, rfc-0007, rfc-0009, limits

RFC 0007, "Large changes", and RFC 0009, "One write, one change", state it.
