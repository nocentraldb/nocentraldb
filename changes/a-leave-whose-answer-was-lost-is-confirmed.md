---
type: fix
tickets: [NOC-276]
introduced: cycle
components: [sdk-rust, relay]
---
## Symptom

A member that left a space could show a change as unconfirmed forever. `SyncStatus::unconfirmed` stayed at 1 and `error` said `not_member`, though the relay held the change.

## When

When the relay appended a member's own leave, or any change that ended its membership, but the connection ended before the `pushed` answer reached the member: a dropped network, or a relay slower than the member's answer timeout.

## Cause

RFC 0007 makes a push idempotent: a member pushes the same frames again and gets `duplicate` for those that made it. Two things broke that promise:

- The relay checked that the uploader is a current member before it looked for a duplicate, so a former member that pushed its leave again got `not_member`.
- The SDK stopped the space on `not_member` and never pushed it again.

## Fix

The relay answers `duplicate` to a change's own author before it checks membership. Anyone else who pushes a change the relay holds must still be a current member, so a former member learns nothing of changes it did not write.

When the relay first stops a space, the SDK pushes that space's unconfirmed changes once more. The relay confirms those it holds and refuses the others with `not_member`, so none waits for a confirmation that cannot come.
