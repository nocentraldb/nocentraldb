---
type: fix
tickets: [NOC-353]
introduced: cycle
components: [sdk-ts]
---
## Symptom

A write from the TypeScript SDK in the browser that set a link failed with `invalidValue`, "the value does not fit the field project of tasks". `tx.create(model.tasks, { project: id })` could not link a task to its project, an update could not move the link, and `nc.add(id)` on many links failed the same way. A personal field sealed under the person a link names could not be written in the same transaction as that link.

## When

In the browser, for every `nc.link` and `nc.links` field. The Rust SDK was not affected.

## Cause

The browser passes an object id to the core as its 32 hex digits, and the core wrote that text into the field as it was. A link takes an object id, so the core refused the text.

## Fix

The core reads the field's type from the lock and writes a text for a link, or an element added to or removed from many links, as the object id it spells. A text that spells no object id is refused with `invalidValue`. A link written in the browser reads back as its id there, and a Rust device reads it as an `ObjectId`.
