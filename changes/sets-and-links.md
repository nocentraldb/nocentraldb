---
type: internal
tickets: [NOC-17]
components: [core]
---
`ncdb-core` adds sets (add wins, lost removals readable) and links to objects of the same space (one target as a basic value, many as a set), which resolve to ok, deleted, missing or wrong collection against the copy's objects; they pass the convergence harness's contract C1 to C7 natively and in wasm32.
