# RFC 0014: Deleting a space, and every space of an account

REQUEST FOR COMMENTS · NOVEMBER 2026 · Ticket NOC-416 · Draft

This RFC lets a space that synced be deleted: at the relay, on every member that comes back, and with its keys. It also lets an account delete every space it holds, which is what "delete my account" needs. Today a synced space cannot be deleted at all. RFC 0003 and RFC 0008 leave it to a later RFC, and neither SDK has a call for it ([The problem today](#the-problem-today)).

It builds on the roles and the sign-off of [RFC 0008](0008-membership.md) and the storage of [RFC 0007](0007-relay.md); [How this RFC relates to the others](#how-this-rfc-relates-to-the-others) names the rest.

**How to give feedback.** Start a discussion in the category [RFC feedback](https://github.com/nocentraldb/nocentraldb/discussions/categories/rfc-feedback), naming RFC 0014 and the section you mean, or open a pull request against this file to change its text. [Giving feedback](README.md#giving-feedback) says how, and what happens to your point.

**Status.** Draft, for the owner to decide. It adds two control entry types, a signed format and a relay error, which GENESIS.md reserves for the owner, and it drops a space's history, which principle 3 does not foresee ([Agreement with GENESIS.md](#agreement-with-genesismd)). Each place where the owner must choose names the options and a recommendation; [Questions for the owner](#questions-for-the-owner) lists them, Q1 to Q6. The text below describes the recommended answers.

**At 0.1.** Not built. Release 0.1 has no call that deletes a synced space; the [follow-up tickets](#follow-up-tickets) build it once the owner decides.

**Out of scope.** Deleting a local ad-hoc space that never synced, which is a local deletion ([RFC 0003, "Sub-agents get an ad-hoc space"](0003-keys-and-cryptography.md#sub-agents-get-an-ad-hoc-space)). Deleting the archive of a compacted space, which the compaction RFC must define, since no archive exists yet. Moving a space to another relay. How a work order is claimed and validated, which the work-order RFC defines; this RFC only says who signs off a deletion.

## Summary

| Question | Answer |
| --- | --- |
| Who may delete a space? | An admin performs the deletion, and a second account that holds an admin member in the space signs it off, as for an erasure. In a space whose members all belong to one account, that account signs off its own deletion, which stays provisional for 7 days and can be taken back (Q1). |
| How does a deletion travel? | As a control change, the **space deletion** (entry type 9), with the sign-off in it. A **deletion take-back** (entry type 10) ends a provisional one. Both are signed and readable by the relay, like a shred entry. |
| When is it final? | A signed-off deletion is final when accepted: its verdict reads only its causal past, as a shred's does. A provisional one is final 7 days after the relay accepted it, unless the relay accepted a take-back first; the relay keeps the time, as it does for a provisional erasure (Q2). |
| What does each member's copy do? | Once it accepts a final deletion and can check it, a member deletes every row of the space, forgets its space keys and person keys, overwrites the freed pages and keeps only a small record that the space was deleted. The app is told, with the count of its own changes the relay never confirmed. |
| What does the relay delete, and when? | Everything of the space: the log, the frames in S3, the membership table, the generation graph, the person-key store, uploads and subscriptions. It stops serving the space at once, deletes the content within a day, keeps the control changes as proof for 90 days and a tombstone of 50 bytes for good, so no stale member can bring the space back (Q4). |
| Is the space key crypto-shredded? | Not as a step of its own. The space key exists only wrapped in key envelopes inside the log and in members' memory, so deleting the log deletes it. A deletable key record at the relay would reach nothing more (Q3). |
| How does it relate to erasure? | It uses erasure's sign-off and its provisional 7 days. It is wider: it removes non-personal content too, and with it the space's erasure audit. It does not replace an erasure of a person in the spaces that stay. |
| What does deleting an account do? | It deletes every space whose members all belong to the account, the account space last, and leaves every space it shares with another account. Where the account holds the only admins of a shared space, it stops and asks the app to hand the role on or delete that space first (Q5). |
| What does it not reach? | Former members' copies, devices that never come back, members whose core is older or modified, the relay's point-in-time backups for 35 days, exports, and copies of objects made into other spaces. [What this does not do](#what-this-does-not-do) lists each. |

## The problem today

A space ends today in one of two ways, and neither removes its content:

- **Every member leaves**, or is removed. Each member that leaves keeps its copy ([RFC 0009, "Spaces"](0009-sdk-api.md#spaces)), and the relay keeps the log, the key envelopes and the person-key store, with no member left to read them.
- **Every object is deleted.** A delete is a tombstone that can be restored ([RFC 0004, "Creating and deleting an object"](0004-merge-rules-basic-types.md#creating-and-deleting-an-object)): every earlier change stays in the log, readable by every member.

An erasure removes only personal fields, by destroying a person key ([RFC 0003, "Shredding a person's key"](0003-keys-and-cryptography.md#shredding-a-persons-key)). Non-personal content, such as a ticket's title or a note's body, has no key of its own to destroy.

So an app cannot keep the promise "delete my account": the ciphertext stays at the relay, and the content stays readable to every key that ever held the space. nexflow.it asked for both, the ciphertext gone from the relay and the account's content gone with its spaces, without building deletions of its own into DynamoDB and S3 around the relay (NOC-416).

## Terms

| Term | Meaning in this RFC |
| --- | --- |
| Space, member, control change, sealed change, causal past | As in RFC 0002 and RFC 0008. |
| Deletion | A space deletion, entry type 9: a control change that deletes the space it is written in. |
| Performer | The admin member that writes the deletion. |
| Sign-off | The account sign-off of a space deletion: an account key's signature over the space id and the performer ([Account sign-off of a space deletion](#account-sign-off-of-a-space-deletion)). |
| Provisional deletion | A deletion whose performer's own account signed it off, in a space whose members all belong to that account. |
| Take-back | A deletion take-back, entry type 10, that ends a provisional deletion before it is final. |
| Proof | The deletion and every control change of its causal past, which a member needs to check the deletion. |
| Tombstone | What the relay keeps of a deleted space for good: its id, the deletion's change id and when the relay deleted it. |
| Purge | The relay's deletion of a space's stored data once its deletion is final. |

## Who may delete a space

**An admin performs, a second account signs off.** A deletion cannot be undone, removes every member's data and ends the space for everyone, so it follows the rule GENESIS.md gives every job that cannot be undone: a second account signs it off ([GENESIS.md, "Work orders for clean-ups"](../../factory/GENESIS.md#what-we-build)). The rule is RFC 0008's for an erasure, unchanged, so one comparison of accounts serves both ([RFC 0008, "Roles and rights"](0008-membership.md#roles-and-rights)):

- The **performer** must be an admin in the state of the deletion's causal past. An erasure's performer may be a writer; a deletion also ends every membership, which only an admin may do.
- The **sign-off** must come from an account that holds an admin member in that state and is not the performer's account, compared by account key and by every recovery key, as RFC 0008 compares them (NOC-201).
- When exactly one account holds an admin member there, and it is the performer's, another account that holds a writer member may sign off, as for an erasure (Carsten, 2026-11-11, NOC-228). A reader never signs off.
- **In a space whose members all belong to one account**, that account signs off its own deletion, and the deletion is provisional for 7 days ([Final and provisional](#final-and-provisional)). This is the case of a person's private spaces, of a sub-agent's synced ad-hoc space, whose members belong to the parent's account, and of the account space.

A stolen admin device alone therefore cannot delete a shared space, and a stolen device of a one-account space can delete it only provisionally, which another device of the account sees and can take back within 7 days ([RFC 0008, "A provisional erasure in a space with one account"](0008-membership.md#a-provisional-erasure-in-a-space-with-one-account) gives the same reasoning for an erasure).

**A deletion is a work order**, as an erasure is (Carsten, 2026-11-20, NOC-362). Until the work-order RFC defines how one is claimed and validated, the SDKs delete with an interim call and a sign-off the app carries as text, as `Db::erase` does ([What changes for developers](#what-changes-for-developers)). The deletion names no work order, so the relay does not learn which sealed change was one.

**Not taken (Q1).** Any admin, at once: one admin, or a thief with one admin device, could destroy a team's shared space for every account in it, which no other right in RFC 0008 allows; today an admin can remove the others, but they keep their copies. Any admin, with 7 days in which any admin may take it back: a shared space would depend on another admin being online within the 7 days, and the deleting admin could simply delete again.

## The deletion in the log

A deletion is a control change, so that the relay can read and check it without a key and every member reaches the same verdict, as for a shred entry. Two entry types join RFC 0008's table:

| Entry type | Name | Right its author needs |
| --- | --- | --- |
| 9 | Space deletion | admin, plus the sign-off of [Who may delete a space](#who-may-delete-a-space) |
| 10 | Deletion take-back | admin, of the provisional deletion's performer's account |

### Space deletion

```text
space deletion (entry type 9)
└── 8  sign-off           the account sign-off of a space deletion, over the space id
                          and the performer's member id
```

The space is the change's own space, named in its header, so the body does not name it again. The deletion is provisional when every member in the state of its causal past belongs to the performer's account, compared as above, and the sign-off is that account's. A sign-off by an account key that the deletion's causal past supersedes is not valid ([RFC 0008, "Account key statement entry"](0008-membership.md#account-key-statement-entry)).

### Deletion take-back

```text
deletion take-back (entry type 10)
└── 8  deletion           change id of the provisional space deletion
```

Its author must be an admin, in the state of its causal past, of the account that performed the deletion, and the deletion must be provisional and in its causal past. It mirrors RFC 0008's erasure decision with key 9 false. A finalize is not needed, because a provisional deletion becomes final by the relay's clock ([Final and provisional](#final-and-provisional)); reusing entry type 8 with a flag was not taken, because its key 8 names a shred and its rules are an erasure's.

### Account sign-off of a space deletion

A new signed format for RFC 0003, beside the account sign-off of a shred:

```text
sign-off (CBOR map, canonical)
├── 1 account key        32 bytes
└── 2 signature          64 bytes, over
                         H("nocentraldb/space-deletion-sign-off" || space id (16 bytes) ||
                           performer member id (32 bytes))
```

Its own context string keeps a deletion sign-off from passing as a shred sign-off and the other way round. Replaying it gains nothing: it authorises the same performer to delete the same space, which happens once. The bytes and test vectors follow RFC 0003's other formats and RFC 0008's vector generator.

### The verdict

A deletion is judged like a shred ([RFC 0008, "A shred is final when accepted"](0008-membership.md#a-shred-is-final-when-accepted)):

- **It is accepted when it is valid when written**: in the state of its causal past, its author is an admin and the sign-off is valid. No revocation excludes it, and no later change rejects it. The sign-off is the defence, as for a shred.
- **A holder that lacks part of its causal past gives it no verdict.** The deletion waits, with no effect, until sync fills the gap.
- **Of several accepted deletions**, the first in RFC 0008's order that is not taken back takes effect; all of them delete the same space. A final deletion wins over a provisional one.
- **Changes concurrent with the deletion**, or written after it by a member that had not seen it, are deleted with the space. That is the point of the deletion, and the SDK reports them ([What each member's copy does](#what-each-members-copy-does)).

## Final and provisional

A deletion that a second account signed off is **final when accepted**, on every member and at the relay.

A provisional deletion is final **7 days after the relay accepted it**, unless the relay accepted a take-back of it before then. The relay keeps the time it accepted each provisional deletion, as it does for a provisional shred, and its hourly sweep makes the deletion final when the 7 days have passed ([What the relay deletes, and when](#what-the-relay-deletes-and-when)). A take-back that reaches the relay after that gets `space_deleted`.

**While provisional,** every member that accepted the deletion keeps its copy and the relay keeps the space, so a take-back loses nothing. The SDK refuses writes into the space with `spaceDeleting`, shows the deletion in the space list with its age, and gives every device of the performer's account a notice when it first holds it, so the account's owner sees a deletion by a stolen device while it can still be taken back. A member that is offline keeps writing until it learns of the deletion; those changes go with the space.

**Why the relay's clock decides here, though RFC 0008 lets no clock finalize an erasure.** RFC 0008 lets members destroy a person key only on a signed finalize in the log, because destroying the key is the one effect a member cannot undo, and members would otherwise take it on the relay's word. A deletion's irreversible step is the purge of the relay's own storage. A member that deleted its copy too early loses nothing while the relay still holds the log, since it pulls the space again. And the case this RFC exists for, deleting an account, takes away the devices that would write a finalize: if the account's last device is deleted, no device is left to finalize after 7 days, and the space would stay at the relay for good.

**Not taken (Q2).** A finalize written by a device of the performer's account after 7 days, as for an erasure: an account that deletes itself, and with it its devices, would never finalize, which defeats account deletion. A provisional deletion that is final on each member when its own clock passes 7 days, with the relay deleting on its own: members and the relay would disagree about a take-back that arrives late, and a member could delete what the relay then keeps.

**In island mode**, without the relay, a final deletion takes effect on every member that accepts it. A provisional one stays provisional until a member reaches the relay, since only the relay's clock finalizes it.

## What each member's copy does

**On accepting a final deletion, and being able to check it,** a member's core does, in one SQLite transaction:

1. It deletes every row of the space: its frames in `log`, its parked changes, its rows in every view table and index, its control state, its pending links, its ad-hoc copy mappings and its held shreds. Every row of the view carries its space id ([RFC 0006, "Where the view lives"](0006-local-view.md#where-the-view-lives)), as leaving a space already uses.
2. It forgets the space's keys: the unwrapped space key generations and person keys it holds in memory. None is on disk: space keys are on disk only wrapped in key envelopes, which are frames of the log, and person keys never are ([RFC 0003, "The person key"](0003-keys-and-cryptography.md#the-person-key)).
3. It records the deletion in a new table, `deleted_spaces`: the space id, the deletion's change id and frame, and when this device accepted it. The core then refuses every change of that space, from the relay or from another member directly, refuses an add to it and never syncs it again. After a relay epoch changes, it pushes the deletion again, so a relay restored from a backup learns it ([What the relay deletes, and when](#what-the-relay-deletes-and-when)).

After the transaction it checkpoints the WAL with `TRUNCATE`, as after a shred or after leaving a space, and `secure_delete` has overwritten the freed pages ([RFC 0006, "What the view stores in plaintext"](0006-local-view.md#what-the-view-stores-in-plaintext)). The limits on flash storage that RFC 0006 states stay.

**The app is told.** The SDK removes the space from `db.spaces.list()` and its subscription, and reports `spaceDeleted` with the number of the member's own changes in that space that the relay never confirmed, so that work lost with the space is never lost silently (principle 3). A reference into the deleted space from another space reads as missing, as a reference to a space the member does not hold ([RFC 0005, "References into another space"](0005-schema-and-model-lock.md#references-into-another-space)).

**Checking the deletion.** A member deletes its copy only on a deletion it has checked against its causal past, never on the relay's word alone: a relay can withhold changes, but it must not be able to make members destroy data. A member that holds the deletion's causal past checks it at once. One that lacks part of it, because it was offline while members changed, fetches the proof from the relay ([The relay's answer to a deleted space](#the-relays-answer-to-a-deleted-space)). If the relay no longer holds the proof, the member cannot check the deletion: the SDK reports the space as `spaceGone`, keeps the copy, and lets the app discard it locally with `space.discard()`, which deletes it as above without a checked deletion.

**Who never deletes:**

- **A former member.** The relay serves it nothing of the space, and its copy is its own (RFC 0003, "What rotation does not do"). It learns of the deletion only if it is a member again, which it no longer can be.
- **A member that never comes back**, and a device that is lost.
- **A member whose core is older** than this RFC parks entry type 9 as a reserved entry type and keeps its copy until it is upgraded ([RFC 0008, "The control bodies"](0008-membership.md#the-control-bodies)). The relay answers it `space_deleted`, which an older core treats as an unknown error and stops syncing the space.
- **A modified client**, which can ignore the deletion.

## What the relay deletes, and when

### On accepting a deletion

The relay runs RFC 0008's fold over its log plus the deletion, as for every control change, and checks the sign-off. When it accepts a deletion:

- **A final deletion** is appended, and in the same atomic step the relay marks the space deleted in its `spaces` record: the tombstone and the proof's position. From then on it serves the space only as a deleted space ([The relay's answer to a deleted space](#the-relays-answer-to-a-deleted-space)) and ends every subscription to it with `space_deleted`. The purge follows.
- **A provisional deletion** is appended, and the relay records when it accepted it. It keeps serving the space, so members learn the deletion and can take it back. When the relay accepts a take-back, it drops that record; when its sweep finds a record older than 7 days, it marks the space deleted as for a final deletion.

### The purge

The relay deletes, for the space:

| What | RFC 0007's AWS backend | The SQLite backend |
| --- | --- | --- |
| Sealed changes, and their change ids | Items of `log` and `changes` | Rows of the log |
| Frames above 8 KiB | Objects under `frames/<space id>/` in S3 | Rows |
| Uploads begun for the space | Items of `uploads`, and objects under `uploads/` | Rows |
| The person-key store | Items of `person_keys` | Rows |
| Subscriptions | Items of `connections` | In memory |
| Control changes, membership table, generation graph | Kept as the proof for 90 days, then deleted (Q4) | The same |

The SQLite backend purges in one transaction, with `secure_delete` on. On AWS a large space does not fit into one Lambda invocation, so the relay deletes in bounded runs that resume where the last one stopped, from the hourly sweep that already removes orphaned frames (NOC-293). Every purge completes within a day of the deletion becoming final. DynamoDB charges a write per deleted item, about 3 cents per 10,000 changes in eu-central-1 at on-demand prices, so a deletion adds no noticeable cost; the tombstones cost nothing worth counting.

### What the relay keeps

- **The tombstone, for good:** the space id, the deletion's change id and when the relay deleted the space, about 50 bytes. It keeps a stale member from bringing the space back: a member that was offline since before the deletion would otherwise push the whole log again, genesis first, the way members repopulate a relay that lost data ([RFC 0007, "Restores and the relay epoch"](0007-relay.md#restores-and-the-relay-epoch)). The relay answers `space_deleted` to a genesis or any change for a tombstoned space. The space id derives from its creator and a nonce, so no other space can need the same id.
- **The proof, for 90 days (Q4):** every control change of the space and the membership table, so that a member that returns within 90 days can check the deletion and delete its copy. After 90 days only the tombstone remains, and such a member gets `spaceGone`.
- **An account's `guest_space` record**, which counts the one space a guest account may create ([RFC 0007, "What the relay checks"](0007-relay.md#what-the-relay-checks)). Deleting that space does not give the account a new one; otherwise the relay would become free storage for any account that deletes and re-creates.

**Not taken (Q4).** Keeping the proof for good: it holds member ids, certificates and account keys, metadata the relay must not keep longer than it needs ([RFC 0007, "What the relay sees"](0007-relay.md#what-the-relay-sees)). Keeping it 35 days, as long as the backups: a laptop left in a drawer for six weeks would come back to `spaceGone`. Keeping no tombstone: a stale member would bring the space back.

### The relay's answer to a deleted space

A new error code joins RFC 0007's table:

| Code | When | What the member does |
| --- | --- | --- |
| `space_deleted` | The space was deleted. Answers every push, pull, subscribe, `keys` and `person-keys` request for it, a genesis included. Key 6 holds the deletion's entry, `{1: position, 2: accepted at, 3: frame}`, while the relay keeps the proof, and only for a member id the space ever held. | Check the deletion; fetch the proof if its causal past is missing; then delete the copy. Without a deletion it can check, report `spaceGone` and keep the copy. |

A member fetches the proof with a new request, served only to a member id the space ever held:

```text
member → relay   proof    {req, space, after: position}
relay  → member  proof    {req, space, entries: [entry, ...], more: bool}
```

The entries are the space's control changes after the given position, in position order, with gaps where sealed changes were. A message stays within RFC 0007's size, and a member pages with `after` until `more` is false. A pull that returns the control changes of a deleted space was not taken: a pull promises dense positions, which a purged log no longer has.

### Restores

A restore from DynamoDB point-in-time recovery would bring a deleted space back, with every change before the restore point and without the tombstone, if the deletion came after it. So on AWS the relay also writes each tombstone as an object under `tombstones/<space id>` in S3, which a DynamoDB restore does not touch, and RFC 0007's restore procedure gains a step after re-applying the shreds: the relay purges every space it finds there again, before it serves any request. Members that hold the deletion push it again after the new epoch, as a second path. The frames in S3 of a deleted space are already gone and do not come back, since versioning is off.

## The space key needs no shred of its own

RFC 0003 erases a person by destroying a key, because the person's fields stay in the log for good and only the key can go. A deleted space keeps nothing in the log, so the question is whether destroying the space key reaches anything that deleting the log does not (Q3). It does not:

- **The space key lives in the log.** Each generation is on disk only wrapped in key envelopes and in the key chain, which are frames of the log ([RFC 0003, "The space key"](0003-keys-and-cryptography.md#the-space-key)). Members hold the unwrapped generations in memory. Deleting the log, at the relay and on each member, deletes every stored copy of the key.
- **What survives the deletion survives a shred too.** The relay's backups keep the envelopes for 35 days, wrapped to member keys that the deletion does not touch, since a member key serves all of a member's spaces. A member that obtains such a backup can read the space; it could read it anyway while it was a member, as RFC 0003 says of person-key records in backups. A former member, a device that never comes back and a modified client keep the key and the content whatever members destroy.

So the deletion deletes the key with the log and adds no key format. **Not taken (Q3):** a space-key record at the relay, deletable like a person-key record, under which envelopes would wrap the space key. It would change RFC 0003's key hierarchy and wire format, the record would sit in the same backups for the same 35 days, and it would reach no reader the deletion misses.

The compaction RFC must keep this true: an archive of a space is a further copy of its content and its key envelopes, which a deletion must purge too.

## Deletion and erasure

| | Erasure | Deletion |
| --- | --- | --- |
| What goes | One person's personal fields, in every space the performer reaches | Everything of one space: content, personal fields, membership, keys |
| How | A person key is destroyed; the ciphertext stays | The ciphertext and the keys are deleted |
| Who signs off | A second account with an admin member; provisional in a space with one account | The same |
| Finalized by | A signed finalize in the log | Acceptance, or the relay's clock for a provisional one |
| The audit | Survives: the shred, its sign-off and the acknowledgements stay in the log | Goes with the space; the tombstone, and the proof for 90 days, remain |

- **A deletion is not an erasure of the person.** It writes no shred entries: the person keys go with the person-key store and the members' memory. A person whose data sits in other spaces is not erased there. An app that deletes an account also erases the person where its data stays, in shared spaces, with `Db::erase` and a second account's sign-off ([RFC 0009, "The Rust SDK"](0009-sdk-api.md#the-rust-sdk)).
- **A deletion removes the erasure audit of its space.** The acknowledgements of past erasures in the space go with its log. Whether a space that held personal data may be deleted without keeping that audit is a question for each data protection officer; the proof shows the deletion for 90 days, and the tombstone that a deletion happened.
- **A provisional erasure in a space that is deleted** ends with the space: its person key is gone with the store, whether or not a finalize came.
- **An erasure running across spaces** skips a deleted space, which the database no longer holds, and reaches a provisionally deleted one, which may still be taken back.
- **Whether deleting ciphertext counts as erasure** is for each data protection officer to decide. This is a design, not legal advice.

## Deleting an account's spaces

An account is a recovery key and the account key it names, held by the account's devices ([RFC 0010](0010-accounts.md)). Deleting an account means deleting what it holds and leaving what it shares. The SDK of one device of the account does it in one call, from the account space, which lists every space each member of the account belongs to ([RFC 0010, "The account space"](0010-accounts.md#the-account-space)):

1. **It sorts the spaces.**
   - A space whose members all belong to the account: **deleted**, signed off by the account itself, so provisionally for 7 days.
   - A space shared with another account, where another account also holds an admin member: **left.** An admin device of the account removes the account's other members, devices and agents, from it, then leaves; where the account holds no admin, each of its members on this device leaves, and the SDK lists the others for the app, as `removeMember` lists the spaces it cannot reach ([RFC 0010, "Losing a device"](0010-accounts.md#losing-a-device)).
   - A space shared with another account where only this account holds admin members: **stopped.** The SDK refuses with `lastAdmin` and names these spaces, before it deletes or leaves anything. The app hands the admin role to a member of another account, or deletes the space with that account's sign-off, and calls again.
2. **It deletes and leaves** the spaces in that order, and the account space last, provisionally, so that every other device of the account still learns of the deletion and can take it back within the 7 days.
3. **It returns** what it deleted, what it left, the spaces it could not reach, and a notice, `accountDeleted`, that says what the deletion does not reach.

After the call, the app may close the database and delete its files. A device that keeps the database open until the 7 days pass can still take the deletion back; one that deletes its files at once can no longer, and the relay purges after 7 days. Recovery with the recovery code does not bring the deleted spaces back once the relay purged them, as RFC 0010 says for a space whose every member was lost.

**What account deletion leaves.** The account's content in shared spaces stays: it is the team's data, its signatures name the account's members, and only an erasure removes its personal fields. Its member certificates stay in the control changes of every shared space it was in, as metadata the relay and the other members hold. Its agents running elsewhere keep their copies, as every removed member does.

**Not taken (Q5).** Deleting every space where the account holds an admin, shared ones included, with the other accounts' sign-off: one person leaving a team would delete the team's space, and the call would wait for every other account. Leaving a shared space even as its last admin: the space would lose its last admin, which RFC 0008 lets no member choose ([RFC 0008, "Roles and rights"](0008-membership.md#roles-and-rights)).

## What changes for developers

A sketch, in TypeScript; the public names are RFC 0009's to decide in its API review, and the interim calls are replaced by the work-order RFC (Q6):

```ts
// On a device of the second account: sign off a deletion it was asked for.
const signOff = await db.account.signOffDeletion(nc.deletionRequest(text));

// On the performing admin's device:
const request = project.deletionRequest();            // text the app carries to the other account
const deleted = await project.delete({ signOff });    // or project.delete() in a one-account space
deleted.notice;      // { code: "spaceDeleted", text: "Members that are offline delete their copy when they come back. Former members keep theirs." }
deleted.provisional; // true in a space with one account: final in 7 days unless taken back
deleted.unconfirmed; // the number of this member's changes the relay never confirmed

await project.takeBackDeletion();                     // within the 7 days, by an admin of the same account

const gone = await db.account.delete();               // deletes the account's own spaces, leaves shared ones
gone.notice;         // { code: "accountDeleted", text: "..." }
```

- **New reports**: `spaceDeleted` when a deletion takes effect on this member, `spaceDeleting` while one is provisional, and `spaceGone` when the relay says the space was deleted but the member cannot check it.
- **New errors**: `spaceDeleting` for a write into a provisionally deleted space, and `lastAdmin` from `db.account.delete()`.
- **`space.discard()`**, today reserved for a local ad-hoc space, also deletes the local copy of a space the SDK reports as gone.
- **The account space** cannot be deleted with `space.delete()`; only `db.account.delete()` deletes it.

## What this does not do

- **Former members keep their copies**, as RFC 0003 says of every removal. Deleting a space does not reach them.
- **A device that never comes back** keeps its copy and its keys. One that comes back after the relay dropped the proof, 90 days after the deletion, cannot check it, and keeps its copy until the app discards it.
- **A member on an older core, or a modified one,** keeps its copy.
- **The relay's backups** keep the space's ciphertext and envelopes for up to 35 days, as they keep a shredded person-key record ([RFC 0007, "Restores and the relay epoch"](0007-relay.md#restores-and-the-relay-epoch)). The relay and anyone who steals a backup cannot read them; a member that obtains one could, as it could while it was a member.
- **The relay's logs** keep the space id and member ids for their retention, 14 days by default, and the tombstone keeps that the space existed and when it was deleted.
- **Copies outside the space stay:** exports, screenshots, an adapter's output, objects copied into an ad-hoc space, and references to the space from other spaces, which read as missing.
- **The audit trail of the space goes**, apart from the proof and the tombstone. GENESIS.md promises that history moves to an archive instead of being dropped; a deletion drops it on purpose, signed off and reported.
- **Flash storage** may keep freed pages, as RFC 0006 states for every deletion on a device.
- **Changes written on an island after the deletion** are deleted with the space when their member learns of it. The SDK counts the ones the relay never confirmed, but cannot hand them back.

## Questions for the owner

Each question has its options and a recommendation. Q1 to Q4 change the wire format or the relay's storage, Q5 and Q6 what a developer sees.

| | Question | Options | Recommended |
| --- | --- | --- | --- |
| Q1 | Who may delete a space? | **A.** An admin, with a second account's sign-off as for an erasure; a one-account space signs itself off, provisionally for 7 days. **B.** Any admin, at once. **C.** Any admin, with 7 days in which any admin may take it back. | A |
| Q2 | What makes a provisional deletion final? | **A.** The relay's clock: 7 days after it accepted the deletion, unless it accepted a take-back. **B.** A finalize in the log by a device of the performer's account after 7 days, as for an erasure. | A |
| Q3 | Is the space key crypto-shredded? | **A.** No step of its own: the key goes with the log, and no key format changes. **B.** A deletable space-key record at the relay, under which envelopes wrap the space key. | A |
| Q4 | What does the relay keep of a deleted space? | **A.** A tombstone for good, and the proof for 90 days. **B.** The tombstone and the proof for good. **C.** The tombstone, and the proof for 35 days, as long as the backups. | A |
| Q5 | What does deleting an account do to the spaces it shares? | **A.** Leaves them, and stops where the account holds the only admins, until the app hands the role on or deletes the space. **B.** Deletes every space where the account is an admin, with the other accounts' sign-off. **C.** Leaves them all, even as the last admin. | A |
| Q6 | Which calls come before the work-order RFC? | **A.** Interim calls in both SDKs, as `Db::erase` is for an erasure, replaced by the work-order RFC. **B.** No call until the work-order RFC. | A |

## Agreement with GENESIS.md

- **The relay never reads content.** A deletion and its sign-off carry ids, public keys and signatures. Nothing in this RFC carries a key or plaintext to the relay; the relay deletes ciphertext it could never read.
- **Every copy converges.** A deletion's verdict reads only its causal past, so every holder reaches the same one. A deleted space has no state left to differ; a copy that cannot check the deletion keeps the space as it was, the same as a member that never learnt of it.
- **Nothing is lost silently, and history moves to an archive.** This RFC goes against the second half on purpose: a deletion drops a space's history instead of archiving it. It is never silent: it needs a second account's sign-off or 7 days in which the account can take it back, every member is told, and its own changes the relay never confirmed are counted. **This RFC flags it to the owner**: GENESIS.md lists archiving, deleting a person's data and changing permissions as work orders, not deleting a space. If the owner takes this RFC, GENESIS.md should name deleting a space among the work orders and its limits under "Shred and audit".
- **Say plainly what it does not do.** [What this does not do](#what-this-does-not-do) names each limit, and the notices say them to the developer.
- **A developer doesn't need to care about keys.** The SDK deletes and forgets every key; the app carries only the sign-off as text.
- **What must come back to the owner.** The control entries, the sign-off format and the relay error are wire format; the purge and the S3 tombstones write to AWS; and the public names are a developer's first view. Each waits for the owner's decision on this RFC.

## Follow-up tickets

This RFC changes no other RFC and no code. Once the owner decides, it leads to these tickets:

1. **The decided RFCs.** RFC 0003 gains the sign-off format and replaces "Deleting a space" under "Not defined here" with a link here; RFC 0008 gains entry types 9 and 10 and their verdict, and drops "Deleting a synced space" from what it does not cover; RFC 0007 gains `space_deleted`, `proof`, the purge, the tombstone and the restore step; RFC 0006 gains `deleted_spaces`; RFC 0009 gains the calls, reports and errors under the names its review decides. GENESIS.md is the owner's to change.
2. **The core.** Entry types 9 and 10, the sign-off, the verdict and the provisional state in `ncdb-core`, with test vectors from RFC 0008's generator and cases in the convergence harness.
3. **The member's copy.** Deleting a space's rows, `deleted_spaces`, refusing its changes and adds, the WAL checkpoint, and pushing the deletion again after a new epoch.
4. **The relay core and the SQLite backend.** The checks, the tombstone, `space_deleted`, `proof`, the purge, the 7-day sweep for provisional deletions, and the backend trait's new calls, with the conformance suite.
5. **The AWS backend.** The purge in bounded runs from the hourly sweep, the tombstone in `spaces` and under `tombstones/` in S3, and the restore step in [Deploying the relay to AWS](../relay-deploy.md). It writes to AWS, so its deploy waits for the owner.
6. **The Rust SDK.** Deleting a space, the sign-off and its request as text, the take-back, `discard` for a gone space, and the reports.
7. **The TypeScript SDK.** The same.
8. **Account deletion in both SDKs.** `db.account.delete()` with the sorting, `lastAdmin`, and the account space last.
9. **The developer pages.** A page on deleting spaces and accounts that states every limit, linked from the erasure page.
10. **The compaction RFC** must purge a deleted space's archive, as [The space key needs no shred of its own](#the-space-key-needs-no-shred-of-its-own) requires: a note on its ticket.

## How it is tested

- **The convergence harness** (RFC 0008): a deletion concurrent with a revocation of its performer, with a write on an island, with a second deletion, and with an add; a provisional deletion and its take-back; a deletion whose causal past a member lacks, which waits.
- **Test vectors** for both entry bodies and the sign-off, generated with independent libraries as RFC 0008's are.
- **The relay's conformance suite,** on SQLite and on the AWS emulator: after a purge, no table, row or S3 key holds the space id apart from the tombstone and, until it expires, the proof; a stale member that pushes the whole log, genesis first, gets `space_deleted` and appends nothing; a provisional deletion is purged after 7 days on a clock the test controls, and not after a take-back; a restore before the deletion purges the space again before it serves.
- **A canary test on a member:** content and personal fields filled with recognisable values, the space deleted, the WAL checkpointed; the SQLite file and its WAL must hold none of the values.
- **SDK tests** in both SDKs for the calls, the reports, `lastAdmin`, and an account deletion that deletes, leaves and stops as [Deleting an account's spaces](#deleting-an-accounts-spaces) says.

## How this RFC relates to the others

It answers what [RFC 0003](0003-keys-and-cryptography.md#not-defined-here) and [RFC 0008](0008-membership.md#what-this-rfc-does-not-cover) left to a later RFC, and reuses RFC 0008's sign-off, verdict and provisional erasure without changing them. It adds to the storage, the errors and the restore procedure of [RFC 0007](0007-relay.md), to the view of [RFC 0006](0006-local-view.md), to the account of [RFC 0010](0010-accounts.md) and to the calls of [RFC 0009](0009-sdk-api.md), each through a follow-up ticket once decided. The work-order RFC will replace its interim calls, and the compaction RFC must purge a deleted space's archive.

## History

- 2026-11-21 · First published as a draft (NOC-416)
