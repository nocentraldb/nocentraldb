---
type: feature
tickets: [NOC-244]
components: [core]
---
## Problem
A personal field must become unreadable once its person is erased. It needs a sealed form that only the person key opens, and that cannot be moved to another field, object or space.

## Use
`ncdb_core::personal` seals a value under a person key at its `Place` (space, object, person-key id, collection and field) and opens it again, as RFC 0003 defines it ("Personal value"). `Sealed` is the map argument 1 of a set carries, `{1: person-key id, 2: sealed bytes}`, and `Person` is the value of the hidden field `$person`, `{1: person id, 2: person-key id}`. Nothing writes or reads them through the SDK yet.
