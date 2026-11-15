---
type: fix
tickets: [NOC-332]
introduced: cycle
components: [sdk-ts]
---
## Symptom

On a phone, a write made in one tab did not show there, and showed only once the person went back to another tab of the app. If that other tab was closed first, the write failed with `storageFailed`, "the tab that held the database closed before it answered; the call may or may not have run".

## When

Two tabs of one app had the same database open, and the tab that opened it first was in the background, as happens when a person switches tabs on an iPhone.

## Cause

The tab that opened the database first owns it, and the other tabs send their calls to it. A phone freezes a hidden tab but leaves it its lock, so the other tabs' calls waited in the frozen tab until it woke, and a write that waited there could not be sent again once it closed.

## Fix

The owning tab gives the database up when it is hidden, after answering the calls it already took, and the visible tab takes it over, so its writes run and show at once. A hidden tab now syncs only while it owns the database.
