---
type: internal
tickets: [NOC-188, NOC-146]
components: [sdk-rust]
---
In the Rust SDK, the code a relay sends when it refuses a pushed change is recorded per change in the view's log the same way `SyncStatus::error` shows a code: unchanged when RFC 0007 lists it, else `unknown_error`, through one shared mapping, so the relay's own text never reaches the log (following the owner's answer on NOC-146).
