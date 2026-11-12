---
type: feature
tickets: [NOC-244]
components: [core, sdk-rust]
---
## Problem
A personal field is sealed under its person's key. An app must read it like any other field while the key is held, and must see that it is erased once the person is erased.

## Use
`space.get` returns a personal field's value, opened with the person key this device holds. While the key is not held yet, the field reads as unset; it reads again once syncing fetches the key, which it now also does while connected, when a new person is announced. After an erasure, provisional or final, the field reads as unset on every device, and `object.erased()` names it. The view drops the erased person's values from its indexes at once.
