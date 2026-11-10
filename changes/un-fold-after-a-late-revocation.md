---
type: breaking
tickets: [NOC-49]
components: [core]
---
## Problem

When a revocation reached a member after it had folded the revoked member's changes, `ncdb-core` kept those changes folded, even the ones above the revocation's cutoff. Two members that held the same changes could show different data, depending on whether the revocation arrived first.

## Use

Nothing to call: when a control change passes, `Log::receive` and `View::receive` check every folded sealed change's membership again. A change the space no longer admits, such as one above a late revocation's cutoff, moves to rejected as "not a member", and the fields and objects it touched are folded again from the changes still folded. The view's tables then equal those of a member that never folded it, in the same transaction. A change that a later control change admits again is folded again.

## Migration

A type that implements `receive::Fold` must implement two new methods: `clear_field(object, collection, field)`, which forgets the state of one field of an object, and `clear_object(object)`, which forgets its creates and deletes. The log folds the operations of the changes still folded into them again afterwards. A type that implements `receive::Checks` must keep `verify` and `open` passing for a change that passed them as control changes are added; only `is_member` may turn from pass to fail.
