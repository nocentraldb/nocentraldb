---
type: fix
tickets: [NOC-391]
introduced: cycle
components: [core, sdk-rust, relay]
---
## Symptom

A device or the relay could destroy a person's key for an erasure that every copy holding all of the space's changes rejects. The erasure cannot be undone, so the person's fields stayed unreadable although the erasure was not valid.

## When

A device or the relay held an erasure but not yet an earlier change it depends on, for example the demotion of the admin whose account signed the erasure off. It happened only when changes arrived out of order, such as after island mode or from a relay that held back changes.

## Cause

The erasure was judged from the changes the copy held, with a gap in its history, and accepted. When the missing change arrived, the erasure was rejected, but the key was already gone.

## Fix

An erasure, and the decision that finalizes a provisional one, now waits until the copy holds everything it depends on, as RFC 0008 says ("A shred is final when accepted"). Until then the key stays, the person's fields stay readable, and the relay does not accept the erasure or delete its record of the key.
