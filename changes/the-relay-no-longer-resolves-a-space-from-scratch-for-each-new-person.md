---
type: fix
tickets: [NOC-427]
introduced: cycle
components: [relay]
---
## Symptom

On the relay, pushing a new person's key got slower with every person already in the space. The push of the person-key entry and the put of its record together cost about 55 ms at 100 persons and about 3 s at 800 persons, measured on one full core. The relay's Lambda gets only a fraction of a core, so it is slower there.

## When

On every push of a control change, above all the person-key entry the SDK writes for each new person, on every put of person-key records, and when the relay hands a removed member its removal.

## Cause

The relay is stateless between requests, so each of these requests read every control change of the space and ran RFC 0008's `resolve` over them from scratch: the push ran it twice, the put once. For every control change, that folds the change's whole causal past, so each new person cost about the square of the persons in the space. The fix in the members (NOC-424) did not reach the relay. On the AWS backend, each of those reads also used read capacity for every sealed change in the log.

## Fix

A warm relay process, such as a Lambda instance between invocations, now remembers each space's control changes and their resolve. It reads only the control changes appended since, and judges only those with the resolver the members use, which returns exactly `resolve`'s state. A new relay epoch, or a log that no longer holds the last entry it read, makes it read the space from the start. It remembers at most 16 spaces, up to a memory budget of 10,240 control changes over all of them, about one space of 10,000 persons and 80 MB, and forgets the least recently used first. A cold start, or a space too large for the budget, resolves the space from scratch once per request instead of up to twice. At 800 persons a new person now costs about 7 ms for the push and 7 ms for the put on a warm instance, where it cost 1,970 ms and 1,009 ms, and about 1,040 ms for both on a cold instance. The relay stores nothing new.
