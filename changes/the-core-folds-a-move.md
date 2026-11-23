---
type: breaking
tickets: [NOC-475]
components: [core]
---
## Problem
An app could not move an object to another space; it had to copy the object and delete the original, which gave it a new id and cut its links (RFC 0016).

## Use
The core now folds a move: operation 6 on the object, which `ncdb_core::field_op::move_object` writes, and the hidden `$movedFrom` of the new space, which `field_op::moved_from` writes. `Object::moved_to` and `Object::moved_from` read them. A moved object is a trace in the view: out of every index, with no fields. The SDK call that moves objects follows in a later release.

## Migration
`ncdb_core::object::ObjectOperation` gains the variants `Move` and `MovedFrom`; a `match` on it needs arms for them. A view an older core folded folds the moves it parked on its first open with checks.
