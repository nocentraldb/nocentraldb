---
type: feature
tickets: [NOC-244]
components: [core]
---
## Problem
A personal field is sealed under its person's key. The view must hold its plaintext only while that key is held and the person is not erased, and its indexes with it.

## Use
`Model::person(collection, field)` says whom a field is about. `View::set_person_keys(space, keys)` gives the view the person keys a space's personal fields open with, in memory only. Each personal field's column then holds the opened value, and reads as unset without its key. Leaving a key out, as for an erased person, empties the field from the view and its indexes at once. The merge state keeps every personal value sealed. Nothing in the SDK calls it yet.
