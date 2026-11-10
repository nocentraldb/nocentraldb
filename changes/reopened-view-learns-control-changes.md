---
type: fix
tickets: [NOC-104]
introduced: cycle
components: [core]
---
## Symptom

A view reopened on an existing database rejected new changes from existing members as "unknown author", or as "no key".

## When

After `View::open` on a database that already held control changes, with checks that knew nothing yet, such as a new `SealChecks`.

## Cause

The checks learn the membership and the keys only from control changes they take through the steps. The control changes already in the log were never taken through them again, since a log that holds a change treats it as a duplicate.

## Fix

`Log::receive`, which `View::receive` runs, first hands the checks every control change the log holds folded, so the checks rebuild the membership, the keys and the generation graph from the log. A control change the checks already hold changes nothing.
