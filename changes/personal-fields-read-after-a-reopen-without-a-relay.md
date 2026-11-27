---
type: fix
tickets: [NOC-547]
introduced: 0.1.0
components: [core, sdk-rust, sdk-ts]
---
## Symptom

After a database was closed and opened again, every personal field read as unset (`null` in TypeScript), while the other fields kept their values. `space.status` said nothing about it.

## When

On a device that wrote personal fields without a relay, in the browser and natively. With a relay, the values came back only once the device had connected and fetched the person keys again.

## Cause

A device held the person keys it drew or fetched in memory only. The relay stores each person key's record, wrapped under the space key; without a relay, nothing else kept it, so a reopened database held no person key, and a personal value whose key it does not hold reads as unset.

## Fix

A device now keeps each person-key record it holds, wrapped as the relay stores it, in its own database, and a reopened database holds those keys again with no relay. No person key is stored in the clear. An erasure removes the records of the key it destroys. `SpaceStatus::missing_person_keys` (`missingPersonKeys` in TypeScript) counts the person keys of a space that the device does not hold though no erasure destroyed them: when it is above `0`, some personal values read as unset because their key is missing, not because they were cleared.

A device that wrote personal fields without a relay on an earlier version and closed its database since then cannot hold those keys again: they were never kept anywhere. Its status now counts them as missing. A device whose relay received the records gets them back on its next connection and keeps them from then on.
