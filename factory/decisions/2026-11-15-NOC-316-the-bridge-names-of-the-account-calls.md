# 2026-11-15 · NOC-316 · The bridge names of the account calls

- Decided by: pm, NOC-316, on the coding agent's proposal
- Why: the worker protocol is internal and ships in one build with both sides, but its names stay once written. They follow the bridge's existing names, a call per SDK call under `account.`: `open` takes `"pairing": true` and `"recover": words`; `account.recoveryCode` answers `{"words": [text]}` or `null`; `account.confirmRecoveryCode` forgets the code and answers `null`, also when none is left, since another tab may have confirmed it; `account.recovered` answers a notice or `null`; and for the later slices `account.pair`, `account.completePairing` and `account.removeMember`. The words cross the bridge only in `account.recoveryCode`'s answer, and no error text carries them.
- Instead of: one `account` call with an operation argument, which would hide in one name what each call does.
- Topics: sdk-ts, bridge, accounts
