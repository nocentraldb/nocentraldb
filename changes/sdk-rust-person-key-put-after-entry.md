---
type: fix
tickets: [NOC-206]
introduced: cycle
components: [sdk-rust]
---
## Symptom

A person-key record the admin wrote could be lost: the relay never stored it, so other members never held the person key, until the admin wrote a correct record again.

## When

When the admin wrote a person-key entry while a push of earlier changes was on the wire: the session put the waiting records at the end of that push, before the relay held the entry that announces the new record.

## Cause

The relay accepts a person-key record only once its log holds the entry that announces it, and refuses the whole request with `bad_record` otherwise. The session dropped a request the relay refused with `bad_record`, so the record was gone. A put whose connection ended before the answer lost its records too.

## Fix

The session puts a waiting record only once the relay has confirmed the person-key entry that announces it; the others wait for a later push. A put whose connection ends before the answer keeps its records for the next connection.
