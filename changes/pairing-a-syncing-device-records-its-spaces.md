---
type: fix
tickets: [NOC-337]
introduced: cycle
components: [sdk-rust, sdk-ts]
---
## Symptom

A new device that synced with the relay while it waited to be paired did not write its records into the account space after `complete_pairing`. The other devices of the account did not learn which spaces it belongs to until the new device was opened again.

## When

When the device was opened with a relay and a pairing request, as an app usually opens a new device, and the spaces the pairing added it to reached it before it completed the pairing.

## Cause

A device writes its records into the account space when it opens, when it creates or leaves a space, and when a fold from the relay changes its spaces. A device that waits to be paired belongs to no account yet, so the folds that brought it the account space and its other spaces wrote nothing. `complete_pairing` wrote the records only for a device that linked an account of its own, and no later fold came.

## Fix

`complete_pairing` writes the device's records into the account space at once, and the sync pushes them.
