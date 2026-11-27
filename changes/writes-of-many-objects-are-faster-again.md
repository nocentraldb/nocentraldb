---
type: fix
tickets: [NOC-542]
introduced: cycle
components: [core, sdk-rust, sdk-ts]
---
## Symptom

Writes of 100 or 1,000 objects took longer than in 0.1.0-beta.2. In Chromium, a write of 1,000 objects into a new space took about 610 ms, against 555 ms with beta.2. A write of 100 objects took 89 ms, against 80 ms.

## When

On every write that creates or changes many objects in one change, and on every change received from another member, natively and in the browser. The more fields the objects have, the larger the cost.

## Cause

Folding a change into the local view prepared each of its SQL statements again for every operation. Since objects can move to another space, the fold also read the object's state from the database once more for every field, to learn whether the object had moved. A task with two fields cost 14 prepared statements and 4 reads of its object's state.

## Fix

A fold now prepares each of its statements once per change, and remembers whether each object has moved while it folds that change. Nothing changes on disk or on the wire. In Chromium, a write of 1,000 objects into a new space now takes about 415 ms, and a write of 100 objects about 66 ms: 17 to 25 % less than with beta.2.
