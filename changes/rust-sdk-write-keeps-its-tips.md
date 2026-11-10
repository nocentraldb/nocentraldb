---
type: fix
tickets: [NOC-138]
introduced: cycle
components: [core, sdk-rust]
---
## Symptom

Each write in the Rust SDK took longer the more changes its space held.

## When

On every write into a space, and on every `members().add`.

## Cause

A write took its heads, its author sequence and the highest clock from the log of its space by reading the whole log and opening every folded sealed change.

## Fix

The core's log keeps these as its tips as changes are written, pulled and un-folded by a revocation (`Log::tips`, `View::tips`), and a write reads them there. After the database opens, the first write into a space opens each of its folded sealed changes once to rebuild them. What a write takes is the same as before.
