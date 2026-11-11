---
type: feature
tickets: [NOC-206, NOC-222]
components: [sdk-rust]
---
## Problem
A member ignores a person-key record that fails its checks, but an app could not tell that a record was bad, or whose it was.

## Solution
`SpaceStatus::bad_person_key_records` lists the accounts, as `Fingerprint`s, whose signed person-key records failed their checks when the member last fetched the space's records: they do not unwrap under their generation, or their key does not match the commitment in the log.
