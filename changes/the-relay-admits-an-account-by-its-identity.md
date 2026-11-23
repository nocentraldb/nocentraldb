---
type: fix
tickets: [NOC-420]
introduced: cycle
components: [relay]
---
## Symptom
After an account recovered from its recovery code, or replaced a lost device with `db.account.removeMember(memberId, { recover })`, the relay refused every new space of that account with `not_allowed`, though the operator had listed it.

## When
On any relay whose `NCDB_ALLOWED_ACCOUNTS`, or `ncdb-relay --allow-account` and `--allow-invitation`, listed the account; and for a guest account, which got a second one-space allowance after each recovery.

## Cause
The allowlist and the guest's one space were keyed by the account key, and both events give the account a new account key under the same recovery key.

## Fix
The relay admits an account by its identity, the recovery key that the certificate's account key statement names, and by the account key only for an account without one. `--allow-invitation` lists that identity, and the deploy guide prints it. An entry that holds an account key keeps matching it until the account's first recovery, so replace such entries with the identity. The relay now also refuses a space from a lost device once it has seen the account's newer statement.
