---
type: fix
tickets: [NOC-57]
introduced: cycle
components: [core, sdk-rust]
---
## Symptom

A space whose genesis names another app's model was refused by `db.spaces.get`, but its content still reached the local view. Its objects sat in the member's database, folded by this app's rules, which are not the rules they were written by.

## When

When sync pulled a space of another model whose sealed changes did not name their writer's model in payload key 64.

## Cause

The checks of a received change compared only the change's own key 64 with the app's lock. Nothing compared the space's genesis model with it before folding.

## Fix

A sealed change of a space whose genesis names another `model.id` than the lock's is parked as "another model" before anything of it is folded, whatever the change says of itself. The space's membership still folds, so `db.spaces.get` names both model ids in its `spaceOfAnotherModel` error, and the space status counts the parked changes.
