---
type: internal
tickets: [NOC-179, NOC-146]
components: [sdk-rust]
---
In the Rust SDK, `SyncStatus::error` shows a code the relay sent that RFC 0007 does not list, including an empty or overlong one, as the fixed code `unknown_error`, so the relay's own text never reaches the app; every code RFC 0007 lists still shows unchanged (decided by the owner on NOC-146).
