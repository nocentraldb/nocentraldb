---
type: internal
tickets: [NOC-74]
components: [core]
---
Debug builds and tests compile the crypto crates with opt-level 3, so crypto-heavy tests run up to 50 times faster; release builds are unchanged.
