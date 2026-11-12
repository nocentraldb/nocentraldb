---
type: internal
tickets: [NOC-284]
components: [sdk-rust]
---
A merge delay a test sets is the deadline itself, so `a_writer_never_rotates` no longer fails when the waiting admin's random delay came out at a few seconds.
