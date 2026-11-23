# 2026-11-22 · NOC-420 · The AWS backend keeps an identity's highest statement number in `spaces` under the identity and the byte `s`

- Decided by: the coding agent of NOC-420
- Why: a 33-byte key collides with no 16-byte space id, no 32-byte guest item and no reserved item, and a separate item lets one append raise the number and record a guest's one space in the same transaction, which may write an item only once. The update's condition is that the stored number is lower; when it fails, the transaction goes again without it, so the number never falls and the append holds.
- Instead of: a `statement` attribute on the guest item, which one genesis would have to write twice; a table of its own, which every stack would have to create.
- Topics: relay, aws, admission, rfc-0015
