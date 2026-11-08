---
type: internal
tickets: [NOC-28]
components: [core]
---
`ncdb-core` takes every received change through the steps of RFC 0002 Amendment 1 and keeps it folded, parked or rejected with its reason, parks unknown operations on their own (D10), checks rejected changes again when a control change arrives (D11) and records a relay refusal beside the change without changing its state (D12); checks and fold plug in through traits until NOC-15 and the field types land.
