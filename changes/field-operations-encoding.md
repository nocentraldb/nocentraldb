---
type: internal
tickets: [NOC-51]
components: [core]
---
`ncdb-core` encodes the set, add and remove operations of basic values, sets and links into RFC 0002 operation maps and decodes them back, exactly as RFC 0004 Amendment 1's examples E1 to E9 show; an operation it cannot read parks as "unknown operation", "unknown argument" or "malformed arguments", natively and in wasm32.
