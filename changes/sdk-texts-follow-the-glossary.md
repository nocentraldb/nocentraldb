---
type: fix
tickets: [NOC-359]
introduced: cycle
components: [core, sdk-rust, sdk-ts]
---
## Symptom
A view holding an unreadable ad-hoc copy id failed with "the database holds an unreadable an ad-hoc copy's id". Other messages used words the glossary avoids ("lookup", "the worker"), called an agent's database a device, or said one thing in several ways, such as an unknown version of an invitation, a pairing answer and an erasure request.

## When
Any call that returned one of these errors since the texts moved into `texts/texts.json`.

## Cause
The texts were written one at a time, before `docs/glossary.md` set the terms.

## Fix
26 texts follow the glossary and say one thing in one way, and ts.notAQuery merges into ts.notAQueryStartOne; `scripts/texts-glossary.test.mjs` holds the rules. No error code, notice code or API name changed.
