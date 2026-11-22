---
type: fix
tickets: [NOC-435]
introduced: cycle
components: [relay]
---
## Symptom

On the relay, each put of person-key records read every person-key record the space stores, so its cost grew with the persons in the space. On the AWS backend each such put was a DynamoDB query over all of the space's records, which used read capacity for every person.

## When

On every put of person-key records, above all the one the SDK sends for each new person.

## Cause

To check that a record replaces the generation stored for its person key, the relay read all of the space's stored records and looked up the ones it needed.

## Fix

The relay reads only the stored records of the person keys the put touches: on SQLite one lookup per person key, on DynamoDB one consistent `GetItem` per person key. A put of one new person's record read 94, 394 and 794 stored records at 100, 400 and 800 persons, and now reads none; on SQLite the put of one more person at 800 persons took about 10 ms and now takes about 7 ms on a warm process. What the relay stores does not change.
