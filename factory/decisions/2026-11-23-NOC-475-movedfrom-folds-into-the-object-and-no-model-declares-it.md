# 2026-11-23 · NOC-475 · `$movedFrom` folds into the object's state beside its creates, deletes and moves, and no model declares it

- Decided by: coding agent, NOC-475
- Why: a hidden field in every collection would change every model's hash and rebuild every existing view once; kept with the object, it needs no model, no view column and no schema change, and travels as RFC 0016 states, a set on the field `$movedFrom`.
- Instead of: adding `$movedFrom` as a field to every collection of the model, as `$person` is added to the collections that can be a person.
- Topics: rfc-0016, move, core, view, hidden-fields
