# RFC 0008: Membership, roles and the first change of a space

REQUEST FOR COMMENTS · NOVEMBER 2026 · ticket NOC-23

This RFC defines who belongs to a space and what each member may do. It covers the first change of a space, the roles and their rights, and the bodies of the control changes. It also defines the rules by which every member and the relay fold those control changes into one membership state, the same on every copy, in any order of arrival.

It builds on [RFC 0001](0001-nocentraldb.md), above all [the design in one page](0001-nocentraldb.md#the-design-in-one-page) and [what it is not](0001-nocentraldb.md#what-it-is-not). It also builds on [RFC 0002](0002-change-and-log-format.md), which reserves control changes as [kind 1](0002-change-and-log-format.md#key-generation-suite-and-kind), with its log state [*rejected*](0002-change-and-log-format.md#rejected-changes) and its rule for changes the relay refuses, on [RFC 0003](0003-keys-and-cryptography.md), which names the control changes and their purpose, on [RFC 0004](0004-merge-rules-basic-types.md), whose style it follows, on [RFC 0006](0006-local-view.md), which keeps a per-space control state that this RFC defines, and on the principles in [GENESIS.md](../../factory/GENESIS.md).

It also uses [RFC 0005](0005-schema-and-model-lock.md), above all the `model.id` in a space's first control entry ([Data from a newer model](0005-schema-and-model-lock.md#data-from-a-newer-model)) and the person id ([Identifying the person](0005-schema-and-model-lock.md#identifying-the-person)).

It is used by [RFC 0007](0007-relay.md), the relay, which applies the membership rules with the data it can see, and by [RFC 0009](0009-sdk-api.md), the SDK's API, which names the calls that write control changes.

**Status.** Decided by the owner (Carsten, 2026-11-10).

**How to give feedback.** Reply in this RFC's discussion, [RFC 0008: Membership, roles and the first change of a space](https://github.com/nocentraldb/nocentraldb/discussions/148), one topic per reply, or open a pull request against this file to change its text. [Giving feedback](README.md#giving-feedback) says how, and what happens to your point.

Membership rules decide how permissions behave, which GENESIS.md asks us to tell developers plainly, and the control-change bodies are part of the wire format; the owner decided both. Each choice below says why it was made and, where a reader would ask, which options were not taken. The core computes `resolve` in `crates/ncdb-core`, and the relay runs the same code in `crates/ncdb-relay`.

## What this RFC does not cover

- **Work orders**, beyond who may perform an erasure and sign it off. A later RFC defines how a work order is claimed, performed and validated.
- **Cryptography and the formats of signed objects.** The primitives, the key envelope's HPKE construction, the person-key commitment, the member certificate, the account sign-off and the key-chain subkey are RFC 0003's ([RFC 0003, "Formats of what keys sign and derive"](0003-keys-and-cryptography.md#formats-of-what-keys-sign-and-derive)). This RFC places them in the control bodies (see [What this RFC uses from RFC 0003](#what-this-rfc-uses-from-rfc-0003)).
- **Revoking a member certificate.** [RFC 0010](0010-accounts.md) keeps it out too: removing the member from every space it belongs to is the revocation, and a new account key stops new certificates in the account's name.
- **Invitations and public API names.** This RFC does not cover how a new member's public identity reaches the adding member, or what the SDK calls are named. [RFC 0009](0009-sdk-api.md) covers both: an invitation is the joining member's public identity, which the app carries to an admin ([RFC 0009, "Invitations and members"](0009-sdk-api.md#invitations-and-members)), and RFC 0009 makes the role names below, and the outcome "lost grant", the public names, so that the SDK's API review decides every name a developer types in one place.
- **Groups.** They are not part of the design: withdrawn by the owner (Carsten, 2026-11-09). A later RFC may bring them back (see [Groups](#groups)).
- **Deleting a synced space**, at the relay and on every member, such as an ad-hoc space that synced (RFC 0003). Deferred to a later RFC: it concerns the relay's storage and each member's copy, and needs no control body here. A local ad-hoc space that never synced is deleted locally.
- **Compaction.** A later RFC. This RFC says what compaction must keep: every control change of a space, forever. The state is a function of the whole set, and a snapshot every member can check is a design of its own. A membership entry is about 300 bytes and an envelope about 150, so a space with 100 removals keeps under 1 MB of control changes. Replacing old control changes by a signed snapshot of the control state was not taken.

## Terms

| Term | Meaning in this RFC |
| --- | --- |
| Space, member, change, header, payload, frame, log | As in RFC 0002. |
| Control change | A change of kind 1 (RFC 0002). Its payload is a control body the relay can read. It carries no content. |
| Sealed change | A change of kind 0 (RFC 0002): content or a work order, encrypted. |
| Genesis | The first change of a space. A control change. |
| Control DAG | The control changes of a space, linked by their control dependencies. |
| Causal past | Of a control change: the control changes reachable through its control dependencies. "c saw d" means d is in c's causal past. |
| Control state | What a member computes from the control changes it holds: who is a member and with which role, which key generations exist, and which person keys exist or are shredded. RFC 0006 stores it in its `control` table. |
| Role | A named set of rights in one space: reader, writer or admin. |
| Grant | A membership entry that gives a member a role. |
| Revocation | A membership entry that takes rights away: it lowers a member's role or removes it. |
| Cutoff | The highest author sequence of the revoked member that the revoking member held when it wrote the revocation. |
| Generation id | The 64-bit number in RFC 0002's header key 4 that names one space key generation. |
| Seniority | A total order on member ids, computed from the control changes; the creator comes first. |

## The design in one page

- **A space starts with a genesis.** The creator writes it. It names the creator's public keys, its certificate and the first key generation. The space id is derived from the creator's member id and a random nonce, so nobody else can write a genesis for that space (see [What it names](#what-it-names)).
- **Three roles.** A reader holds the keys and reads everything. A writer also writes content and person keys and performs erasures. An admin also changes membership, rotates the space key and signs off erasures. Rights apply to the whole space (RFC 0001).
- **The fold reads only control bodies and headers.** Whether a change is accepted depends only on the control changes and on the headers of the changes. It never depends on decrypted content, or on a check only some members can make. The relay can therefore run the same code.
- **A revocation names a cutoff.** Removing a member, or lowering its role, names the highest author sequence of that member the revoking admin held. The member's changes above it lose the revoked right on every member and on the relay.
- **Revocations win.** A revocation wins over a concurrent grant. A removed or demoted member regains a right only through a grant that saw the revocation. That grant also starts a new key generation, so writes made in between never count.
- **One order, computed from the set.** Every member puts its control changes into one order computed from the changes alone, and applies them in that order. The order of arrival never matters. Seniority settles cycles of concurrent revocations.
- **Concurrent rotations merge.** A generation id is derived from its rotation, so two rotations never share one. When a member that is no longer in the space may hold the current generation, the next admin that notices rotates again.
- **A shred is final when accepted.** Its verdict reads only its causal past, so no revocation undoes it. Members destroy the person key and the relay deletes its record as soon as they accept the shred, and the view treats the person's fields as erased at once (see [A shred is final when accepted](#a-shred-is-final-when-accepted)). One exception: in a space with one account, that account signs off its own erasure, which stays provisional for 7 days and can be taken back until an erasure decision finalizes it (see [A provisional erasure in a space with one account](#a-provisional-erasure-in-a-space-with-one-account)).
- **A newer account key reaches the fold.** After a lost device, a member carries the account's newer account key statement into the space, and the fold then refuses adds and sign-offs under the older account key (see [Account key statement entry](#account-key-statement-entry)).
- **The relay runs the same code** over the control changes it holds. Where it holds fewer changes than a member, the members decide (see [How the relay applies the rules](#how-the-relay-applies-the-rules)).

## The first change of a space

### Who creates it

Any member creates a space by writing its genesis, on its own device, offline if it likes. The creator becomes the space's first member and its first admin. A relay accepts the genesis only from a creator its operator allows, and answers `not_allowed` otherwise ([RFC 0007, "What the relay checks"](0007-relay.md#what-the-relay-checks)). Members do not check that, because it is a rule of the relay, not of the space.

### What it names

The genesis is a control change whose body (see [Genesis](#genesis)) names:

- **The space id, through a nonce.** The space id is the first 16 bytes of `H("nocentraldb/space-id" || creator member id || nonce)`, where the nonce is 16 random bytes in the body and H is SHA-256 (RFC 0003). It is 128 bits that nobody can predict, and it holds no time. Object ids are drawn at random (RFC 0002); a space id is derived instead, because a random space id does not bind the space to its creator: anyone who learns one could write a competing genesis, and with a lower change id take the space over on every member that holds both. Keeping 128 random bits, and the genesis with the lower change id wins, was not taken.
- **The creator**: its signing key, its agreement key and its member certificate (RFC 0003). Its member id is the header's author member id (RFC 0002, header key 2), which every reader recomputes from the two keys.
- **The first key generation**, by its generation id (see [Generation ids](#generation-ids)). The genesis carries no envelope. The creator writes a key envelope for itself as its second change, so that the relay's `keys` request finds it like any member's.
- **The suite**, in the header (RFC 0002, header key 8). The genesis's suite is the space's suite. v1 has one suite.
- **The model id** ([RFC 0005, "Data from a newer model"](0005-schema-and-model-lock.md#data-from-a-newer-model)). A space records the `model.id` it was created with in its first control entry, and the SDK refuses to write into a space of another model, so two apps that share a space share one model lineage. The genesis carries it in key 13, sealed, so that only members can read it (see [Genesis](#genesis)). Keeping the model id out of the genesis was not taken.

The genesis does **not** name the space's name or its description. Those are content, and the relay must not read them (principle 1).

### How the relay and members recognise it

A control change is a genesis when its entry type is genesis. It is valid when all of these hold:

- its author sequence is 1, and it has no control dependencies;
- the space id in the header equals the space id derived from the author member id and the nonce;
- its signature verifies with the signing key in its body, which hashes, together with the agreement key, to the author member id;
- the certificate verifies against the account key it names (RFC 0003).

Only the creator can write a valid genesis for its space id, because anyone else's member id derives another space id. The creator can write two, with the same nonce: an equivocation (RFC 0002). RFC 0002 keeps and folds both changes of an equivocation. A space cannot have two first changes, so this RFC narrows that rule for the genesis. Both stay in the log. The one with the lower change id, compared as bytes, is the genesis, and the other is kept with the reason "equivocated genesis" and folds nothing. A member that holds only one takes that one, and switches when it receives one with a lower id. Every member that holds both picks the same one. Only the creator's own device can cause it, by a backup restore or a clone. The alternative was not taken: both are rejected, and the space is unusable.

The relay creates a space's log when it accepts a valid genesis for a space id it does not hold, and answers `space_exists` otherwise (RFC 0007). Every other control change has the genesis in its causal past. A control change without control dependencies that is not a genesis is malformed.

## Roles and rights

Rights are per space. Every member of a space holds its keys and a full copy (RFC 0001), so no member can be kept from reading part of a space. A member holds exactly one role:

| Right | Reader | Writer | Admin |
| --- | --- | --- | --- |
| Receive key envelopes and read the whole space | yes | yes | yes |
| Acknowledge a shred entry | yes | yes | yes |
| Carry an account key statement into the space | yes | yes | yes |
| Leave the space, or lower its own role | yes | yes | yes |
| Write sealed changes: content and work orders | | yes | yes |
| Write person-key entries | | yes | yes |
| Perform an erasure: write its shred entry | | yes | yes |
| Add members; raise or lower another member's role; remove members | | | yes |
| Rotate the space key: write key chain entries and key envelopes | | | yes |
| Sign off an erasure, through its account | | only when the performer's account is the only admin account | yes |
| Finalize or take back a provisional erasure | | | only an admin of the performer's account |

- **Who may write what is checked per change**, by members and the relay alike. Each control change type needs one right (see [The control bodies](#the-control-bodies)). A sealed change needs the right to write sealed changes.
- **The sign-off of an erasure** comes from an account, not a member (RFC 0003). The account must hold an admin member in the space, in the state of the shred entry's causal past. It must not be the account of the member that performs the erasure. One exception (Carsten, 2026-11-11, NOC-228): when exactly one account holds an admin member in that state, and it is the performer's account, another account that holds a writer member there may sign off; readers may not. A team whose one admin performs the erasure can then still erase, with a second person's approval, and a reader still approves none. A space whose members all belong to one account has no second account. There the performer's own account may sign off its erasure, on one condition: the erasure is provisional for 7 days and can be taken back until then (Carsten, 2026-11-11, NOC-228, NOC-230; see [A provisional erasure in a space with one account](#a-provisional-erasure-in-a-space-with-one-account)). An erasure cannot be undone, and a reader's account should not approve one, so any account with a member in the space, other than the performer's, was not taken. "Account" here means the account's identity, its recovery key, which the account key statement in a certificate names ([RFC 0010, "The account's identity"](0010-accounts.md#the-accounts-identity)). The fold compares accounts so that a statement can only block a sign-off, never enable one (Carsten, 2026-11-11, NOC-201): the sign-off's account key and the performer's are one account when they are equal, or when any certificate in the shred's causal past, of a current or a former member, the performer's included, attaches the same recovery key to each. Key 6 sits outside the certificate's signature, so whoever writes an add can attach a statement; comparing by recovery key alone was not taken, because a statement attached to a second certificate of the performer's own account key would then make that account "another account", able to sign off its own erasure, and an erasure cannot be undone. Waiting for the statement entry that carries a newer account key statement into a space (NOC-187) was not taken either. Until that entry exists, a person's account keys before and after a recovery count as two accounts unless a certificate in the space attaches the same recovery key to both; that is no weaker than comparing account keys.
- **A reader cannot write content**, but it can read everything, and its device can leak what it read. The reader role limits writes, not knowledge (principle 4). Typical readers are adapters (RFC 0007), dashboards and auditors.
- **Agents hold roles like every member.** The SDK adds an agent as a writer unless the app asks for another role, because most agents in RFC 0001's examples write; the SDK's notice says the role in plain words in every case. Reader, unless the app asks for more, was not taken: every writing agent would become a two-step setup. No default was not taken either: the safest, but every call grows for the common case. A sub-agent in an ad-hoc space (RFC 0003) is a writer there, and its parent is the admin.
- **A space can lose its last admin**, when the last admin leaves or admins remove each other at the same time. The SDK refuses to let the last admin leave, but it cannot prevent the concurrent case. A space without an admin keeps working for content, but nobody can change its membership or rotate its key. That the most senior writer becomes admin automatically was not taken: it would give the role to someone who was never chosen for it.

What the roles do **not** do:

- They give no rights inside the space's data. Every writer may change every object of the space. Finer rights come from more spaces (RFC 0001, "What it is not").
- A revocation does not take back what a member already read (RFC 0003, "What rotation does not do").
- A role is checked against the changes a member holds. A member that has not seen a revocation yet treats the revoked member as it was, until the revocation reaches it.

**Why three roles.** Readers cover adapters, dashboards and auditors, which should not write. Writer and admin only was not taken, nor capabilities per member, which would add a check per capability on every member and the relay, for cases nobody has asked for yet; more roles can come later. Rights per collection or per object were not taken: they need a reader that enforces them on content, which the relay cannot be (RFC 0001), and more spaces give the same effect. A quorum of admins for each membership change was not taken: it needs several admins online, and a small team's space would stop when one is on holiday.

## The control bodies

Every control change carries its body as RFC 0002's payload (header key 6). The body is a CBOR map in RFC 0002's [canonical profile](0002-change-and-log-format.md#the-cbor-profile), not encrypted. Its first keys follow the layout of RFC 0002's payload map where they share a meaning:

```text
control body (CBOR map; the payload of a kind 1 change)
├── 1 entry type          uint, see the table below (RFC 0002: "key 1 names the entry type")
├── 2 clock               reserved; absent, see below
├── 3 control deps        array of change ids, sorted by bytes, no id twice:
│                         the heads of the author's control DAG when it wrote
├── 4 to 7                reserved
└── 8 ...                 the fields of the entry type
```

Key 3 means what payload key 3 means in RFC 0002: the author's heads when it wrote, restricted to control changes. Key 2 is held free for a clock. Control changes carry none, because a clock would show the relay when admins act, offline work included, which RFC 0002 keeps from the relay for every change. Control bodies that carry the author's hybrid logical clock, as payloads do, were not taken. The audit of a space's membership therefore has an order (see [Folding the membership state](#folding-the-membership-state)) but no time of writing, only the relay's time of acceptance.

| Entry type | Name | Right its author needs |
| --- | --- | --- |
| 0 | Genesis | none: it creates the space |
| 1 | Membership entry | admin; or any member, to leave or to lower its own role |
| 2 | Key envelope | admin |
| 3 | Key chain entry | admin |
| 4 | Person-key entry | writer |
| 5 | Shred entry | writer, plus an admin account's sign-off |
| 6 | Acknowledgement | reader |
| 7 | Account key statement entry | reader (Carsten, 2026-11-11, NOC-187) |
| 8 | Erasure decision | admin, of the provisional shred's performer's account (Carsten, 2026-11-11, NOC-230) |

Other entry types are reserved. A member that meets one keeps the change, parked, as RFC 0002 does for every [change it does not understand](0002-change-and-log-format.md#versioning-old-clients-keep-what-they-do-not-understand). Keys 8 to 63 of a body are "must understand", keys from 64 up "may ignore", as in RFC 0002's maps.

One membership entry with an action keeps the rules about grants and revocations in one place, so one entry type per action, with add, set role and remove as separate types, was not taken. Field numbers from 8 up leave the keys RFC 0002's payload map uses free for meanings shared with it. The entry types and field numbers are part of the wire format, so they cannot change once shipped.

**The header** of a control change has the same keys as every change (RFC 0002). Its key generation (header key 4) is the generation the author writes sealed changes with when it writes the control change, or the first generation for the genesis. The fold does not use it.

### Genesis

```text
genesis (entry type 0)
├── 8  nonce              16 random bytes; the space id derives from it
├── 9  signing key        32 bytes, the creator's Ed25519 public key
├── 10 agreement key      32 bytes, the creator's X25519 public key
├── 11 certificate        the creator's member certificate (RFC 0003)
├── 12 first generation   uint, the generation id of generation 1
└── 13 model id           bstr, required: the creator's model.id (RFC 0005), sealed (below)
```

**The model id field** (key 13), byte by byte:

```text
model id field = nonce (24 bytes) || ciphertext (16 bytes) || tag (16 bytes)     56 bytes
  key        = HKDF-SHA256(ikm = generation 1's key, salt = none,
                           info = "nocentraldb/model-id" || space id (16 bytes))
  plaintext  = model.id as its 16 raw bytes (RFC 0005 writes it as 32 hex digits)
  AEAD       = XChaCha20-Poly1305 (RFC 0003), random nonce
  associated data = "nocentraldb/model-id" || space id (16 bytes) || creator member id (32 bytes)
```

Every member can open it, because every member holds generation 1's key through the key chain (RFC 0003). The relay cannot. The associated data binds the field to its space and creator, so it cannot be copied into another genesis. `model.id` is 128 random bits, but readable in the genesis it would tell the relay which spaces share one app's model lineage, across every customer on the relay, and RFC 0002 lists the model as something the relay cannot see. No rule of the relay needs it. Sealing costs 40 bytes per space, a subkey label in RFC 0003 and a local check on decryption. Two other forms were not taken: the 16 raw bytes of `model.id`, readable by the relay; and `H(space id || model.id)`, which hides the id, but with which members could only confirm their own model, so the SDK could not tell a user which app a space needs.

Key 13 is "must understand" and required: a genesis without it is malformed. A member checks it when it folds the genesis; a model id that does not decrypt is a local check (see [What the fold may read](#what-the-fold-may-read)): the member reports it and its SDK refuses to write into the space, because it cannot tell which model the space belongs to. The subkey label `"nocentraldb/model-id"` is a new use of a space key, so RFC 0003 lists it next to its other subkeys, as its [model-id subkey](0003-keys-and-cryptography.md#model-id-subkey).

### Membership entry

```text
membership entry (entry type 1)
├── 8  action             uint: 1 add, 2 set role, 3 remove
├── 9  member id          32 bytes, the member the entry is about (the subject)
├── 10 role               uint: 1 reader, 2 writer, 3 admin; absent for remove
├── 11 signing key        32 bytes   ┐
├── 12 agreement key      32 bytes   │ add only
├── 13 certificate        map        ┘ (RFC 0003)
├── 14 from generation    uint: the generation id from which the subject's sealed changes
│                         count under this entry; for an add, and for a set role that
│                         gives the right to write
└── 15 cutoff             uint: the subject's highest author sequence the author held;
                          0 if it held none; for set role and remove
```

- **Add** gives a role to a member that is not a member in the state of the add's causal past. Its author checks the certificate first (RFC 0003). The from generation is the generation the author writes with. The author then writes a key envelope for the new member, for that generation.
- **Set role** changes a member's role. It is a grant of the new role. It is also a revocation, with a cutoff, of every right above the new role, for every grant of the same member that it did not see. A demotion therefore revokes what the member loses. A raise revokes nothing from the grants it saw, but caps a concurrent higher raise (see [The function](#the-function), step 3).
- **Remove** is a revocation of every right, with a cutoff. An admin removes others. Any member may remove itself (leave), naming its own highest sequence as the cutoff. A removal is answered by a rotation (see [Key chain entry](#key-chain-entry)). A member that leaves cannot rotate, because it must not know the next key, so the next admin that sees the removal rotates for it.
- **A re-grant** is an add of a member that was removed, or a set role that gives the right to write to a member that lost it, or to a reader. It must have the revocation in its causal past. Its from generation must be a **fresh generation**: one whose key chain entry the re-grant's author wrote, with no key envelope to the subject for it in the re-grant's causal past. Like every rule of step 1, this reads only the re-grant's causal past. The subject therefore cannot have held that key before the re-grant, unless another admin wrote it an envelope for that generation concurrently, while the subject was not a member in that admin's own view. Step 1 rejects such an envelope, and the exposure rule counts it, but a key once delivered cannot be taken back. Everything it wrote while it lacked the right is sealed under a generation it held before, which never counts under the re-grant (see [Sealed changes](#sealed-changes)). An admin that re-grants rotates first, so a re-grant costs one envelope per member. A member key serves every space of a device, so never re-adding a member id would force a new device key everywhere for one mistake, and was not taken. A re-grant without a fresh generation was not taken either: a demoted or removed member's writes in between would count again, also ones it pre-dated with high sequence numbers.

![Adding and removing a member: to add one, an admin writes a membership entry with the role, the member's public keys and its certificate, then a key envelope with the current generation wrapped to the new member; the relay sends added, and the new member pulls the log and walks the key chain back to generation 1; to remove one, an admin writes a membership entry with a cutoff, a key chain entry for a new generation that encrypts the previous keys, key envelopes for every remaining member, and re-wraps the person-key records under the new generation](0008-add-and-remove.svg)

### Key envelope

RFC 0003 fixes its content: one generation's space key, wrapped with HPKE to one member's agreement key, with an `info` that binds the space id, the generation id and the recipient's member id. Its fields:

```text
key envelope (entry type 2)
├── 8  recipient          32 bytes, the recipient's member id
├── 9  generation         uint, the generation id of the wrapped key
├── 10 encapsulated key   bytes, HPKE's enc
└── 11 ciphertext         bytes, the HPKE ciphertext of the 32-byte key
```

The recipient and the generation sit outside the wrapping, so the relay can serve each member its own envelopes (RFC 0007, `keys`). The author must be an admin. The recipient must be a member, and the generation must exist, both in the state of the envelope's causal past. Whether the wrapped key is the right key only the recipient can tell. That is a local check, not a fold rule (see [What the fold may read](#what-the-fold-may-read)).

### Key chain entry

RFC 0003 fixes its content: the previous generation's key, encrypted under the new generation, so that the holder of the new key can read the whole history. After concurrent rotations a generation can have more than one previous generation, so the entry lists them:

```text
key chain entry (entry type 3)
├── 8  generation         uint, the new generation id (see below)
├── 9  previous           array of {1: generation id, 2: that generation's key, encrypted
│                         under the new generation's key-chain subkey (RFC 0003)},
│                         sorted by generation id
└── 10 answers            array of change ids: the revocations this rotation answers
```

Each encrypted previous key is 72 bytes, `nonce (24) || ciphertext (32) || tag (16)` with a random nonce, the layout of everything a space key seals ([RFC 0003, "Key-chain subkey"](0003-keys-and-cryptography.md#key-chain-subkey)).

The author must be an admin, and every listed previous generation must exist in the state of the entry's causal past. After a key chain entry, the author writes a key envelope for every member of the state, itself included, and none for anyone else. It also re-wraps the person-key records under the new generation (RFC 0003, step 3). Whether the encrypted previous keys are correct only a holder of the new key can tell. That is a local check.

### Generation ids

RFC 0002's header key 4 is a uint that names the key generation. RFC 0003 numbers generations, and admits that two islands can rotate at the same time. If both rotations produced "generation 5", every change under generation 5 would be ambiguous: a member could not tell which key to use, and RFC 0003 forbids trying several. This RFC therefore gives the number a structure, derived so that no member can choose it:

```text
generation id (uint, 64 bits)
├── upper 16 bits   ordinal: 1 for the genesis; one more than the highest previous ordinal
└── lower 48 bits   tag: the first 48 bits of
                    H("nocentraldb/generation" || space id || author member id ||
                      canonical array of the previous generation ids)
                    with no previous generations for the genesis
```

The members and the relay recompute the tag from the key chain entry, and reject an entry whose generation id does not match. Two honest rotations differ in their author, or in their previous generations, because an author that rotates twice has seen its first rotation. Their ids are therefore different, except by a 48-bit hash collision. A member cannot steer one with a single try: the only inputs it controls are its own member id and which existing generations it lists. If two accepted key chain entries still name the same id, both are rejected, and the SDK reports it.

**What the tag does not stop.** As RFC 0003 notes, an admin that wants a collision can search about 2⁴⁸ member keys for one whose tag collides with a rotation it expects, and get that key made admin. Both rotations are then rejected. The attack blocks a rotation and exposes no key, and a malicious admin can block rotations more simply by removing the admins who would rotate. A longer tag is not worth its cost: the header's 64 bits would leave fewer for the ordinal, or the header format would have to change. Generations are related by the **generation graph**: each points to its previous generations, and "descends from" follows those links. The graph holds every key chain entry that is valid when written (see [The function](#the-function), step 1), accepted or later rejected. A rotation that a later revocation rejects still links its generation, so honest writes under it keep counting. That such changes are rejected with the generation was not taken: honest writes on an island whose admin is revoked later would be lost, and the exposure rule already makes the next admin rotate away from such a key. An entry that was never valid when written, such as one by a member that never held the admin role in its own causal past, does not count. The exposure rule counts every key chain entry and envelope in the set, whatever its verdict. Both rotations that share an id are rejected. If the first in the order kept the id, a member that grinds a colliding id could win against an honest rotation that comes later; rejecting both only blocks them, and with derived ids that needs about 2⁴⁸ tries. That the first in the order keeps the id was not taken.

The structure keeps RFC 0002's header format, and 65,535 rotations per space are enough for v1. Generation numbers only, with the lower key chain hash winning, as RFC 0003 first sketched, were not taken: two keys under one number make the changes under it ambiguous. Nor was a 32-byte generation id in the header, which changes RFC 0002's header format; nor a random tag, with which an admin could pick a tag that collides with another rotation on purpose, with one try. Rejecting every generation id that two rotations share without deriving it was not taken either: changes written on the losing island would become undecryptable, and a malicious admin could cause it.

### Person-key entry

RFC 0003 and RFC 0007 each define part of it. RFC 0003 has the entry carry the person-key id and a commitment to the key. RFC 0007 has it announce each person-key record by generation and hash, so that the relay accepts a record only with its entry. This RFC carries both:

```text
person-key entry (entry type 4)
├── 8  person-key id      16 random bytes (RFC 0003)
├── 9  commitment         32 bytes (RFC 0003)
├── 10 generation         uint, the generation id the record is wrapped under
└── 11 record hash        32 bytes, SHA-256 of the wrapped record bytes (RFC 0007)
```

The first accepted entry for a person-key id creates the person key. Every later one announces a re-wrapped record and must repeat the same commitment, or it is rejected. Whether the record's key matches the commitment only a member that unwraps it can tell. That is a local check, as RFC 0003 already has it.

**What this does not protect against.** The first accepted person-key entry in the order creates the person key. A member earlier in the order can win with its own concurrent first entry and choose the key. Honest entries are then rejected as "commitment". This adds little, since every member already reads every personal field while the key exists, and a shred never protects against a member who copied the data ([RFC 0003, "Shredding a person's key"](0003-keys-and-cryptography.md#shredding-a-persons-key)).

[RFC 0005, "Identifying the person"](0005-schema-and-model-lock.md#identifying-the-person) gives a person a person id that spans spaces. The person-key id stays per space, as RFC 0003 decided ([RFC 0003, "The person key"](0003-keys-and-cryptography.md#the-person-key)), and never derives from the person. The erasure work order finds the person-key ids of a person through the person id inside sealed data. The relay never sees the person id.

### Shred entry

```text
shred entry (entry type 5)
├── 8  person-key id      16 bytes
└── 9  sign-off           the account sign-off of RFC 0003, over the space id, the
                          person-key id and the performer's member id
```

The performer is the shred entry's author, a writer. The sign-off is valid when its account holds an admin member in the state of the shred's causal past, and that account is not the account of the performer's certificate, compared by account key and by recovery key as [Roles and rights](#roles-and-rights) says. When exactly one account holds an admin member in that state, and it is the performer's account, a sign-off whose account holds a writer member there is valid too; readers may not sign off (Carsten, 2026-11-11, NOC-228). When every member in that state belongs to the performer's account, the performer's own account may sign off, and the shred is **provisional** (Carsten, 2026-11-11, NOC-228, NOC-230; see [A provisional erasure in a space with one account](#a-provisional-erasure-in-a-space-with-one-account)). A sign-off by an account key that is superseded in the shred's causal past is not valid (see [Account key statement entry](#account-key-statement-entry)). The shred entry names no work order: the relay must not learn which sealed change was a work order (RFC 0001). The work order, inside its sealed change, names the shred entry instead.

### Acknowledgement

```text
acknowledgement (entry type 6)
├── 8  shred              change id of the shred entry
└── 9  deleted            bool: false for "seen", true for "deleted my copy"
```

A member writes two acknowledgements per shred, both as soon as it accepts the shred. The first, with `deleted` false, says it accepted it. The second, with `deleted` true, follows once it deleted its copy of the key, which it does at once (see [A shred is final when accepted](#a-shred-is-final-when-accepted)). The audit's evidence of erasure is the second kind. A further acknowledgement of the same kind, for the same shred by the same member, changes nothing. An acknowledgement by a member whose rights are revoked above the cutoff is rejected like any of its changes. The audit then shows that member as never having confirmed.

### Account key statement entry

```text
account key statement entry (entry type 7)
└── 8  statement          the account key statement of RFC 0010, as a CBOR map:
                          1 recovery key, 2 account key, 3 number, 4 signature
```

It carries a newer account key statement into a space, so that the fold itself refuses an older account key after a lost device ([RFC 0010, "Losing a device"](0010-accounts.md#losing-a-device)). The statement proves itself: the recovery key signs it under its own context string ([RFC 0003, "Account key statement"](0003-keys-and-cryptography.md#account-key-statement)), so the entry adds no signature beyond the change's own. Any member may carry it, a reader too, so that a space whose only member of the account was the lost device still learns it from another member (Carsten, 2026-11-11, NOC-187). Only members of the same account writing it was not taken, because such a space would never learn the newer key from the fold. Neither was an entry without the recovery key, which would save 32 bytes but make every member look the key up in certificates; nor an entry that carries a whole new certificate, since a certificate stays fixed in the entry that admitted its member (RFC 0010).

It is **valid when written** when, in its causal past:

- the statement's signature verifies under its recovery key;
- the recovery key is one that the certificate of a current or a former member names, so the account has a stake in the space;
- its number is higher than the number of every statement for that recovery key in the causal past, in certificates and in statement entries alike.

Two entries with the same number written concurrently are both valid. If they name the same account key, that is the same fact twice. If they name different account keys, someone who holds the recovery code made two statements, and both count. A later entry with the same or a lower number is not valid when written. What it changes is in [The function](#the-function), steps 1 and 5. A member already admitted under an older certificate stays a member: removing the lost device stays [RFC 0010's](0010-accounts.md#losing-a-device) `removeMember`. The entry costs about 140 bytes per space each time an account changes its account key.

**Example.** The account of the test vectors below changes its account key after a lost device: the recovery key names the account key of the Ed25519 secret `0x12` repeated, with number 2, and a member writes that statement into the space after the add. In CBOR diagnostic notation:

```text
{1: 7, 3: [h'087b89e7…dcc581a'], 8: {1: h'90e506d6…d438d743', 2: h'204040e3…67877c21', 3: 2, 4: h'5f1a43b3…6c59c80c'}}
```

The body is 180 bytes; the statement in key 8 is 140:

```text
a3010703815820087b89e732c9ca0c89af393851752b287291599108dc6c82ea
f934134dcc581a08a401582090e506d6c805585c4901c7b7cd36405df43b0cb4
678fc9afb7378af4d438d743025820204040e364c10f2bec9c1fe500a1cd4c24
7c89d650a01ed7e82caba867877c2103020458405f1a43b37dbd4ef73ef9f5b8
0532c5a0162426a1185cdd8d24ac2e56eb8c62a7d46296698e6617a6696fc434
8dd4180f8640f522e7252654cfa6fc4f6c59c80c
```

### Erasure decision

```text
erasure decision (entry type 8)
├── 8  shred              change id of the provisional shred entry
└── 9  final              bool: true finalizes the erasure, false takes it back
```

It ends a provisional erasure, either way (see [A provisional erasure in a space with one account](#a-provisional-erasure-in-a-space-with-one-account)). Its author must be an admin, in the state of the entry's causal past, of the account that performed the shred, and the shred must be provisional and in the causal past (Carsten, 2026-11-11, NOC-230). Any admin of the space was not taken: an account that joins later should neither take back nor finalize an erasure it never signed off. Two entry types, one to take back and one to finalize, were not taken: they would need the same check twice.

**Example.** A decision on the shred entry whose change id is `0x5d` repeated, a placeholder, with the shred as its control dependency. To finalize, 76 bytes:

```text
{1: 8, 3: [h'5d5d…5d5d'], 8: h'5d5d…5d5d', 9: true}
a40108038158205d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d
5d5d5d5d5d5d5d0858205d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d
5d5d5d5d5d5d5d5d5d5d09f5
```

To take it back, the same body with `9: false`, which changes the last byte from `f5` to `f4`:

```text
a40108038158205d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d
5d5d5d5d5d5d5d0858205d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d5d
5d5d5d5d5d5d5d5d5d5d09f4
```

**Test vectors.** `crates/ncdb-core/tests/vectors/membership/generate.py` writes the bodies above with independent libraries (cbor2 in canonical mode and cryptography's Ed25519), beside the vectors of the genesis and the membership entries: `statement-2.hex`, `statement-entry.body.hex`, `erasure-decision-finalize.body.hex` and `erasure-decision-take-back.body.hex`. The add before them is the vector `add`, whose change id is `087b89e7…dcc581a`; the recovery key is the vectors' `recovery-public`.

### What this RFC uses from RFC 0003

RFC 0003 defines the formats of what keys sign and derive ([RFC 0003, "Formats of what keys sign and derive"](0003-keys-and-cryptography.md#formats-of-what-keys-sign-and-derive)). This RFC uses them as they are:

- **The member certificate** names the account key, the member id, the kind (device, agent or sub-agent) and, for a sub-agent, the parent member id, and carries the account key statement. The account key signs a device's or an agent's certificate. A member that checks a certificate checks the chain from the account's identity: the statement against the recovery key it names, then the certificate against the account key the statement names ([RFC 0010, "The account's identity"](0010-accounts.md#the-accounts-identity)). RFC 0010 also decided that a control entry carries a newer account key statement into a space, so that the fold refuses certificates and sign-offs by an older account key: the [account key statement entry](#account-key-statement-entry) (Carsten, 2026-11-11, NOC-187). A sub-agent's certificate is signed by its parent's member signing key, under its own context string (`"nocentraldb/sub-agent-cert"`), because a parent agent does not hold the account key and must not. Its key 1 still carries the parent's account key, for attribution. A member that checks a sub-agent's certificate, in an add entry or a genesis, checks the chain: the parent's own certificate, which must be of kind agent or sub-agent and name the same account key. The parent's certificate must be in this space's control changes, in the genesis or add entry that made the parent a member, so the check reads only the control set. In RFC 0003's ad-hoc space the parent is the creator, so it always is. An add entry whose sub-agent's parent is not a member in the state of the add's causal past is not valid when written. For the sign-off below, a sub-agent counts as its parent's account. The account key, as for every other kind, was not taken for a sub-agent: a parent agent never holds the account key and must not (RFC 0003, "Sub-agents get an ad-hoc space").
- **The account sign-off** names the space id, the person-key id and the performer's member id, with the account key and its signature.
- **The key-chain subkey** is the HKDF-SHA256 subkey of the new generation's key that encrypts the previous keys in a key chain entry.

## Folding the membership state

The control state must be the same on every member that holds the same control changes, whatever order they arrived in (principle 2). The relay must reach the same state over the changes it holds. This section defines that state as a function of the set, as RFC 0004 defines a field's state. The order of arrival, or the relay's order, does not decide: it differs between members and islands, so copies would diverge. Nor does a total order of membership changes, as MLS commits have, which needs one party to order them, and island mode cannot have one (RFC 0003).

### What the fold may read

The fold reads only what every member and the relay can read: the control bodies and the headers of all changes. It never reads a decrypted payload, and never a fact that only some members can check. Those facts are:

- whether a key envelope or a key chain entry holds the right key;
- whether a person-key record matches its commitment;
- whether a sealed change decrypts.

They are **local checks**. A member that finds a wrong key reports it, with the author whose signature covers it, and the SDK shows it. The control state does not change. RFC 0002 rejects a sealed change that does not decrypt ([Rejected changes](0002-change-and-log-format.md#rejected-changes)). Every member that holds the generation's key reaches the same verdict, because AEAD decryption is deterministic. A member that holds no key for it rejects it with the reason "no key" until an envelope arrives. Fold rules that check wrapped keys or decryption were not taken: only some members can check them, so the control state would split.

### The function

Let S be the set of control changes of a space that a holder has, each with a valid signature and a canonical body, and the genesis chosen as above. The control state is `resolve(S)`, in five steps.

**1. Validity in the causal past.** A control change c is *valid when written* if `resolve` of its causal past gives its author the right its entry type needs, and c's own rules hold in that state. The own rules are: an add's subject is not a member, and a re-grant saw the revocation and names a fresh generation; an envelope's recipient is a member and its generation exists; a key chain entry's previous generations exist and its generation id matches; a person-key entry repeats the commitment of its id; a shred names a person key that exists and carries a valid sign-off; an account key statement entry holds the rules of [its section](#account-key-statement-entry); an erasure decision names a provisional shred in its causal past, its author is an admin of that shred's performer's account, a finalize holds no take-back of the shred in its causal past, and a take-back holds no finalize of it. An add is not valid when written if its subject's certificate names an account key that is superseded in its causal past, and a shred is not valid when written if its sign-off's account key is (Carsten, 2026-11-11, NOC-187). For an add, the statement its certificate carries in key 6 counts as well, as if it were in the causal past: a certificate that rests on a statement older than one the space knows names a superseded key, even where no earlier certificate named that key (Carsten, 2026-11-11, NOC-187; NOC-233). An account key K is **superseded** in a set of control changes when the set holds a statement for a recovery key R that names K with number n, in a certificate's key 6 or in a statement entry, and a statement for R with a number higher than n. The causal past is a smaller set, so the definition is well-founded. A control dependency the holder does not hold is a gap. The SDK reports it and asks for it (RFC 0002), and the causal past then counts only what the holder holds.

**2. Seniority.** A total order on member ids:

- the creator first;
- then every member that holds a grant of admin in S that is valid when written, by the smallest pair (depth, change id) over those grants. A grant's **depth** is one more than the smallest depth among the admin grants its author holds in the state of the grant's causal past; the creator has depth 0. Depth follows only the chain of admin grants, so a missing dependency elsewhere in the control DAG cannot change it. Change ids compare as bytes;
- then every other member id, by its bytes.

A grant is one deeper than the grant its author's right rested on. So an admin ranks before every admin whose only admin grants it wrote itself. A member that another admin made admin at a smaller depth ranks by that grant instead. Seniority from each revocation's own causal past was not taken: two revocations could rank their authors differently, so the order would not be total.

**3. Revocation edges.** A revocation R (a set role or a remove; a remove sets "none") *excludes* a change y of its subject when all of these hold:

- y needs a right above the role R sets;
- y's author sequence is above R's cutoff;
- y is not covered by a re-grant: y's causal past holds no grant of that right to its author that saw R. A grant of a right the author held before R does not count, because R caps it;
- y is not a shred entry: no revocation excludes a shred (see [A shred is final when accepted](#a-shred-is-final-when-accepted)).

The third condition lets a re-granted member's later control changes count again. They must have the re-grant in their causal past, so changes written before the re-grant, whatever their sequence numbers, stay excluded. Sealed changes have no control dependencies; their re-grants work through the fresh generation (see [Sealed changes](#sealed-changes)). Each revocation that is valid when written wants to come before the changes it excludes. The edges are added one revocation at a time:

- revocations in the order of their authors' seniority, most senior first; then by author sequence; then by change id;
- within one revocation, its edges in the order of the excluded changes' ids;
- an edge that would close a cycle with the control dependencies and the edges already added is skipped.

A cycle arises when concurrent revocations exclude each other. The first revocation added keeps its edges, so the senior author's revocation wins, and overlapping cycles are settled one edge at a time in this fixed order. An edge never contradicts a control dependency: a change the revocation excludes is above the cutoff, so the revocation's author did not hold it, and it is not in the revocation's causal past.

**4. The order.** The control changes are put into one order. A change comes after its control dependencies and after the revocations whose edges point at it. Among the changes that may come next, the change whose author is more senior comes first, then the lower change id.

**5. Apply.** Start from the genesis's state: its author is the only member, an admin, at the first generation. Take each change in the order and accept it when all of these hold:

1. it is valid when written (step 1);
2. its author holds the right in the state so far: the accepted changes before it in the order;
3. no accepted revocation before it in the order excludes it.

A shred entry, and an erasure decision that finalizes, are the exceptions: each is accepted when it is valid when written, and conditions 2 and 3 do not apply to it (see [A shred is final when accepted](#a-shred-is-final-when-accepted)). An accepted change updates the state. A rejected change stays in the log in RFC 0002's state *rejected* with its reason, and changes nothing. These rules apply to the whole set rather than to the order:

- A person-key entry that a shred of the same person-key id did not see is rejected when S holds an accepted shred for that id, wherever the entry falls in the order. A shred therefore wins over a concurrent re-wrap.
- Of two accepted shreds of one person-key id, the first in the order takes effect, and the second changes nothing.
- An add concurrent with an account key statement entry, neither in the other's causal past, is rejected when that entry is accepted in S and supersedes the account key the add's certificate names, wherever the add falls in the order (Carsten, 2026-11-11, NOC-187; NOC-233). An add that the entry saw stays, and so does its member: removing the lost device stays a remove. An add written by a thief before it learnt of the newer statement therefore loses to it on every copy. Rejecting an add is reversible: the subject's changes are un-folded, and nothing is destroyed. Refusing only adds whose causal past holds the newer statement was not taken, because a concurrent add would still pass.
- A take-back is rejected when S holds an accepted finalize of the same shred, wherever it falls in the order: a finalize wins over every concurrent take-back (Carsten, 2026-11-11, NOC-230). Of several finalizes, or several take-backs, of one shred, the first changes the state, and the others change nothing.

**Roles.** A membership entry that sets role r is a grant of r, and caps at r every grant of the same subject that did not see it. An add or set role sets its role; a remove sets "none". A member's role is the highest role among its accepted grants, each lowered to the lowest cap that an accepted entry not seen by that grant puts on it. A grant therefore loses to a concurrent removal or demotion, and a raise to a concurrent lower raise. It keeps the rights it gives over every entry it saw, so a re-grant that saw the revocation stands. Every cap is a revocation in step 3, with the capping entry's cutoff. The changes a member made under the higher role above that cutoff are therefore excluded wherever they land in the order, and the rule for sealed changes applies the same caps.

**The state** holds, per member id: its public keys and certificate, its role, its accepted grants with their from generations, and its accepted revocations with their cutoffs. Per generation: its id, its previous generations, its envelopes, and which member ids may hold it (see [Two removals that rotate the key at once](#two-removals-that-rotate-the-key-at-once)). Per person-key id: its commitment, its newest record's generation and hash, and whether a shred destroyed it, with its acknowledgements, or whether a provisional shred awaits its decision, was taken back or was finalized. The state has a canonical encoding: equal states encode to equal bytes, which is what the convergence harness compares (RFC 0004, C1).

### Sealed changes

A sealed change is judged by its header against the final state. The order plays no part. A sealed change by author X with author sequence s, under generation g, is accepted by the membership rule when X holds an accepted grant G of a role that writes such that:

- g is G's from generation, or descends from it in the generation graph; and
- for every accepted revocation R of X's right to write that G did not see, s is at most R's cutoff.

Otherwise the change is rejected with the reason "not a member". It is checked again whenever a control change of the space arrives (RFC 0002).

This **membership verdict** needs nothing but headers and control changes. It never waits for a sealed change that has not arrived, it never depends on which sealed changes a member holds, and every holder of the same control changes reaches the same one, the relay included. Decryption comes after it and is not part of it. RFC 0002's step 5 rejects a change with "no key" or "undecryptable", and that verdict differs from member to member: a member without the generation's key rejects with "no key" what a member with the key folds. Those rejections are local. They are checked again when an envelope arrives, and the convergence promise for them is RFC 0002's: every member that holds the key reaches the same verdict.

### Why it converges

Every step depends only on S and on the headers of the changes. The control dependencies, cutoffs and generation ids are inside the signed changes. Seniority, the edges and the order are computed from S with fixed tie-breaks: depth, author sequence and change ids, which every holder computes from the bytes (RFC 0002). No step reads the order of arrival, a clock, a decrypted payload or a check only some holders can make. Two holders of the same S therefore compute the same order and accept the same control changes. They reach the same membership verdict for every sealed change, from the same header fields. Whether a member can decrypt it is local, as [Sealed changes](#sealed-changes) says. Step 1 applies the same function to each causal past, which is a subset of S.

The fold does not need to be incremental. A space holds few control changes, hundreds rather than millions, so a holder recomputes `resolve(S)` whenever a control change arrives. RFC 0006 then re-checks every change whose verdict changed, and un-folds it where needed.

### What a new change can change

`resolve` is a function of the set, not of a growing prefix. Adding a change to S can change the verdict of a change accepted before. That is the point of a revocation that arrives late. It happens in exactly these ways:

- a revocation arrives that excludes the change, or that excludes a grant its author's right depends on;
- a concurrent set role arrives that caps a grant, with a cutoff below changes made under that grant;
- a grant arrives that gives some member an admin grant of smaller depth, so a cycle is settled the other way;
- a missing control dependency arrives that a change's validity when written rests on;
- a shred arrives that a person-key entry did not see;
- a change arrives that moves which of two shreds of one person-key id comes first in the order, which changes only which of them the state names, since both are accepted and destroy the same key;
- an equivocated genesis with a lower change id arrives late, and becomes the genesis (see [How the relay and members recognise it](#how-the-relay-and-members-recognise-it));
- a key chain entry arrives that names the same generation id as an accepted one, and both are rejected (see [Generation ids](#generation-ids));
- an account key statement entry arrives that is concurrent with an add, and supersedes the account key of the add's certificate, so the add is rejected (see [Account key statement entry](#account-key-statement-entry));
- a finalize arrives that a take-back of the same provisional shred did not see, so the take-back is rejected and the erasure is final (see [A provisional erasure in a space with one account](#a-provisional-erasure-in-a-space-with-one-account)).

**Un-folding.** A later control change can make a check fail that a change has already passed, in exactly the ways listed above. A sealed change then moves from folded to rejected, and RFC 0006 un-folds it ([Control changes and changes that wait for them](0006-local-view.md#control-changes-and-changes-that-wait-for-them), "Un-folding"). A control change that loses its verdict is recomputed through `resolve`; its effects follow at once, and they can all be undone. The one effect that cannot, destroying a person key, belongs to a shred, or to the finalize of a provisional one, and neither loses its verdict once it is accepted (see [A shred is final when accepted](#a-shred-is-final-when-accepted)). Un-folding is allowed because without it, a revocation that arrives late could not undo what a stolen device wrote meanwhile.

Every such change has to be concurrent with the change whose verdict flips, or with one of the changes it rests on. Members see verdicts change while concurrent control changes are still arriving, and the SDK reports each change. That is the price of island mode under every option.

### The cutoff

A revocation's cutoff is the subject's highest author sequence that the revoking member held when it wrote the revocation, counting sealed and control changes alike. The subject's changes at or below it are **inside**; those above it are **excluded** and lose the rights the revocation took away. A writer demoted to reader keeps acknowledging shreds; a removed member keeps no right.

The cutoff is a number in a control body, and the author sequence is a number in every header. Members and the relay therefore compute the same verdict, without decrypting anything. The rule is safe against a member that lies about its control dependencies: it cannot change what the revoking member held. A change written after the revocation carries a higher sequence and is excluded.

**What the cutoff does not stop.** A revoked member can write a new change under a sequence number at or below its cutoff that it already used. That is an equivocation (RFC 0002). Every member detects it once it holds both changes, and the SDK reports it, but the second change is inside the cutoff and is accepted, as RFC 0002 accepts both changes of every equivocation. The revoked member holds only generations up to the revocation's rotation, so such a change is sealed under an old generation. The SDK marks it as written by a revoked member under an old generation.

A control change inside the cutoff still counts only where its author holds the right in the state so far (step 5). Whether a revoked admin's fork comes before or after its revocation in the order can depend on its change id: a fork has the same causal past as the change it duplicates, and step 4 puts the lower change id first. A fork that sorts first can come before the revocation, while its author is still an admin, and keep its effect; one that sorts later has no right and is rejected. Every copy reaches the same verdict, but the change id decides it, and a revoked device can write forks until one sorts first. The harness pins this case (NOC-226). It cannot undo an erasure, because no revocation excludes a shred.

**Why a cutoff.** It needs only headers, so members and the relay reach one verdict. The same cutoff plus a hash over the subject's change ids up to it was not taken: a member can check it only when it holds all of them, which splits verdicts between members. Counting the changes the relay accepted before the revocation, and refusing every change from the removal on, was not taken: it depends on the relay's order, which members on an island do not have, and refuses changes the remover had already received. A cutoff checked against the dependencies inside sealed changes was not taken: only members that decrypt and hold those changes can check it, so members and the relay would reach different verdicts. Revoking by time was not taken: the clock is encrypted, and a removed member can write any clock.

### A shred is final when accepted

The view can always be recomputed, so un-folding a change costs nothing but work. One effect of a control change cannot be undone: destroying a person key. A shred entry's verdict therefore reads only its causal past, and once a holder accepts a shred, no change that can still arrive rejects it (Carsten, 2026-11-11, NOC-224):

- **A shred is accepted when it is valid when written** (step 1): in the state of its causal past, its author holds the right to perform an erasure, the person key exists, and the sign-off is valid. No revocation excludes a shred (step 3), and step 5 does not ask whether its author still holds the right in the state so far. The two rules for the whole set still apply: a shred wins over a person-key entry it did not see, and of two shreds of one person-key id the first in the order takes effect. Both are accepted and destroy the same key.
- **Every holder reaches that verdict as soon as it holds the shred and its causal past**, whatever else it holds, members and the relay alike. A late equivocated genesis does not change it either, since step 1 reads only the shred's causal past and the genesis in it.

| Control change | Its effects, all at once |
| --- | --- |
| Shred entry | The fold treats the person's fields as erased on every member (RFC 0003, step 4). The member writes its "seen" acknowledgement, deletes its copy of the person key and writes its "deleted" acknowledgement. The relay deletes the person-key record. |
| Key envelope | The member unwraps the key into its keystore and can decrypt. If the envelope is rejected later, the member keeps the key only to read changes already accepted under it, and never writes with it. |
| Provisional shred entry | The fold treats the person's fields as erased on every member. The member writes its "seen" acknowledgement and keeps the person key; the relay keeps the record. |
| Erasure decision that finalizes | What a shred entry does, for its provisional shred: the member deletes its copy of the person key and writes its "deleted" acknowledgement, and the relay deletes the record. |
| Erasure decision that takes back | The fold reads the person's fields again. Nothing irreversible: a concurrent finalize that arrives later erases them again. |
| Membership entry, key chain entry, person-key entry, acknowledgement, account key statement entry | They change the control state; the relay changes whom it serves. Nothing irreversible. |

A member that is offline destroys the key when it syncs and accepts the shred. Until then it holds the key, as RFC 0003 says of every device that has not come back.

**The price.** A concurrent revocation of the performer no longer stops an erasure: the shred stands even when a revocation the performer did not see removed it. The sign-off is the whole defence. A second account, which holds an admin member in the shred's causal past, signed off this performer and this person key (see [Shred entry](#shred-entry)). A thief with a stolen writer device therefore cannot erase without that account. A thief who also controls such an account can erase a person's data for good: a loss of data, not a leak, and such a pair can already remove members.

**Waiting until the shred is stable was not taken**, which was this RFC's earlier rule. A shred was stable once every admin and former admin had written a control change after seeing it, or was bounded by a stable revocation, and its key was destroyed only then. That rule holds only against admins that are honest or already revoked. An admin device that is stolen and not yet removed can still reject a stable shred: by a new change whose dependencies and cutoff leave the shred out, which nobody can check, or by an equivocation under a sequence the rule already counted, which is inside every cutoff (NOC-224; the harness pins both, NOC-226). The wait also let one lost admin device keep a key alive for as long as it stayed away. Two repairs were not taken either: refusing a second change under a counted sequence, because holders differ in what they call stable, so the state would split; and treating an admin's equivocation as a revocation of that admin, because the device chooses where to fork and can exclude a grant the shred rests on.

### A provisional erasure in a space with one account

In a space whose members all belong to one account, no second account can sign off an erasure. There the performer's own account signs off, and the erasure is **provisional** for 7 days: the person shows as erased at once and can be restored, and only then is the erasure final (Carsten, 2026-11-11, NOC-228, NOC-230). Every other erasure stays final when accepted.

**Final is a change in the log, not a timer.** Control changes carry no clock (see [The control bodies](#the-control-bodies)), so no two copies agree on "7 days after the shred", and destroying the key is the one effect that cannot be undone. A provisional erasure therefore becomes final only through an [erasure decision](#erasure-decision) that finalizes it. Members destroy the key, and the relay deletes the record, only once they accept that decision. The decision is ordered and convergent like every control change.

- **Who writes the finalize.** The SDK of a device of the performer's account writes it, as an admin of the space, once that device has held the provisional shred for 7 days by its own clock.
- **The relay's gate.** The relay refuses a finalize that arrives less than 7 days, by the relay's own clock, after it accepted the shred, with the code `too_early`; the SDK tries again later ([RFC 0007, "What the relay checks"](0007-relay.md#what-the-relay-checks)). A thief with a device of the account therefore cannot finalize at once. In island mode, without the relay, the 7 days rest on the device's own clock.
- **The take-back.** Within the 7 days, an admin of the performer's account takes the erasure back with an erasure decision whose key 9 is false. The person's fields are then read again on every member.

**Convergence.** A provisional shred has three outcomes, each a function of the set of control changes:

- **Final** when S holds an accepted finalize of it. A finalize is accepted when it is valid when written, so no change that can still arrive rejects it, as for a shred.
- **Taken back** when S holds an accepted take-back of it and no accepted finalize.
- **Provisional** otherwise.

A take-back that arrives after a device destroyed the key can only be concurrent with the finalize that device accepted, and a finalize wins over every concurrent take-back ([The function](#the-function), step 5). Every copy therefore ends final. No copy destroys a key without an accepted finalize, so a device's 7 days running out decides nothing on its own. A take-back accepted before a concurrent finalize arrives is overturned by it: the view erases the fields again, which only the view changes, since the key still exists.

**While provisional,** the view treats the person's fields as erased, members keep the person key and the relay keeps its record. The SDK reports each provisional erasure in the space status, with its age on the device and whether the device's account may take it back. Every device of the account, not only the performer's, gets a notice when it first holds a provisional shred, which names the performing device and the 7 days ([RFC 0009, "Sync and status"](0009-sdk-api.md#sync-and-status)). An app must show it: it is how the account's owner sees, on another device, an erasure by a stolen one, while it can still be taken back.

**A second account joins within the 7 days.** The erasure stays provisional and finalizes as before: its sign-off was valid in its causal past, the same reading as a shred's. The new account sees it in its status, but may neither take it back nor finalize it. An erasure that lapses unless the new account signs it off was not taken: a thief could then cancel one by adding a member. Neither was refusing to add an account while an erasure is provisional, which would block a team for up to 7 days.

**What this costs.** A wire change: the erasure decision, entry type 8. The relay keeps when it accepted each provisional shred, one row per shred, and answers a new code. If no device of the account comes back online, the erasure stays provisional and the key is never destroyed: safe for the data, not for the erasure, and the status shows it. Older cores reject a shred that the performer's own account signed off, so they show the person's data until they are upgraded and never destroy the key early. Copies agree once every member runs a core that knows the rule.

**Not taken.** Each device counting 7 days and deleting the key on its own: copies would diverge for good when a take-back arrives after one device deleted its key. The relay alone telling members that an erasure is final: members would take an irreversible step on the relay's word, with no signed record in the log, and island mode would never finalize. A person confirming each finalize: it would not meet "final after 7 days" without a person.

### Concurrent add and remove of one member

Alice removes Mallory's laptop, which was stolen. At the same time, on another island, Bob, who has not seen the removal, raises the laptop's role from writer to admin. Both entries are valid when written.

Bob's entry is a grant that did not see the removal, so Alice's removal caps it at "none". On every copy the laptop is removed. Every change it made above Alice's cutoff is rejected, whether it reached a member before or after the removal. Bob's entry stays accepted but without effect, and the SDK shows it as a **lost grant**, the way RFC 0004 shows a lost removal.

This is the opposite of RFC 0004's sets, where an addition wins over a concurrent removal. For data, a lost addition is data that is gone. For membership, a lost removal leaves a stolen device writing. So the removal wins, and Bob can repeat the grant once he has seen the removal and still wants it. The rule that the grant wins, as an addition does in RFC 0004's sets, was not taken: a lost grant is cheap to repeat. Concurrent role changes follow the same rule: when two admins set a member's role at the same time, the member ends with the lower role. Each set role carries a cutoff, so whatever the member did under the higher role above that cutoff is excluded on every copy, wherever it lands in the order.

### Two removals that rotate the key at once

Alice removes Xavier and rotates the key. At the same time, on another island, Bob removes Yvonne and rotates. Alice's new generation has an envelope for Yvonne, and Bob's has one for Xavier. When the islands meet, every member holds both removals and both key chain entries:

1. Both removals are accepted; neither excludes the other. Xavier and Yvonne are removed on every copy.
2. Both generations exist. Their ordinals are equal and their ids differ, because their authors differ.
3. The **current generation**, which members write sealed changes with, is a head of the generation graph: a generation that no accepted key chain entry lists as previous. Among the heads, it is the one with the highest ordinal, then the one whose key chain entry comes first in the order. Every member picks the same one. A generation that two key chain entries name is never a head: both entries are rejected (see [Generation ids](#generation-ids)), so neither can be the one writers seal under, and the generations before it stay heads. The first generation, which no entry can name, keeps at least one head in every graph. Letting it be current was not taken: members would seal under an id that may name two keys. A generation whose key chain entry was valid when written but is rejected later is still a head and can be current: changes sealed under it keep counting, so the space keeps working until the next rotation, and step 5 rotates away from it once it is exposed. Leaving rejected rotations out of the heads was not taken: a generation a removed admin holds would never be rotated away from. A member that holds no envelope for it writes with the newest generation it holds, and the SDK reports that it waits for an envelope.
4. **Who may hold a generation**: the author of its key chain entry, every recipient of any envelope for it, and everyone who may hold a generation that descends from it. Every envelope in S counts, accepted or rejected, because a rejected envelope may still have been delivered. The creator holds the first generation, which has no key chain entry. The authors counted are those of every key chain entry the generation graph holds, valid when written, whatever its final verdict; an entry that was never valid when written adds no generation, so its author is not counted. The author of an envelope is not counted for it, since an admin that wraps a key holds it through its own entry or envelope. Counting only accepted envelopes was not taken, because a rejected envelope may still have been delivered; nor was counting the authors of entries that were never valid when written, because a removed member could then force rotations by naming the current generation. A generation is **exposed** when anyone who may hold it is not a member of the state.
5. When the current generation is exposed, an admin writes a **merge rotation**: a key chain entry that lists every head as previous, with envelopes for the current members only. The SDK of any admin that sees an exposed current generation does so after a random delay of up to a minute, unless it sees another admin's merge rotation first. Exposure means a removed member may read new changes, and an admin's app may not be built to handle a report; it costs one rotation per exposure, and rarely two. Leaving it to the SDK to report the exposure while the app asks an admin to rotate was not taken.
6. Two merge rotations written at the same time are concurrent rotations again, and step 5 repeats. Each round needs two admins to rotate within the same minute without seeing each other.

**What this does not do.** Until the merge rotation lands, members write under an exposed generation. Xavier or Yvonne could read those changes if they obtained them. The SDK reports the exposure until it ends. If no admin is online, it lasts until one is. Envelopes that a holder never received cannot count: a removed admin's envelope that never left its island makes a generation exposed only where it arrives. The same rule covers a member that left (it cannot rotate for itself) and a removal whose author never rotated, for example because its device crashed between the two changes.

### Two admins who remove each other

Alice removes Bob, and at the same time Bob, or whoever holds Bob's device, removes Alice. Each removal excludes the other, because each is above the other's cutoff. Step 3 adds the senior author's edges first. If Alice became admin first, her removal comes before Bob's. Then Bob is removed, and Bob's removal of Alice is excluded and rejected. Every member computes the same outcome.

In a ring of three, where A removes B, B removes C and C removes A at the same time, with A the most senior: A's and B's edges are added, and C's edge, which would close the cycle, is skipped. In the order, A's removal is accepted, B's is excluded, and C's is accepted, because nobody whose revocation is accepted revoked C. A and B are removed, C stays, and A's removal of B stands, because it comes before C's removal in the order.

Seniority is not a judgement of who is right. It is a fixed way to keep one side of a cycle, so that a compromised admin cannot cancel its own removal by removing the remover at the same moment. Both of two mutual removals taking effect was not taken: a stolen admin device would take an honest admin down with it, and could leave the space without an admin. Neither of two mutual removals taking effect was not taken either: a compromised admin would cancel its own removal. It protects only against a junior attacker. A compromised senior admin wins the cycle. Another admin, or the creator, must then remove it again once the honest admin reports it.

### A member removed while offline who keeps writing

Carol's laptop is offline for a week. Meanwhile Alice removes it, naming as cutoff the laptop's highest sequence she held, 40. The laptop keeps writing, sequences 41 to 60, under the old generation, the only one it holds.

- **On the relay.** When the laptop reconnects, it pulls first (RFC 0007, island mode), sees its removal, and the SDK reports it. If it pushes anyway, the relay refuses sequences 41 to 60: they are above the cutoff.
- **On members that received them directly**, on the laptop's island: when Alice's removal reaches them, the fold rejects sequences 41 to 60 and un-folds what they had applied (RFC 0006). Their state then equals that of every member that holds the same changes.
- **On the laptop itself.** It folds its own removal like any member, and its own changes above the cutoff become rejected in its log. The SDK tells the app that no member accepts the changes made since the cutoff. The laptop keeps its copy of everything it received before.

Changes the laptop wrote before Alice's removal but above sequence 40 are rejected too, because Alice had not received them. Neither the relay nor the members can tell an honest late change from a backdated one: the clock is encrypted, and the sequence number is the only order they can check. The SDK warns an admin who removes a member whose recent changes it has not received yet.

Changes at or below the cutoff that arrive late are accepted everywhere, by the relay too.

### A member demoted and raised again

Alice lowers Dan from writer to reader, with cutoff 70. Dan, offline, keeps writing as if he were a writer, under generation 7, which he holds. Later Alice raises Dan to writer again.

The raise is a re-grant. Alice writes a key chain entry for a fresh generation 8 first, then the raise, with generation 8 as its from generation, then the envelopes, Dan's after the raise. Dan's sealed changes above 70 under generation 7 never count: generation 7 does not descend from generation 8, and the demotion caps every grant Dan held before. His changes under generation 8 and its descendants count from the raise on. He could not have written under generation 8 before, because he had no key for it. If another admin rotated to a concurrent generation 8′ while the raise was on its way, Dan's changes under 8′ do not count either, until a merge rotation gives him a descendant of both. That loses honest writes in a rare case, and the SDK reports them as rejected.

### What this does not do

- **A member that has not seen a revocation** treats the revoked member as it was. It accepts the member's changes until the revocation arrives, and then rejects those above the cutoff (RFC 0006, un-folding). The app sees those changes appear and disappear.
- **A revoked member keeps its copy.** Revocation and rotation keep it from writing, and from reading what is written under later generations. It still holds what it received (RFC 0003).
- **A revoked member's equivocations inside its cutoff are accepted**, as RFC 0002 accepts every equivocation. The SDK reports them (see [The cutoff](#the-cutoff)).
- **Grants a revoked admin made inside its cutoff stay.** If a stolen device added a member before the removing admin had received that add, the added member stays until an admin removes it too. The SDK lists the members a removed admin added, so that the remover can review them.
- **Verdicts are not final**, except a shred's. Concurrent control changes can still flip them, as [What a new change can change](#what-a-new-change-can-change) lists. An accepted shred stays accepted (see [A shred is final when accepted](#a-shred-is-final-when-accepted)).
- **Nothing is enforced at write time.** Two admins can make decisions that contradict each other; the rules decide afterwards, the same way on every copy.

## How the convergence harness checks it

The control state is treated like a field type under RFC 0004's plugin contract. Its state is `resolve(S)`. Its apply adds one control change to S. Its read is the membership the SDK shows. Its outcome per change is **accepted**, **rejected** with a reason, or **lost** for a grant that a revocation it did not see capped. The convergence harness (NOC-9) and its plugin-contract checks (NOC-19) run it with simulated admins, writers and readers on several islands. Each member writes membership entries, envelopes, rotations, person-key entries, shreds, acknowledgements and sealed changes from its own view. The harness delivers the union to at least three copies and to a simulated relay, in random orders with duplicates, and checks:

| # | Property | How the harness checks it |
| --- | --- | --- |
| M-C1 | Converges in any order: equal sets give equal control state bytes, equal verdicts for every control change, and equal membership verdicts for every sealed change. | Compares the canonical state bytes and those verdicts across copies (RFC 0004, C1 and C4). Rejections for "no key" are compared only between copies that hold the same keys. |
| M-C2 | Idempotent. | Duplicates in every delivery; the whole set applied twice. |
| M-C3 | Deterministic natively and in WebAssembly. | The same seed on both targets (NOC-10). |
| M-C4 | The fold reads only control bodies and headers. | Gives copies the same control changes but different subsets of the sealed changes, and gives the relay simulator no keys. The control state and the membership verdict of every sealed change both copies hold must be equal. |
| M-C5 | Local checks never split the state. | Mixes in envelopes and key chain entries that hold wrong keys, and person-key records that miss their commitment. The control state must be equal on every copy. Only the local reports differ, and only on the members that can tell. |
| M-C6 | Seniority and the order are total. | Generates mutual removals, rings and overlapping cycles of concurrent revocations, ties of depth, and missing dependencies. Checks the outcomes against a reference implementation of steps 2 to 5. For every pair of sets S ⊂ S′, checks that every verdict that differs between them is explained by one of the causes in [What a new change can change](#what-a-new-change-can-change). |
| M-C7 | Writes without the right never come back, and re-granted writes do count. | Revoked members keep writing sealed and control changes under every generation they hold, and pre-write high sequence numbers. They are re-granted and raised again, two admins raise a member to different roles at once, and admins try to reuse a generation id. No write without the right may be accepted on any copy. Every change a re-granted member writes after seeing its re-grant must be accepted. |
| M-C8 | An accepted shred stays accepted. | Whenever a copy accepts a shred, the harness adds further generated changes: concurrent revocations of its performer and of the admins its sign-off rests on, equivocations of control changes under sequences already used, and admins whose dependencies and cutoffs leave the shred out. The shred must stay accepted in every extension. |
| M-C9 | Exposure counts every envelope. | Rejected rotations and envelopes from removed admins, delivered to some copies only. Once a merge rotation is accepted, no one outside the state may hold the current generation in any copy that holds those envelopes. |
| M-C10 | Shreds are ordered. | Two shreds of one id, and shreds concurrent with re-wraps. Exactly one shred takes effect, and no re-wrap it did not see is accepted. |
| M-C11 | The relay agrees. | After every delivery, the relay simulator's membership table equals `resolve` of its own log. Every change it refused is one that the members also reject for the same S, or one listed in [How the relay applies the rules](#how-the-relay-applies-the-rules). |
| M-C12 | The checks fire. | Broken fixtures the harness must reject: add wins for membership; arrival order instead of the computed order; a revocation without its cutoff; seniority by arrival; a re-grant without a fresh generation; exposure that counts only accepted envelopes; a revocation that excludes a shred; a take-back that wins over a concurrent finalize; an add whose superseded account key is accepted. |
| M-C13 | A finalize wins over every concurrent take-back. | Provisional shreds with finalizes and take-backs from several devices of the account, concurrent and in sequence, delivered in any order. Every copy reaches the same outcome, no copy destroys a key without an accepted finalize, and none restores a person whose key a copy destroyed (Carsten, 2026-11-11, NOC-230). |
| M-C14 | A newer account key statement wins over a concurrent add. | Account key statement entries concurrent with adds whose certificates name the superseded account key, and with sign-offs by it. Every such add is rejected on every copy in any order; a sign-off is judged by its shred's causal past only (Carsten, 2026-11-11, NOC-187). |

## How the relay applies the rules

The relay holds no key. It does hold every accepted control change of a space, and their bodies, cutoffs and control dependencies are readable (RFC 0002, kind 1). It therefore runs the same `resolve`, from the same Rust code that the members run, over the control changes in its log. Its membership table is `resolve` of its log, recomputed with each append of a control change. That table holds roles, grants with their from generations, revocations with their cutoffs, and the generation graph. RFC 0007's membership table holds them ([RFC 0007, "What the relay checks"](0007-relay.md#what-the-relay-checks)). The relay's membership table as the truth was not taken: members could not check it, and island mode would stop every membership change.

**On a push**, the relay checks each change against its current state, in addition to RFC 0007's checks:

- **A control change** is accepted when `resolve` over the relay's log plus that change accepts it. Dependencies the relay does not hold count as gaps, as on a member.
- **A sealed change** is accepted by the rule in [Sealed changes](#sealed-changes), which needs only its header: author, author sequence and generation id.
- **The uploader** must be a current member (RFC 0007).
- **A person-key record** needs its person-key entry. It replaces the stored record when its generation is the stored record's generation or descends from it, because generation ids that carry a tag have no "newer" ([RFC 0007, "Keys and the person-key store"](0007-relay.md#keys-and-the-person-key-store)).
- **A shred's record deletion** happens as soon as the relay accepts the shred entry, by the same rule as on a member (see [A shred is final when accepted](#a-shred-is-final-when-accepted)). For a provisional shred, it happens once the relay accepts the erasure decision that finalizes it.
- **A finalize that comes too early** is refused with `too_early` when it arrives less than 7 days, by the relay's clock, after the relay accepted its provisional shred. The relay keeps that time per provisional shred ([RFC 0007, "What the relay checks"](0007-relay.md#what-the-relay-checks); Carsten, 2026-11-11, NOC-230). This is the one check the members cannot repeat, since they share no clock. The members still decide whether the erasure is final; the gate only delays what the relay accepts.

Where the relay and the members can disagree, and who decides:

| Case | Relay | Members | What follows |
| --- | --- | --- | --- |
| A revocation arrives after the relay accepted a change it excludes. | Has accepted it, and never removes an entry from its log. | Reject it when they hold the revocation. | The relay's table follows the revocation; the stale entry stays as bytes. The members decide. |
| The relay refused a change because of a revocation that a concurrent change later makes ineffective. | Refused it at the time. | Accept it once they hold both. | RFC 0002's rule alone would never push it to that relay again. A member pushes it again, once, whenever its own fold newly accepts it after a control change arrived, and the relay accepts it then. |
| Changes that never reach the relay, for example exchanged only on an island. | Does not know them. | Fold them. | Members that hold different sets can differ, as RFC 0002 states. |
| A key or record is wrong, or a sealed change does not decrypt. | Cannot tell. | Report it, or reject the sealed change; the signature names the author. | Local checks; the control state does not change. |
| A space is created by a creator the relay operator does not allow. | Refuses the genesis with `not_allowed` (RFC 0007). | Do not check it. | The space works in island mode and on relays that allow its creator. |

The relay decides nothing the members cannot check. It can refuse a change the members later accept; the members then push it again. Without that, a change that a concurrent revocation made the relay refuse, and that the members later accept, would never reach the members that sync only through the relay; it cannot loop. Keeping RFC 0002's rule, never to push it to that relay again, was not taken.

**What the relay learns from this RFC**, in addition to the lists in RFC 0002 and RFC 0007:

- **Roles.** Each member's role in each space, and when it changes.
- **Cutoffs.** For each revocation, how many of the subject's changes the revoking admin had received.
- **Who saw what.** Through the control dependencies: which control changes each admin had seen when it wrote, so how up to date each admin was, and which admins acted on separate islands.
- **Accounts across spaces.** The account key and its statement in each member certificate, so which devices and agents belong to one person or organisation, in every space the relay serves.
- **The sub-agent tree.** Which member is a sub-agent of which parent, from the certificates.
- **Erasure approvers.** Which accounts sign off erasures, and in which spaces; which erasures were provisional, and when each was taken back or finalized.
- **Account key changes.** From account key statement entries, when an account changed its account key, which RFC 0010 already lists.
- **The generation graph.** When concurrent rotations happened, and which merge rotations followed.

It still learns no content, no space name and no model id, which the genesis seals.

Control bodies and control dependencies are readable so that the relay applies the same rules. Hiding the control bodies from the relay, so that only members apply the rules and the relay checks only signatures, was not taken: anyone with a valid key in the space could fill its log with changes every member rejects, and the relay could not stop a removed member. Of the list above, the account key weighs most, because it links one person's devices and agents in every space on that relay.

## Groups

Groups are not part of this design: they were withdrawn by the owner (Carsten, 2026-11-09), who decided to keep them out of every definition for now, because they caused more problems than they solved. A member is a person's device, an agent or a sub-agent, and nothing else. There is no group in the control state, and the SDK expands no list of members into a space. An admin who gives several members the same spaces adds each of them to each space, and removes each of them from each space; [RFC 0009](0009-sdk-api.md) keeps adding several members in one call as plain batching. If a later need shows that groups are useful, they come back through a later RFC. Designing groups now, as a principal with its own control state, was not taken: a group's membership lives outside the space, and folding one space must not depend on another space's changes.

Such an RFC has to answer what makes a group that is a member of many spaces hard to design:

- **A group's membership lives outside the space.** If a space's control state depended on a group's membership, which changes in another log, then folding one space would depend on another space's changes. RFC 0002 keeps every change, and every fold, inside one space. Crossing spaces needs its own convergence argument.
- **Removing someone from a group rotates every space of the group.** With per-member envelopes ([RFC 0003, "Group key distribution: options compared"](0003-keys-and-cryptography.md#group-key-distribution-options-compared)), that is one rotation per space and one envelope per remaining member in each. A group key would reduce the envelopes, but it adds a second layer of keys and rotations to design.
- **The relay would have to check membership through the group**, which needs the group's control state on every relay that holds any of its spaces.

## Agreement with GENESIS.md and the other RFCs

This RFC agrees with GENESIS.md and RFC 0001: the relay checks membership without reading content; every member, an agent included, is visible in the member list with its own key and role; rights are per space; a removed member keeps its copy; and the rules converge in any order. It makes them, and the other RFCs, more precise in these points, each stated where the other text says it:

- **Groups** (GENESIS.md, "Spaces"; RFC 0001, "The design in one page") were members of many spaces in the first texts. They were withdrawn by the owner (Carsten, 2026-11-09), and GENESIS.md, RFC 0001 and the FAQ no longer name them ([RFC 0001, "Groups leave the design"](0001-nocentraldb.md#groups-leave-the-design)).
- **What the relay sees.** The relay cannot see which changes a sealed change builds on, but it sees the control dependencies of control changes and the items in [How the relay applies the rules](#how-the-relay-applies-the-rules) ([RFC 0002, "What the relay can see"](0002-change-and-log-format.md#what-the-relay-can-see-and-what-it-cannot); [RFC 0001, "The design in one page"](0001-nocentraldb.md#the-design-in-one-page)). The FAQ lists them too.
- **Space ids** are derived from the creator's member id and a nonce; object ids stay random ([RFC 0002, "Space"](0002-change-and-log-format.md#space)).
- **The key generation** in RFC 0002's header key 4 has a structure, ordinal and tag; the format does not change ([RFC 0002, "Key generation, suite and kind"](0002-change-and-log-format.md#key-generation-suite-and-kind)).
- **Equivocation.** Of two geneses, both are kept but only the one with the lower change id is the genesis; a revoked member's equivocations inside its cutoff are accepted ([RFC 0002, "Author and author sequence"](0002-change-and-log-format.md#author-and-author-sequence)).
- **Payload keys.** A control body keeps key 1 as the entry type and key 3 as the dependencies, and leaves key 2, the clock, absent ([RFC 0002, "Key generation, suite and kind"](0002-change-and-log-format.md#key-generation-suite-and-kind)).
- **Pushing again.** A member pushes a change the relay refused again once its own fold accepts it ([RFC 0002, "A change the relay refuses after the member folded it"](0002-change-and-log-format.md#a-change-the-relay-refuses-after-the-member-folded-it)).
- **Concurrent rotations** get derived generation ids, exposure and the merge rotation ([RFC 0003, "Removing a member and rotating the space key"](0003-keys-and-cryptography.md#removing-a-member-and-rotating-the-space-key)).
- **The sign-off** of an erasure comes from an account with an admin member in the space, or with a writer member when the performer's account is the only admin account, or, provisionally for 7 days, from the performer's own account when it is the space's only account ([RFC 0003, "Shredding a person's key"](0003-keys-and-cryptography.md#shredding-a-persons-key)).
- **Account key statements** reach the fold through the account key statement entry, as RFC 0010 decided ([RFC 0010, "Losing a device"](0010-accounts.md#losing-a-device)).
- **Formats.** RFC 0003 defines the member certificate, the account sign-off and the key-chain subkey, and this RFC uses them, including the sub-agent certificate its parent signs ([What this RFC uses from RFC 0003](#what-this-rfc-uses-from-rfc-0003)).
- **The person-key entry** carries RFC 0003's commitment and RFC 0007's record hash and generation ([RFC 0003, "The person key"](0003-keys-and-cryptography.md#the-person-key); [RFC 0007, "Keys and the person-key store"](0007-relay.md#keys-and-the-person-key-store)).
- **The relay's membership table** holds roles, grants with their from generations, revocations with their cutoffs and the generation graph. The relay checks author sequences against cutoffs, replaces a person-key record only with the same or a descendant generation, and deletes a record as soon as it accepts its shred ([RFC 0007, "What the relay checks"](0007-relay.md#what-the-relay-checks)).
- **RFC 0006's control table** holds what [The function](#the-function) lists. Control changes are recomputed through `resolve`, and a shred's effects follow at once ([RFC 0006, "Control changes and changes that wait for them"](0006-local-view.md#control-changes-and-changes-that-wait-for-them)).
- **The model id** stands in the genesis, key 13, required and sealed; RFC 0003 defines its subkey ([RFC 0003, "Model-id subkey"](0003-keys-and-cryptography.md#model-id-subkey)). A model id in every change's payload is RFC 0002's and RFC 0005's to carry out, not this RFC's ([RFC 0002, "The writer's model"](0002-change-and-log-format.md#the-writers-model)).
- **Principle 3 and the team lead's erasure.** GENESIS.md promises that erasure destroys a key. Here every member destroys it as soon as it accepts the shred, and the relay deletes its record then; a device that is offline destroys it when it syncs. RFC 0001 names that limit in [its team lead's list](0001-nocentraldb.md#the-team-lead-who-has-to-pass-the-data-protection-review), and RFC 0003 and the FAQ alongside it.
- **Add wins** stays the rule for RFC 0004's sets. Membership resolves the other way, so the revocation wins; membership is not a field type of the schema.

## Summary of the choices

| Question | This RFC's answer |
| --- | --- |
| How does a space start? | With a genesis by its creator, naming the creator's keys and certificate and the first generation; the space id derives from the creator and a nonce. |
| Which roles exist? | Reader, writer and admin, per space. |
| Who may change membership? | Admins. Every member may leave. |
| Who signs off an erasure? | Another account that holds an admin member in the space; when the performer's account is the only admin account, another account with a writer member; in a space with one account, that account itself, and the erasure stays provisional for 7 days. |
| What do control changes carry? | An entry type, the control dependencies and the fields of their type, readable by the relay; no clock. |
| What may the fold read? | Control bodies and headers only. Wrong keys and failed decryption are local reports. |
| How does the membership state converge? | One order computed from the set: causal order, revocations first, edges added by seniority, then the rules applied in that order. |
| Concurrent grant and revocation? | The revocation wins; the grant is lost and visible. |
| Two rotations at once? | Two generations with derived, distinct ids; the next admin that sees exposure rotates again. |
| A removed member that keeps writing? | Its changes above the cutoff are rejected by every member and refused by the relay; equivocations inside the cutoff are reported, not rejected. |
| A member raised again? | Only its changes under a fresh generation count. |
| When is a person key destroyed? | As soon as a member or the relay accepts the shred: its verdict reads only its causal past, so no revocation undoes it. A provisional shred's key is destroyed once an erasure decision finalizes it, which the relay accepts no sooner than 7 days after the shred. |
| What happens after a lost device? | A member carries the account's newer account key statement into the space; the fold then refuses adds and sign-offs under the superseded account key. |
| How does the relay apply the rules? | The same code over the control changes it holds; where it holds less, the members decide. |
| Groups? | None: withdrawn by the owner (Carsten, 2026-11-09); a later RFC may bring them back. |

## History

- 2026-11-10 · First published (RFC 0001 to RFC 0010 rewritten as fresh texts, NOC-165)
