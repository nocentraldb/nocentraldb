---
type: internal
tickets: [NOC-153]
components: [sdk-rust]
---
The Rust SDK's test helper for re-adding a member passes the view mutably to the next-change lookup again, so the crate's tests compile after NOC-138 and NOC-153 met on main.
