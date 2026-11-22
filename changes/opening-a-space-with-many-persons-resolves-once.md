---
type: fix
tickets: [NOC-438]
introduced: cycle
components: [core, sdk-rust, sdk-ts]
---
## Symptom

Opening a database took longer and longer as more persons in a space had a personal field. With 0.1.0-beta.1 in Chromium, opening took about 22 s at 300 persons, and the reported time at 700 persons was 245 s.

## When

On every `nc.open` of a database whose spaces hold many control changes. Each new person adds one control change, the entry that announces their key.

## Cause

When it opened a space, the member gave its checks the space's control changes one at a time. After each one, the checks worked out the space's control state again (RFC 0008, "The function"). Before the fix for NOC-424, each of those rounds also judged every control change again from its whole causal past.

## Fix

The checks now take every control change of the space at once and work out the control state once. The state and the space keys are the same as before. In Chromium, a space with 1,000 persons now opens in about 2.7 s, down from 5.3 s with NOC-424's fix alone. That single round still grows faster than the number of persons, because it judges each control change from its own causal past.
