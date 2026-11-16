---
type: fix
tickets: [NOC-343]
introduced: cycle
components: [core, sdk-rust, sdk-ts]
---
## Symptom
Opening with a lock that breaks a rule on its own failed with `modelRefused`, but its message printed the core's Debug form, such as `Lock([IndexUnknownField { collection: "tasks", … }])`, instead of the text `ncdb model build` shows. A declared index whose lookup would not run through it failed with `storageFailed`.

## When
`Db::open` and `nc.open` with a lock that `ncdb model build` would refuse, or whose declared index the view's check refuses.

## Cause
The core's finding had no text of its own, so the error printed its Debug form; and the SDK sent the index check's refusal down the path of storage failures.

## Fix
The message is "the lock is refused: " followed by each refusal's lines, as `ncdb model build` prints them, or by one of five new texts for the view's own index check. An index whose lookup would not run through it is `modelRefused`, and its message names the index.
