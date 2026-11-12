---
type: feature
tickets: [NOC-248]
components: [core, sdk-rust]
---
## Problem
A lock can name a field of a plugin type, but the core has no code for plugin types yet. Its operations parked as an unknown field, as if the lock did not name the field at all, so an app could not tell the two apart.

## Use
Nothing to call. An operation on a field whose plugin type and rule no code implements now parks with its own reason, and `space.status().current().parked` counts it under `ParkReason::UnknownType` (RFC 0011, "A type that is not registered"). The database opens anyway, the rest of each change is folded, and what was parked is folded once the type is registered.
