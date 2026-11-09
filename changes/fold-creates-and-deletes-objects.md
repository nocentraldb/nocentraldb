---
type: internal
tickets: [NOC-39]
components: [core]
---
`ncdb-core` encodes and decodes an object's create and delete exactly as RFC 0004 Amendment 1's examples E12 to E14 show, and the fold of a received change puts objects into the copy with their collection and sets their deleted flag, so links resolve to "deleted"; an object or field operation that does not decode parks with its reason, natively and in wasm32.
