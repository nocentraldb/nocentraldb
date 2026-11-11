---
type: internal
tickets: [NOC-198]
components: [core]
---
`ncdb-core` computes when a control change is stable (RFC 0008, M20/M21), so that a shred may destroy its person key for good, and names the admins whose runs block each waiting shred, for the space status's `shreds_waiting` and the relay's deletion (NOC-113).
