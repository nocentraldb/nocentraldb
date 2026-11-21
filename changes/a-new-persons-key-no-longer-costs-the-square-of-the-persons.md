---
type: fix
tickets: [NOC-424]
introduced: cycle
components: [core, sdk-rust, sdk-ts]
---
## Symptom

Creating an object with a personal field got slower with every person already in the space. In the browser it cost about 35 ms per object at 100 persons and about 1.5 s at 800 persons. Objects without personal fields stayed fast.

## When

On every write that gives a person their first personal value, so that the SDK draws that person's key. Writes for persons who already have a key were not affected.

## Cause

A person's key is announced in a control change of the space. After each control change, the member worked out the space's control state again from scratch (RFC 0008, "The function"): for every control change, it folded that change's whole causal past to judge whether it was valid when written. So each new person's key cost about the square of the persons in the space. The member also opened every personal field of the space again after each new key, though nothing was sealed under that key yet.

## Fix

The member now remembers, from one control change to the next, what depends only on a single change or on one change's causal past: its body, whether its signature verifies, its causal past and its judgement. A new person's key judges only its own entry, and the view opens only the values sealed under that new key. The control state, and so convergence, is exactly what it was. What is left grows slowly with the number of control changes: natively about 10 µs per person in the space, about 10 ms at 800 persons.
