---
type: fix
tickets: [NOC-139, NOC-183]
introduced: cycle
components: [core]
---
## Symptom

Two copies of one space could hold different keys for a previous key generation, so one opened the changes sealed under it and the other rejected them as undecryptable.

## When

When two valid key chain entries for one generation gave different keys for the same previous generation, which only an admin that holds the new key could write, and the copies received them in different orders.

## Cause

The checks filled a previous generation with the first key the key chain gave for it and kept that key when a later control change arrived, so the key depended on the order of arrival.

## Fix

The checks derive the key chain keys again from scratch after every control change, from the envelope keys and the set of entries, taken in change id order. Every copy that holds the same entries holds the same keys, and a key from an envelope to the member still wins.
