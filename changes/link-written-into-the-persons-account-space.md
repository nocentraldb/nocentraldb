---
type: fix
introduced: cycle
tickets: [NOC-310]
components: [sdk-rust]
---
## Symptom
After a database with an account of its own was paired into the person's account, the person's other devices never learned that its old account was linked.

## When
The database had created a space before the pairing, so it already held an account space of its old account.

## Cause
The database wrote its link statement into the first account space it held, which was its old account's, and then forgot the statement.

## Fix
A database writes to the account space of its current account only, and waits until that one arrives.
