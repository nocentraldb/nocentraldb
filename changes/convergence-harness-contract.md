---
type: internal
tickets: [NOC-19]
components: [core]
---
The convergence harness of `ncdb-core` checks the plugin contract C1 to C7 of RFC 0004, and digests its final state bytes with `sha2` for the golden digest of C3.
