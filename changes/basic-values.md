---
type: internal
tickets: [NOC-16]
components: [core]
---
`ncdb-core` adds the field-type trait of RFC 0004 and basic values (string, number, boolean, choice) with last writer wins on read and losing writes kept as conflicts; they pass the convergence harness's contract C1 to C7 natively and in wasm32.
