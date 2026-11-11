---
type: internal
tickets: [NOC-225]
components: [relay]
---
The relay no longer deletes a shredded person key's record once the shred is stable; the deletion waits until NOC-224 settles whether a stable shred can still be rejected.
