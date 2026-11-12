---
type: feature
tickets: [NOC-244]
components: [core, sdk-rust]
---
## Problem
A personal field must be erasable: its value must be readable only under the key of the person it is about, so that destroying the key erases it.

## Use
Write a personal field like any other field. The SDK seals the value under the key of the person the schema's `person` names: the object itself, or the target of its person link. The first value of a person draws the person's key and announces it before the content. A personal field whose person link is unset is refused with `ErrorCode::NoPerson`. `tx.create_with(collection, values, Create { same_person_as: Some(object) })` records the same person as `object`, read from another space, so that one erasure reaches both.
