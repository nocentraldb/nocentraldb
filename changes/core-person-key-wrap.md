---
type: internal
tickets: [NOC-206]
components: [core]
---
The core wraps and unwraps a person key under a generation of the space key, as RFC 0003's "Person-key record" gives the wrapped bytes (`membership::wrap_person_key`, `unwrap_person_key`), with a secret `keys::PersonKey` that never prints its bytes.
