---
type: fix
tickets: [NOC-389]
introduced: cycle
components: [core]
---
## Symptom
A member that held two key envelopes for one generation, one with the right key and one with a wrong key, read the generation's changes only when the right envelope arrived last. When the wrong one arrived last, it rejected them as undecryptable.

## When
An admin, or a removed admin within its cutoff, writes a second envelope for the same generation to a member, with another key.

## Cause
The core kept one key per generation, the last envelope's, and opened changes only with that key.

## Fix
The core keeps every key the envelopes of a generation unwrap to and tries each. A change counts when exactly one of them opens it, in any order of arrival. The member writes under the key the generation's genesis or key chain entry confirms, and the core lists each wrong envelope with its author.
