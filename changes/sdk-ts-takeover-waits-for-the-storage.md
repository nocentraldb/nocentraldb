---
type: fix
tickets: [NOC-352]
introduced: cycle
components: [sdk-ts]
---
## Symptom

In Safari, a write in the visible tab failed with `storageFailed` right after the tab that owned the database was hidden, or it waited until that tab was shown again.

## When

Two tabs of one app had the same database open, the owning tab was hidden, and the browser froze it, or the device was slow.

## Cause

A hidden tab gives the database up to the visible tab. The visible tab could start opening it before the hidden tab's worker had given the database's storage back. Safari gives a worker's storage back only while the worker's tab runs, so the open failed while that tab was frozen.

## Fix

The worker of the tab that gives the database up now gives the storage back itself before it ends, and the visible tab waits for it instead of failing. In Safari, if the hidden tab is frozen before its worker gave the storage back, the visible tab's writes wait until the frozen tab runs again. They are not lost, and they do not fail. The guide's step 8 describes this limit.
