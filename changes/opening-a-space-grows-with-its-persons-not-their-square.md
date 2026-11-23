---
type: fix
tickets: [NOC-442]
introduced: cycle
components: [core, sdk-rust, sdk-ts]
---
## Symptom

Opening a database still got much slower as a space gained persons with a personal field, and it held a lot of memory. With 0.1.0-beta.2 in Chromium, opening took about 2.1 s at 1,000 persons, about 70 s at 5,000 and about 6 min at 10,000. Writes in a space with 10,000 persons took the tab above 4 GB.

## When

On every `nc.open` of a database whose spaces hold many control changes, and in the relay when it starts cold. Each new person adds one control change, the entry that announces their key.

## Cause

To work out a space's control state (RFC 0008, "The function"), the member judged each control change from the state of its whole causal past, and computed that state from scratch for every control change: about 50 million steps at 10,000 persons. It also kept each control change's causal past as its own list of change ids, which grew with the square of the persons too.

## Fix

The member now carries the state of the control changes it judged so far on to the next one. An entry that announces a person's key, written after all of them, is judged from that state and added to it as one more change. Every other control change is still judged from scratch. The control state is exactly what it was: tests check it against the state computed from scratch, in every order of arrival. Natively, a space with 10,000 persons now opens in about 0.7 s and takes about 72 MB to work out its control state, where it took about 4 min and 3 GB.
