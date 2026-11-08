# RFC 0008: Membership, roles and the first change of a space

REQUEST FOR COMMENTS · DRAFT 0.2 · NOVEMBER 2026 · ticket NOC-23

This RFC defines who belongs to a space and what each member may do. It covers the first change of a space, the roles and their rights, and the bodies of the control changes. It also defines the rules by which every member and the relay fold those control changes into one membership state, the same on every copy, in any order of arrival.

It builds on [RFC 0001](0001-nocentraldb.md), above all [the design in one page](0001-nocentraldb.md#the-design-in-one-page) and [what it is not](0001-nocentraldb.md#what-it-is-not). It also builds on [RFC 0002](0002-change-and-log-format.md), which reserves control changes as [kind 1](0002-change-and-log-format.md#key-generation-suite-and-kind) and leaves "membership changes" to a later RFC, on RFC 0002's [Amendment 1](0002-change-and-log-format.md#amendment-1-atomic-unit-and-rejected-changes), with its log state *rejected* (D11) and its rule for changes the relay refuses (D12), both decided by the owner, on [RFC 0003](0003-keys-and-cryptography.md), which names the control changes and their purpose, on [RFC 0004](0004-merge-rules-basic-types.md), whose style it follows, and on the principles in [GENESIS.md](../../factory/GENESIS.md).

It uses several drafts that are still open pull requests. It names them without links, because they are not on `main` yet:

- RFC 0005 (schema, NOC-11, PR #35), decisions D13 and D14.
- RFC 0006 (the local view, NOC-12, PR #36), which keeps a per-space control state that this RFC defines.
- RFC 0007 (the relay, NOC-13, PR #37), which applies the membership rules with the data it can see.

**Status.** A draft for the owner's decision. Membership rules decide how permissions behave, which GENESIS.md asks us to tell developers plainly. The control-change bodies are part of the wire format. The owner decides both. Nothing here is built. Draft 0.2 reworks draft 0.1 after pm's review. The fold now uses only control bodies and headers. Seniority and the order are fully specified. Re-grants cannot bring back writes made without the right. Irreversible effects wait until a change is stable. The open decisions are in [Decisions for the owner](#decisions-for-the-owner), each with options and a recommendation. Until the owner decides, the design below describes the recommended option of each decision.

## What this RFC does not cover

- **Work orders**, beyond who may perform an erasure and sign it off. A later RFC defines how a work order is claimed, performed and validated.
- **Cryptography and the formats of signed objects.** The primitives, the key envelope's HPKE construction, the person-key commitment, the member certificate, the account sign-off and the key-chain subkey are RFC 0003's (its section "Formats of what keys sign and derive", decision 15). This RFC places them in the control bodies (see [What this RFC uses from RFC 0003](#what-this-rfc-uses-from-rfc-0003)).
- **Revoking a member certificate.** Deferred to a later RFC about accounts, as RFC 0003 says. In v1, removing the member from every space it belongs to is the revocation.
- **Invitations and public API names.** This RFC does not cover how a new member's public identity reaches the adding member, or what the SDK calls are named. The role names below are working names (decision M12).
- **Groups as members**, which this RFC names and defers (see [Groups](#groups)).
- **Deleting a synced space**, at the relay and on every member, such as an ad-hoc space that synced (RFC 0003). Deferred to a later RFC: it concerns the relay's storage and each member's copy, and needs no control body here. A local ad-hoc space that never synced is deleted locally.
- **Compaction.** A later RFC. This RFC says what compaction must keep: every control change (decision M16).
- **Code.** Nothing in `crates/` or `packages/` changes.

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
| Stable | A control change is stable on a holder when no change that can still arrive can reject it (see [Effects that wait until a change is stable](#effects-that-wait-until-a-change-is-stable)). |

## The design in one page

- **A space starts with a genesis.** The creator writes it. It names the creator's public keys, its certificate and the first key generation. The space id is derived from the creator's member id and a random nonce, so nobody else can write a genesis for that space (decision M7, which reopens RFC 0002's D3).
- **Three roles.** A reader holds the keys and reads everything. A writer also writes content and person keys and performs erasures. An admin also changes membership, rotates the space key and signs off erasures. Rights apply to the whole space (RFC 0001).
- **The fold reads only control bodies and headers.** Whether a change is accepted depends only on the control changes and on the headers of the changes. It never depends on decrypted content, or on a check only some members can make. The relay can therefore run the same code.
- **A revocation names a cutoff.** Removing a member, or lowering its role, names the highest author sequence of that member the revoking admin held. The member's changes above it lose the revoked right on every member and on the relay.
- **Revocations win.** A revocation wins over a concurrent grant. A removed or demoted member regains a right only through a grant that saw the revocation. That grant also starts a new key generation, so writes made in between never count.
- **One order, computed from the set.** Every member puts its control changes into one order computed from the changes alone, and applies them in that order. The order of arrival never matters. Seniority settles cycles of concurrent revocations.
- **Concurrent rotations merge.** A generation id is derived from its rotation, so two rotations never share one. When a member that is no longer in the space may hold the current generation, the next admin that notices rotates again.
- **Irreversible effects wait.** Destroying a person key, its "deleted" acknowledgement and the relay's deletion of its record wait until the shred entry is stable. The view treats the person's fields as erased at once.
- **The relay runs the same code** over the control changes it holds. Where it holds fewer changes than a member, the members decide (see [How the relay applies the rules](#how-the-relay-applies-the-rules)).

## The first change of a space

### Who creates it

Any member creates a space by writing its genesis, on its own device, offline if it likes. The creator becomes the space's first member and its first admin. A relay accepts the genesis only from a creator its operator allows (RFC 0007, decision R6). Members do not check that, because it is a rule of the relay, not of the space.

### What it names

The genesis is a control change whose body (see [Genesis](#genesis)) names:

- **The space id, through a nonce.** The space id is the first 16 bytes of `H("nocentraldb/space-id" || creator member id || nonce)`, where the nonce is 16 random bytes in the body and H is SHA-256 (RFC 0003). It is 128 bits that nobody can predict, and it holds no time. RFC 0002's D3 decided that space ids are drawn like object ids, at random. This RFC asks the owner to revisit that decision (M7), because a random space id does not bind the space to its creator.
- **The creator**: its signing key, its agreement key and its member certificate (RFC 0003). Its member id is the header's author member id (RFC 0002, header key 2), which every reader recomputes from the two keys.
- **The first key generation**, by its generation id (see [Generation ids](#generation-ids)). The genesis carries no envelope. The creator writes a key envelope for itself as its second change, so that the relay's `keys` request finds it like any member's.
- **The suite**, in the header (RFC 0002, header key 8). The genesis's suite is the space's suite. v1 has one suite.
- **The model id, if the owner decides so** (RFC 0005, D13). The genesis reserves a field for it. This RFC recommends leaving the field empty and putting the model id into a sealed change, so that the relay cannot see it (decision M8). D13 stays the owner's decision; this RFC only offers the place.

The genesis does **not** name the space's name or its description. Those are content, and the relay must not read them (principle 1).

### How the relay and members recognise it

A control change is a genesis when its entry type is genesis. It is valid when all of these hold:

- its author sequence is 1, and it has no control dependencies;
- the space id in the header equals the space id derived from the author member id and the nonce;
- its signature verifies with the signing key in its body, which hashes, together with the agreement key, to the author member id;
- the certificate verifies against the account key it names (RFC 0003).

Only the creator can write a valid genesis for its space id, because anyone else's member id derives another space id. The creator can write two, with the same nonce: an equivocation (RFC 0002). RFC 0002 keeps and folds both changes of an equivocation. A space cannot have two first changes, so this RFC narrows that rule for the genesis (see [Conflicts](#conflicts-with-genesismd-and-the-other-rfcs)). Both stay in the log. The one with the lower change id, compared as bytes, is the genesis, and the other is kept with the reason "equivocated genesis" and folds nothing. A member that holds only one takes that one, and switches when it receives one with a lower id. Every member that holds both picks the same one.

The relay creates a space's log when it accepts a valid genesis for a space id it does not hold, and answers `space_exists` otherwise (RFC 0007). Every other control change has the genesis in its causal past. A control change without control dependencies that is not a genesis is malformed.

## Roles and rights

Rights are per space. Every member of a space holds its keys and a full copy (RFC 0001), so no member can be kept from reading part of a space. A member holds exactly one role:

| Right | Reader | Writer | Admin |
| --- | --- | --- | --- |
| Receive key envelopes and read the whole space | yes | yes | yes |
| Acknowledge a shred entry | yes | yes | yes |
| Leave the space, or lower its own role | yes | yes | yes |
| Write sealed changes: content and work orders | | yes | yes |
| Write person-key entries | | yes | yes |
| Perform an erasure: write its shred entry | | yes | yes |
| Add members; raise or lower another member's role; remove members | | | yes |
| Rotate the space key: write key chain entries and key envelopes | | | yes |
| Sign off an erasure, through its account | | | yes |

- **Who may write what is checked per change**, by members and the relay alike. Each control change type needs one right (see [The control bodies](#the-control-bodies)). A sealed change needs the right to write sealed changes.
- **The sign-off of an erasure** comes from an account, not a member (RFC 0003). The account must hold an admin member in the space, in the state of the shred entry's causal past. It must not be the account of the member that performs the erasure. This narrows RFC 0003, which asks only for "an account that has a certified member in the space" (decision M9).
- **A reader cannot write content**, but it can read everything, and its device can leak what it read. The reader role limits writes, not knowledge (principle 4). Typical readers are adapters (RFC 0007), dashboards and auditors.
- **Agents hold roles like every member.** The SDK adds an agent as a writer unless the app asks for another role (decision M14). A sub-agent in an ad-hoc space (RFC 0003) is a writer there, and its parent is the admin.
- **A space can lose its last admin**, when the last admin leaves or admins remove each other at the same time. The SDK refuses to let the last admin leave, but it cannot prevent the concurrent case. A space without an admin keeps working for content, but nobody can change its membership or rotate its key (decision M13).

What the roles do **not** do:

- They give no rights inside the space's data. Every writer may change every object of the space. Finer rights come from more spaces (RFC 0001, "What it is not").
- A revocation does not take back what a member already read (RFC 0003, "What rotation does not do").
- A role is checked against the changes a member holds. A member that has not seen a revocation yet treats the revoked member as it was, until the revocation reaches it.

## The control bodies

Every control change carries its body as RFC 0002's payload (header key 6). The body is a CBOR map in RFC 0002's [canonical profile](0002-change-and-log-format.md#the-cbor-profile), not encrypted. Its first keys follow the layout of RFC 0002's payload map where they share a meaning:

```text
control body (CBOR map; the payload of a kind 1 change)
├── 1 entry type          uint, see the table below (RFC 0002: "key 1 names the entry type")
├── 2 clock               reserved; absent (decision M18)
├── 3 control deps        array of change ids, sorted by bytes, no id twice:
│                         the heads of the author's control DAG when it wrote
├── 4 to 7                reserved
└── 8 ...                 the fields of the entry type
```

Key 3 means what payload key 3 means in RFC 0002: the author's heads when it wrote, restricted to control changes. Key 2 is held free for a clock. Control changes carry none, because a clock would show the relay when admins act, offline work included, which RFC 0002 decided against for every change (D6). The audit of a space's membership therefore has an order (see [Folding the membership state](#folding-the-membership-state)) but no time of writing, only the relay's time of acceptance.

| Entry type | Name | Right its author needs |
| --- | --- | --- |
| 0 | Genesis | none: it creates the space |
| 1 | Membership entry | admin; or any member, to leave or to lower its own role |
| 2 | Key envelope | admin |
| 3 | Key chain entry | admin |
| 4 | Person-key entry | writer |
| 5 | Shred entry | writer, plus an admin account's sign-off |
| 6 | Acknowledgement | reader |

Other entry types are reserved. A member that meets one keeps the change, parked, as RFC 0002 does for every [change it does not understand](0002-change-and-log-format.md#versioning-old-clients-keep-what-they-do-not-understand). Keys 8 to 63 of a body are "must understand", keys from 64 up "may ignore", as in RFC 0002's maps.

**The header** of a control change has the same keys as every change (RFC 0002). Its key generation (header key 4) is the generation the author writes sealed changes with when it writes the control change, or the first generation for the genesis. The fold does not use it.

### Genesis

```text
genesis (entry type 0)
├── 8  nonce              16 random bytes; the space id derives from it
├── 9  signing key        32 bytes, the creator's Ed25519 public key
├── 10 agreement key      32 bytes, the creator's X25519 public key
├── 11 certificate        the creator's member certificate (RFC 0003)
├── 12 first generation   uint, the generation id of generation 1
└── 13 model id           text, optional; absent unless the owner decides otherwise (M8)
```

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
- **A re-grant** is an add of a member that was removed, or a set role that gives the right to write to a member that lost it, or to a reader. It must have the revocation in its causal past. Its from generation must be a **fresh generation**: one whose key chain entry the re-grant's author wrote, with no key envelope to the subject for it in the re-grant's causal past. Like every rule of step 1, this reads only the re-grant's causal past. The subject therefore cannot have held that key before the re-grant, unless another admin wrote it an envelope for that generation concurrently, while the subject was not a member in that admin's own view. Step 1 rejects such an envelope, and the exposure rule counts it, but a key once delivered cannot be taken back. Everything it wrote while it lacked the right is sealed under a generation it held before, which never counts under the re-grant (see [Sealed changes](#sealed-changes)). An admin that re-grants rotates first, so a re-grant costs one envelope per member (decision M5).

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

**What the tag does not stop.** As RFC 0003 notes, an admin that wants a collision can search about 2⁴⁸ member keys for one whose tag collides with a rotation it expects, and get that key made admin. Both rotations are then rejected. The attack blocks a rotation and exposes no key, and a malicious admin can block rotations more simply by removing the admins who would rotate. A longer tag is not worth its cost: the header's 64 bits would leave fewer for the ordinal, or the header format would have to change (M6, option C). Generations are related by the **generation graph**: each points to its previous generations, and "descends from" follows those links. The graph holds every key chain entry that is valid when written (see [The function](#the-function), step 1), accepted or later rejected. A rotation that a later revocation rejects still links its generation, so honest writes under it keep counting (decision M17). An entry that was never valid when written, such as one by a member that never held the admin role in its own causal past, does not count. The exposure rule counts every key chain entry and envelope in the set, whatever its verdict. Rejecting both rotations that share an id is decision M23. This keeps RFC 0002's format and gives its number a structure (decision M6).

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

RFC 0005's D14 proposes a person id that spans spaces. The person-key id stays per space, as RFC 0003 recommends (decision 7), and never derives from the person. If D14 is decided, the erasure work order finds the person-key ids of a person through the person id inside sealed data. The relay never sees the person id.

### Shred entry

```text
shred entry (entry type 5)
├── 8  person-key id      16 bytes
└── 9  sign-off           the account sign-off of RFC 0003, over the space id, the
                          person-key id and the performer's member id
```

The performer is the shred entry's author, a writer. The sign-off is valid when its account holds an admin member in the state of the shred's causal past, and that account is not the account of the performer's certificate. The shred entry names no work order: the relay must not learn which sealed change was a work order (RFC 0001). The work order, inside its sealed change, names the shred entry instead.

### Acknowledgement

```text
acknowledgement (entry type 6)
├── 8  shred              change id of the shred entry
└── 9  deleted            bool: false for "seen", true for "deleted my copy"
```

A member writes two acknowledgements per shred. It writes the first, with `deleted` false, as soon as it accepts the shred. That one is a control change that has the shred in its causal past, so an admin's first acknowledgement is what lets the shred become stable. It writes the second, with `deleted` true, after it deleted its copy of the key, which it does only once the shred is stable (see [Effects that wait until a change is stable](#effects-that-wait-until-a-change-is-stable)). The audit's evidence of erasure is the second kind. A further acknowledgement of the same kind, for the same shred by the same member, changes nothing. An acknowledgement by a member whose rights are revoked above the cutoff is rejected like any of its changes. The audit then shows that member as never having confirmed.

### What this RFC uses from RFC 0003

RFC 0003 defines the formats of what keys sign and derive (its section "Formats of what keys sign and derive", decision 15). This RFC uses them as they are:

- **The member certificate** names the account key, the member id, the kind (device, agent or sub-agent) and, for a sub-agent, the parent member id. The account key signs a device's or an agent's certificate. A sub-agent's certificate is signed by its parent's member signing key, under its own context string (`"nocentraldb/sub-agent-cert"`), because a parent agent does not hold the account key and must not. Its key 1 still carries the parent's account key, for attribution. A member that checks a sub-agent's certificate, in an add entry or a genesis, checks the chain: the parent's own certificate, which must be of kind agent or sub-agent and name the same account key. The parent's certificate must be in this space's control changes, in the genesis or add entry that made the parent a member, so the check reads only the control set. In RFC 0003's ad-hoc space the parent is the creator, so it always is. An add entry whose sub-agent's parent is not a member in the state of the add's causal past is not valid when written. For the sign-off below, a sub-agent counts as its parent's account.
- **The account sign-off** names the space id, the person-key id and the performer's member id, with the account key and its signature.
- **The key-chain subkey** is the HKDF-SHA256 subkey of the new generation's key that encrypts the previous keys in a key chain entry.

## Folding the membership state

The control state must be the same on every member that holds the same control changes, whatever order they arrived in (principle 2). The relay must reach the same state over the changes it holds. This section defines that state as a function of the set, as RFC 0004 defines a field's state.

### What the fold may read

The fold reads only what every member and the relay can read: the control bodies and the headers of all changes. It never reads a decrypted payload, and never a fact that only some members can check. Those facts are:

- whether a key envelope or a key chain entry holds the right key;
- whether a person-key record matches its commitment;
- whether a sealed change decrypts.

They are **local checks**. A member that finds a wrong key reports it, with the author whose signature covers it, and the SDK shows it. The control state does not change. RFC 0002's Amendment 1 rejects a sealed change that does not decrypt. Every member that holds the generation's key reaches the same verdict, because AEAD decryption is deterministic. A member that holds no key for it rejects it with the reason "no key" until an envelope arrives (D11).

### The function

Let S be the set of control changes of a space that a holder has, each with a valid signature and a canonical body, and the genesis chosen as above. The control state is `resolve(S)`, in five steps.

**1. Validity in the causal past.** A control change c is *valid when written* if `resolve` of its causal past gives its author the right its entry type needs, and c's own rules hold in that state. The own rules are: an add's subject is not a member, and a re-grant saw the revocation and names a fresh generation; an envelope's recipient is a member and its generation exists; a key chain entry's previous generations exist and its generation id matches; a person-key entry repeats the commitment of its id; a shred names a person key that exists and carries a valid sign-off. The causal past is a smaller set, so the definition is well-founded. A control dependency the holder does not hold is a gap. The SDK reports it and asks for it (RFC 0002), and the causal past then counts only what the holder holds.

**2. Seniority.** A total order on member ids:

- the creator first;
- then every member that holds a grant of admin in S that is valid when written, by the smallest pair (depth, change id) over those grants. A grant's **depth** is one more than the smallest depth among the admin grants its author holds in the state of the grant's causal past; the creator has depth 0. Depth follows only the chain of admin grants, so a missing dependency elsewhere in the control DAG cannot change it. Change ids compare as bytes;
- then every other member id, by its bytes.

A grant is one deeper than the grant its author's right rested on. So an admin ranks before every admin whose only admin grants it wrote itself. A member that another admin made admin at a smaller depth ranks by that grant instead.

**3. Revocation edges.** A revocation R (a set role or a remove; a remove sets "none") *excludes* a change y of its subject when all of these hold:

- y needs a right above the role R sets;
- y's author sequence is above R's cutoff;
- y is not covered by a re-grant: y's causal past holds no grant of that right to its author that saw R. A grant of a right the author held before R does not count, because R caps it.

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

An accepted change updates the state. A rejected change stays in the log in RFC 0002's state *rejected* (D11) with its reason, and changes nothing. Two rules apply to the whole set rather than to the order:

- A person-key entry that a shred of the same person-key id did not see is rejected when S holds an accepted shred for that id, wherever the entry falls in the order. A shred therefore wins over a concurrent re-wrap.
- Of two accepted shreds of one person-key id, the first in the order takes effect, and the second changes nothing.

**Roles.** A membership entry that sets role r is a grant of r, and caps at r every grant of the same subject that did not see it. An add or set role sets its role; a remove sets "none". A member's role is the highest role among its accepted grants, each lowered to the lowest cap that an accepted entry not seen by that grant puts on it. A grant therefore loses to a concurrent removal or demotion, and a raise to a concurrent lower raise. It keeps the rights it gives over every entry it saw, so a re-grant that saw the revocation stands. Every cap is a revocation in step 3, with the capping entry's cutoff. The changes a member made under the higher role above that cutoff are therefore excluded wherever they land in the order, and the rule for sealed changes applies the same caps.

**The state** holds, per member id: its public keys and certificate, its role, its accepted grants with their from generations, and its accepted revocations with their cutoffs. Per generation: its id, its previous generations, its envelopes, and which member ids may hold it (see [Two removals that rotate the key at once](#two-removals-that-rotate-the-key-at-once)). Per person-key id: its commitment, its newest record's generation and hash, and whether a shred destroyed it, with its acknowledgements. The state has a canonical encoding: equal states encode to equal bytes, which is what the convergence harness compares (RFC 0004, C1).

### Sealed changes

A sealed change is judged by its header against the final state. The order plays no part. A sealed change by author X with author sequence s, under generation g, is accepted by the membership rule when X holds an accepted grant G of a role that writes such that:

- g is G's from generation, or descends from it in the generation graph; and
- for every accepted revocation R of X's right to write that G did not see, s is at most R's cutoff.

Otherwise the change is rejected with the reason "not a member" (D11). It is checked again whenever a control change of the space arrives (D11).

This **membership verdict** needs nothing but headers and control changes. It never waits for a sealed change that has not arrived, it never depends on which sealed changes a member holds, and every holder of the same control changes reaches the same one, the relay included. Decryption comes after it and is not part of it. RFC 0002 Amendment 1, step 5, rejects a change with "no key" or "undecryptable", and that verdict differs from member to member: a member without the generation's key rejects with "no key" what a member with the key folds. Those rejections are local. They are checked again under D11 when an envelope arrives, and the convergence promise for them is RFC 0002's: every member that holds the key reaches the same verdict.

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
- a change arrives that moves which of two shreds of one person-key id comes first in the order;
- an equivocated genesis with a lower change id arrives late, and becomes the genesis (see [How the relay and members recognise it](#how-the-relay-and-members-recognise-it));
- a key chain entry arrives that names the same generation id as an accepted one, and both are rejected (decision M23).

Every such change has to be concurrent with the change whose verdict flips, or with one of the changes it rests on. A change that the authors of all possible revocations have seen can no longer flip. [Effects that wait until a change is stable](#effects-that-wait-until-a-change-is-stable) uses this. Members see verdicts change while concurrent control changes are still arriving, and the SDK reports each change (decision M4).

### The cutoff

A revocation's cutoff is the subject's highest author sequence that the revoking member held when it wrote the revocation, counting sealed and control changes alike. The subject's changes at or below it are **inside**; those above it are **excluded** and lose the rights the revocation took away. A writer demoted to reader keeps acknowledging shreds; a removed member keeps no right.

The cutoff is a number in a control body, and the author sequence is a number in every header. Members and the relay therefore compute the same verdict, without decrypting anything. The rule is safe against a member that lies about its control dependencies: it cannot change what the revoking member held. A change written after the revocation carries a higher sequence and is excluded.

**What the cutoff does not stop.** A revoked member can write a new change under a sequence number at or below its cutoff that it already used. That is an equivocation (RFC 0002). Every member detects it once it holds both changes, and the SDK reports it, but the second change is inside the cutoff and is accepted, as RFC 0002 accepts both changes of every equivocation. The revoked member holds only generations up to the revocation's rotation, so such a change is sealed under an old generation. The SDK marks it as written by a revoked member under an old generation (decision M3).

### Effects that wait until a change is stable

The view can always be recomputed, so un-folding a change costs nothing but work. Some effects of control changes cannot be undone. They wait until the change can no longer be rejected:

| Control change | Effect at once, reversible | Effect that waits until it is stable |
| --- | --- | --- |
| Shred entry | The fold treats the person's fields as erased on every member (RFC 0003, step 4). The member writes its "seen" acknowledgement. | The member deletes its copy of the person key and then writes its "deleted" acknowledgement. The relay deletes the person-key record. |
| Key envelope | The member unwraps the key into its keystore and can decrypt. | Nothing irreversible: if the envelope is rejected later, the member keeps the key only to read changes already accepted under it, and never writes with it (decision M17). |
| Membership entry, key chain entry, person-key entry, acknowledgement | They change the control state; the relay changes whom it serves. | Nothing irreversible. |

**When a change is stable.** A holder computes its stable changes as the smallest set closed under this rule, starting from none. An accepted control change c is stable when, for every member id that holds the admin role in its state, or held it and lost it to an accepted revocation, the holder holds every change of that member from author sequence 1 up to one of:

- a control change of that member whose causal past contains c;
- the cutoff of a revocation that took the role from that member and is itself stable.

A revocation that is not stable yet bounds nothing: until it is, its subject counts as an admin whose run must reach a change that saw c. Applying the rule again until nothing is added gives one answer. The rule only adds, so this ends.

**Why nothing can reject it then.** Only a revocation can reject an accepted change. It does so by excluding the change, or a grant that the author's right rests on, which is in c's causal past. Only an admin can write such a revocation. Every admin that saw c holds c's author's changes up to c, so its cutoff is at least c's sequence, and c is inside. Every revocation an admin wrote before it saw c, or before it lost the role, is among the changes the holder holds. Admins made later were made by an admin that had already seen c, or by one of those held changes. Their grants are in their own revocations' causal past, so c is too. A revocation that bounds a former admin's run is stable itself, so it cannot be undone and give that member's later changes the role back. Step 1 is fixed for a given causal past. Seniority can still change, but it only decides between revocations that exclude each other, and none can exclude c. The harness checks this argument (see [How the convergence harness checks it](#how-the-convergence-harness-checks-it)).

**What this costs, and the way forward.** A shred becomes stable only when every admin has written a control change after seeing it. One admin device that is lost or stays offline therefore blocks the destruction of the key, the "deleted" acknowledgements and the relay's deletion indefinitely. The person's fields are erased in every view at once, but the key still exists on the members, which the audit shows. The way forward:

- **An alarm.** When a shred has waited longer than a set period, the SDK of every admin raises an alarm. It names the admin devices whose runs block it, and the shred's age.
- **Demoting or removing the blocking admins.** An admin lowers each blocking device from admin to writer, or removes it. The demotion's cutoff ends the run the holder needs, and once the demotion is stable, the shred can become stable. A demotion to writer needs no rotation, because the device keeps the right to read and write.
- **An admin's first acknowledgement counts.** Every admin device that is online writes a "seen" acknowledgement as soon as it accepts the shred (see [Acknowledgement](#acknowledgement)). An admin that is online but otherwise idle therefore never blocks; only one that does not come online does.

How long the period is, and whether the SDK may demote an idle admin device on its own after a longer one, is decision M21. Until the shred is stable, the erasure has not fully happened. That conflicts with what principle 3's team lead expects, and the RFC names it (see [Conflicts](#conflicts-with-genesismd-and-the-other-rfcs)).

### Concurrent add and remove of one member

Alice removes Mallory's laptop, which was stolen. At the same time, on another island, Bob, who has not seen the removal, raises the laptop's role from writer to admin. Both entries are valid when written.

Bob's entry is a grant that did not see the removal, so Alice's removal caps it at "none". On every copy the laptop is removed. Every change it made above Alice's cutoff is rejected, whether it reached a member before or after the removal. Bob's entry stays accepted but without effect, and the SDK shows it as a **lost grant**, the way RFC 0004 shows a lost removal.

This is the opposite of RFC 0004's sets, where an addition wins over a concurrent removal. For data, a lost addition is data that is gone. For membership, a lost removal leaves a stolen device writing. So the removal wins, and Bob can repeat the grant once he has seen the removal and still wants it (decision M2). Concurrent role changes follow the same rule: when two admins set a member's role at the same time, the member ends with the lower role. Each set role carries a cutoff, so whatever the member did under the higher role above that cutoff is excluded on every copy, wherever it lands in the order.

### Two removals that rotate the key at once

Alice removes Xavier and rotates the key. At the same time, on another island, Bob removes Yvonne and rotates. Alice's new generation has an envelope for Yvonne, and Bob's has one for Xavier. When the islands meet, every member holds both removals and both key chain entries:

1. Both removals are accepted; neither excludes the other. Xavier and Yvonne are removed on every copy.
2. Both generations exist. Their ordinals are equal and their ids differ, because their authors differ.
3. The **current generation**, which members write sealed changes with, is a head of the generation graph: a generation that no accepted key chain entry lists as previous. Among the heads, it is the one with the highest ordinal, then the one whose key chain entry comes first in the order. Every member picks the same one. A member that holds no envelope for it writes with the newest generation it holds, and the SDK reports that it waits for an envelope.
4. **Who may hold a generation**: the author of its key chain entry, every recipient of any envelope for it, and everyone who may hold a generation that descends from it. Every envelope in S counts, accepted or rejected, because a rejected envelope may still have been delivered. A generation is **exposed** when anyone who may hold it is not a member of the state.
5. When the current generation is exposed, an admin writes a **merge rotation**: a key chain entry that lists every head as previous, with envelopes for the current members only. The SDK of any admin that sees an exposed current generation does so after a random delay of up to a minute, unless it sees another admin's merge rotation first (decision M15).
6. Two merge rotations written at the same time are concurrent rotations again, and step 5 repeats. Each round needs two admins to rotate within the same minute without seeing each other.

**What this does not do.** Until the merge rotation lands, members write under an exposed generation. Xavier or Yvonne could read those changes if they obtained them. The SDK reports the exposure until it ends. If no admin is online, it lasts until one is. Envelopes that a holder never received cannot count: a removed admin's envelope that never left its island makes a generation exposed only where it arrives. The same rule covers a member that left (it cannot rotate for itself) and a removal whose author never rotated, for example because its device crashed between the two changes.

### Two admins who remove each other

Alice removes Bob, and at the same time Bob, or whoever holds Bob's device, removes Alice. Each removal excludes the other, because each is above the other's cutoff. Step 3 adds the senior author's edges first. If Alice became admin first, her removal comes before Bob's. Then Bob is removed, and Bob's removal of Alice is excluded and rejected. Every member computes the same outcome.

In a ring of three, where A removes B, B removes C and C removes A at the same time, with A the most senior: A's and B's edges are added, and C's edge, which would close the cycle, is skipped. In the order, A's removal is accepted, B's is excluded, and C's is accepted, because nobody whose revocation is accepted revoked C. A and B are removed, C stays, and A's removal of B stands, because it comes before C's removal in the order.

Seniority is not a judgement of who is right. It is a fixed way to keep one side of a cycle, so that a compromised admin cannot cancel its own removal by removing the remover at the same moment (decision M4). It protects only against a junior attacker. A compromised senior admin wins the cycle. Another admin, or the creator, must then remove it again once the honest admin reports it.

### A member removed while offline who keeps writing

Carol's laptop is offline for a week. Meanwhile Alice removes it, naming as cutoff the laptop's highest sequence she held, 40. The laptop keeps writing, sequences 41 to 60, under the old generation, the only one it holds.

- **On the relay.** When the laptop reconnects, it pulls first (RFC 0007, island mode), sees its removal, and the SDK reports it. If it pushes anyway, the relay refuses sequences 41 to 60: they are above the cutoff.
- **On members that received them directly**, on the laptop's island: when Alice's removal reaches them, the fold rejects sequences 41 to 60 and un-folds what they had applied (RFC 0006). Their state then equals that of every member that holds the same changes.
- **On the laptop itself.** It folds its own removal like any member, and its own changes above the cutoff become rejected in its log. The SDK tells the app that no member accepts the changes made since the cutoff. The laptop keeps its copy of everything it received before.

Changes the laptop wrote before Alice's removal but above sequence 40 are rejected too, because Alice had not received them. Neither the relay nor the members can tell an honest late change from a backdated one: the clock is encrypted, and the sequence number is the only order they can check. The SDK warns an admin who removes a member whose recent changes it has not received yet (decision M3).

Changes at or below the cutoff that arrive late are accepted everywhere, by the relay too, unlike RFC 0007's decision R7 A.

### A member demoted and raised again

Alice lowers Dan from writer to reader, with cutoff 70. Dan, offline, keeps writing as if he were a writer, under generation 7, which he holds. Later Alice raises Dan to writer again.

The raise is a re-grant. Alice writes a key chain entry for a fresh generation 8 first, then the raise, with generation 8 as its from generation, then the envelopes, Dan's after the raise. Dan's sealed changes above 70 under generation 7 never count: generation 7 does not descend from generation 8, and the demotion caps every grant Dan held before. His changes under generation 8 and its descendants count from the raise on. He could not have written under generation 8 before, because he had no key for it. If another admin rotated to a concurrent generation 8′ while the raise was on its way, Dan's changes under 8′ do not count either, until a merge rotation gives him a descendant of both. That loses honest writes in a rare case, and the SDK reports them as rejected.

### What this does not do

- **A member that has not seen a revocation** treats the revoked member as it was. It accepts the member's changes until the revocation arrives, and then rejects those above the cutoff (RFC 0006, un-folding). The app sees those changes appear and disappear.
- **A revoked member keeps its copy.** Revocation and rotation keep it from writing, and from reading what is written under later generations. It still holds what it received (RFC 0003).
- **A revoked member's equivocations inside its cutoff are accepted**, as RFC 0002 accepts every equivocation. The SDK reports them (see [The cutoff](#the-cutoff)).
- **Grants a revoked admin made inside its cutoff stay.** If a stolen device added a member before the removing admin had received that add, the added member stays until an admin removes it too. The SDK lists the members a removed admin added, so that the remover can review them.
- **Verdicts are not final until a change is stable.** Concurrent control changes can still flip them, as [What a new change can change](#what-a-new-change-can-change) lists.
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
| M-C8 | A stable change stays accepted. | Whenever a holder calls a change stable, the harness adds further generated changes. These include concurrent revocations by former admins, revocations of the revocations that bounded them, and missing dependencies of non-admins. The change must stay accepted in every extension. A broken fixture that bounds a former admin by a revocation that is not stable must fail. |
| M-C9 | Exposure counts every envelope. | Rejected rotations and envelopes from removed admins, delivered to some copies only. Once a merge rotation is accepted, no one outside the state may hold the current generation in any copy that holds those envelopes. |
| M-C10 | Shreds are ordered. | Two shreds of one id, and shreds concurrent with re-wraps. Exactly one shred takes effect, and no re-wrap it did not see is accepted. |
| M-C11 | The relay agrees. | After every delivery, the relay simulator's membership table equals `resolve` of its own log. Every change it refused is one that the members also reject for the same S, or one listed in [How the relay applies the rules](#how-the-relay-applies-the-rules). |
| M-C12 | The checks fire. | Broken fixtures the harness must reject: add wins for membership; arrival order instead of the computed order; a revocation without its cutoff; seniority by arrival; a re-grant without a fresh generation; exposure that counts only accepted envelopes; a shred whose key is destroyed before it is stable. |

## How the relay applies the rules

The relay holds no key. It does hold every accepted control change of a space, and their bodies, cutoffs and control dependencies are readable (RFC 0002, kind 1). It therefore runs the same `resolve`, from the same Rust code that the members run, over the control changes in its log. Its membership table is `resolve` of its log, recomputed with each append of a control change. That table holds roles, grants with their from generations, revocations with their cutoffs, and the generation graph. RFC 0007's backend stores only positions of addition and removal, so its table has to grow (see [Conflicts](#conflicts-with-genesismd-and-the-other-rfcs)).

**On a push**, the relay checks each change against its current state, in addition to RFC 0007's checks:

- **A control change** is accepted when `resolve` over the relay's log plus that change accepts it. Dependencies the relay does not hold count as gaps, as on a member.
- **A sealed change** is accepted by the rule in [Sealed changes](#sealed-changes), which needs only its header: author, author sequence and generation id.
- **The uploader** must be a current member (RFC 0007).
- **A person-key record** needs its person-key entry (RFC 0007, R12). It replaces the stored record when its generation is the stored record's generation or descends from it. RFC 0007's "same or newer generation" no longer works with generation ids that carry a tag.
- **A shred's record deletion** happens when the shred becomes stable on the relay, by the same rule as on a member, not in the transaction that appends it (decision M20).

Where the relay and the members can disagree, and who decides:

| Case | Relay | Members | What follows |
| --- | --- | --- | --- |
| A revocation arrives after the relay accepted a change it excludes. | Has accepted it, and never removes an entry from its log. | Reject it when they hold the revocation. | The relay's table follows the revocation; the stale entry stays as bytes. The members decide. |
| The relay refused a change because of a revocation that a concurrent change later makes ineffective. | Refused it at the time. | Accept it once they hold both. | Under D12 A, the member never pushes it to that relay again, so it reaches others only directly. This RFC asks the owner to let a member push it again once its own fold accepts it (decision M19). |
| Changes that never reach the relay, for example exchanged only on an island. | Does not know them. | Fold them. | Members that hold different sets can differ, as RFC 0002 states. |
| A key or record is wrong, or a sealed change does not decrypt. | Cannot tell. | Report it, or reject the sealed change (D11); the signature names the author. | Local checks; the control state does not change. |
| A space is created by a creator the relay operator does not allow. | Refuses the genesis (RFC 0007, R6). | Do not check it. | The space works in island mode and on relays that allow its creator. |

The relay decides nothing the members cannot check, but it can refuse a change the members later accept, and without decision M19 that refusal stands.

**What the relay learns from this RFC**, in addition to the lists in RFC 0002 and RFC 0007 (decision M11):

- **Roles.** Each member's role in each space, and when it changes.
- **Cutoffs.** For each revocation, how many of the subject's changes the revoking admin had received.
- **Who saw what.** Through the control dependencies: which control changes each admin had seen when it wrote, so how up to date each admin was, and which admins acted on separate islands.
- **Accounts across spaces.** The account key in each member certificate, so which devices and agents belong to one person or organisation, in every space the relay serves.
- **The sub-agent tree.** Which member is a sub-agent of which parent, from the certificates.
- **Erasure approvers.** Which accounts sign off erasures, and in which spaces.
- **The generation graph.** When concurrent rotations happened, and which merge rotations followed.

It still learns no content, no space name and, under decision M8, no model id.

## Groups

GENESIS.md says that "groups can be members of many spaces, so rights change without moving data". A group would be one principal, such as "the design team", that is a member of many spaces. Adding a person to the group would give them every one of those spaces at once.

This RFC **defers groups** to a later RFC, for three reasons:

- **A group's membership lives outside the space.** If a space's control state depended on a group's membership, which changes in another log, then folding one space would depend on another space's changes. RFC 0002 keeps every change, and every fold, inside one space. Crossing spaces needs its own convergence argument.
- **Removing someone from a group rotates every space of the group.** With per-member envelopes (RFC 0003, decision 1), that is one rotation per space and one envelope per remaining member in each. A group key would reduce the envelopes, but it adds a second layer of keys and rotations to design.
- **The relay would have to check membership through the group**, which needs the group's control state on every relay that holds any of its spaces.

Until then, the SDK offers a group as a list it expands. Adding a group to a space writes one add entry per member of the group. Removing a person from a group writes one removal in each space the SDK added them to through it. Rights still change without moving data. What v1 loses is a single entry that changes many spaces at once, and a group that exists in the control state (decision M10). This partly conflicts with GENESIS.md (see [Conflicts](#conflicts-with-genesismd-and-the-other-rfcs)).

## Rejected options

| Option | Why not |
| --- | --- |
| Add wins for membership, as RFC 0004's sets | A concurrent grant would undo the removal of a stolen device. |
| The order of arrival, or the relay's order, decides | Differs between members and islands, so copies diverge (principle 2). |
| The relay's membership table as the truth | Members could not check it, and island mode would stop every membership change. |
| A cutoff checked against the dependencies inside sealed changes (draft 0.1) | Only members that decrypt and hold those changes can check it, so members and the relay would reach different verdicts. |
| Refusing every change of a removed member from its removal on (RFC 0007, R7 A) | Depends on the relay's order, and refuses changes the remover had already received. |
| Revoking by time | The clock is encrypted, and a removed member can write any clock. |
| A re-grant without a fresh generation | A demoted or removed member's writes in between would count again, also ones it pre-dated with high sequence numbers. |
| Never re-adding a member id | A member key serves every space of a device, so a removal by mistake would force a new device key everywhere. |
| A total order of membership changes, as MLS commits | Needs one party to order them, which island mode cannot have (RFC 0003). |
| A quorum of admins for each membership change | Needs several admins online; a small team's space would stop when one is on holiday. |
| Both of two mutual removals take effect | A stolen admin device takes an honest admin down with it, and can leave the space without an admin. |
| Neither of two mutual removals takes effect | A compromised admin cancels its own removal by removing the remover at the same moment. |
| Seniority from each revocation's own causal past (draft 0.1) | Two revocations could rank their authors differently, so the order would not be total. |
| A random tag in the generation id (draft 0.1) | An admin could pick a tag that collides with another rotation on purpose, with one try. |
| Rejecting every generation id that two rotations share, without deriving it | Changes written on the losing island would become undecryptable, and a malicious admin could cause it. |
| Fold rules that check wrapped keys or decryption | Only some members can check them, so the control state would split. |
| Destroying a person key as soon as the shred is accepted | A concurrent revocation can still reject the shred, and the key could not come back. |
| A random space id, and the first genesis wins | Anyone could write a competing genesis with a lower change id and take over the space on members that hold both. |
| The model id in plain text in the genesis, as the default | The relay would learn which app each space belongs to; RFC 0002 lists the model as something it cannot see. |
| Free capability lists per member instead of three roles | More for every member and the relay to check and to get wrong; three roles cover the use cases, and more roles can come later. |
| Rights per collection or per object | Needs a reader that enforces them on content, which the relay cannot be (RFC 0001). More spaces give the same effect. |
| Hiding the control bodies and dependencies from the relay | The relay could not apply the rules, so a removed member could keep filling the space's log. |

## Decisions for the owner

GENESIS.md reserves the wire format, how permissions behave and public API names for the owner. Each decision below names the options and a recommendation.

**M1. Roles.**

- A. Reader, writer and admin, per space. *(recommended)*
- B. Writer and admin only.
- C. A set of capabilities per member.

Recommendation: A. Readers cover adapters, dashboards and auditors, which should not write. C adds a check per capability on every member and the relay, for cases nobody has asked for yet.

**M2. A grant concurrent with a revocation of the same member.**

- A. The revocation wins; the grant stays visible as lost. *(recommended)*
- B. The grant wins, as an addition does in RFC 0004's sets.

Recommendation: A. A lost grant is cheap to repeat; a lost removal leaves a stolen device writing.

**M3. Which changes of a revoked member count.**

- A. Those at or below the cutoff, an author sequence the revocation names. A revoked member's equivocations at or below it are accepted, as RFC 0002 accepts every equivocation, and reported. *(recommended)*
- B. As A, and the revocation also carries a hash over the subject's change ids up to the cutoff, so that members can reject such equivocations once they hold the subject's history.
- C. Those the relay accepted before the revocation (RFC 0007, R7 A).

Recommendation: A. It needs only headers, so members and the relay reach one verdict. B closes the equivocation gap, but a member can check the hash only when it holds every one of the subject's changes up to the cutoff, which splits verdicts between members that do and members that do not. C depends on the relay's order, which members on an island do not have.

**M4. Seniority and verdicts that can change.**

- A. Seniority by the depth of the admin grant, as specified; the senior admin's revocation wins a cycle; verdicts can change while concurrent control changes arrive, and the SDK reports each change. *(recommended)*
- B. Both revocations of a cycle take effect.
- C. Neither takes effect.

Recommendation: A. B lets a compromised admin take an honest one down with it. C lets it cancel its own removal. Verdicts that change while concurrent changes arrive are the price of island mode under every option.

**M5. Re-granting a right a member lost.**

- A. Allowed by a grant that saw the revocation and names a fresh generation; the admin rotates first. *(recommended)*
- B. Never; the member needs a new member key.

Recommendation: A. A member key is shared by all spaces of a device, so B costs a new key everywhere for one mistake. A costs one rotation per re-grant, one envelope per member.

**M6. Generation ids.**

- A. A 16-bit ordinal and a 48-bit tag derived from the space, the rotation's author and its previous generations. *(recommended)*
- B. Generation numbers only, and members write with the generation whose key chain entry has the lower hash (RFC 0003's sketch).
- C. A 32-byte generation id in the header, such as the key chain entry's change id.

Recommendation: A. B leaves two keys under one number, so the changes under it are ambiguous. C changes RFC 0002's header format. A keeps the format, and 65,535 rotations per space are enough for v1. Its limit: an admin that searches about 2⁴⁸ member keys can make one rotation collide and be rejected, which blocks that rotation and exposes no key (see [Generation ids](#generation-ids)).

**M7. Revisit RFC 0002's D3 for space ids.**

- A. Space ids derived from the creator's member id and a random nonce. *(recommended)*
- B. Keep D3: 128 random bits, and the genesis with the lower change id wins.

Recommendation: A. The owner decided D3 for object and space ids alike. This RFC asks to reopen it for space ids, because under B anyone who learns a space id can write a competing genesis, and with a lower change id take the space over on every member that holds both. A keeps what D3 chose it for: 128 bits nobody can predict, with no time in them.

**M8. The model id (RFC 0005, D13).**

- A. Not in the genesis; RFC 0005 puts it into a sealed change, as part of D13. *(recommended)*
- B. In plain text in the genesis's reserved field.
- C. In the genesis as `H(space id || model id)`.

Recommendation: A. RFC 0002 promises the relay cannot see the model. B breaks that. C can be reversed by anyone who can guess model ids, which developers choose as names. D13 stays the owner's decision; the genesis keeps the field for B.

**M9. Who signs off an erasure.**

- A. An account with an admin member in the space, other than the performer's account. *(recommended)*
- B. Any account with a member in the space, other than the performer's (RFC 0003).

Recommendation: A. An erasure cannot be undone, and B lets a reader's account approve one.

**M10. Groups.**

- A. Deferred to a later RFC; the SDK expands a group into one entry per member. *(recommended)*
- B. Designed now, as a principal with its own control state.

Recommendation: A, for the reasons in [Groups](#groups). The owner rules on it because GENESIS.md names groups.

**M11. What the relay sees.**

- A. Control bodies and control dependencies are readable, so the relay applies the same rules. The relay then sees roles, cutoffs, who saw what, the account behind each member across spaces, the sub-agent tree, the erasure approvers and the generation graph. *(recommended)*
- B. Control bodies are encrypted; only members apply the rules, and the relay checks only signatures.

Recommendation: A. Under B, anyone with a valid key in the space can fill its log with changes that every member rejects, and the relay could not stop a removed member. The cost of A is the list in [How the relay applies the rules](#how-the-relay-applies-the-rules). The account key is the item that weighs most: it links one person's devices and agents in every space on that relay.

**M12. Public names.** Reader, writer and admin, and the outcome "lost grant", are working names. Recommendation: keep them here, and decide the public names in the SDK's API review, as RFC 0004 does for its names.

**M13. A space without an admin.**

- A. Its membership is frozen; content goes on. *(recommended)*
- B. The most senior writer becomes admin automatically.

Recommendation: A. B makes someone admin who was never chosen for it. The SDK refuses to let the last admin leave, so A arises only from concurrent removals.

**M14. The default role of an agent.**

- A. Writer, unless the app asks for another role. *(recommended)*
- B. Reader, unless the app asks for more.
- C. No default; the app must always choose.

Recommendation: A. Most agents in RFC 0001's examples write. B makes every writing agent a two-step setup. C is the safest, but makes every call longer for the common case. The SDK's notice says the role in plain words in every case.

**M15. Automatic merge rotations.**

- A. The SDK of every admin device writes a merge rotation when it sees an exposed current generation, after a random delay. *(recommended)*
- B. The SDK reports the exposure, and the app asks an admin to rotate.

Recommendation: A. Exposure means a removed member may read new changes, and an admin's app may not be built to handle the report. A costs one rotation per exposure, and rarely two.

**M16. Keeping every control change.**

- A. Compaction keeps every control change of a space forever. *(recommended)*
- B. Compaction may replace old control changes by a signed snapshot of the control state.

Recommendation: A. The state is a function of the whole set, and B needs a snapshot that every member can check, which is a design of its own. The cost: a membership entry is about 300 bytes, an envelope about 150, so a rotation for 50 members is about 8 KB. A space with 100 removals keeps under 1 MB of control changes on every member and on the relay.

**M17. Sealed changes under a generation whose key chain entry is later rejected.**

- A. They stay accepted when their author's grant allows them. Members keep the key of a rejected generation to read, never to write. *(recommended)*
- B. They are rejected with the generation.

Recommendation: A. Under B, honest writes on an island whose admin is revoked later would be lost. The key of a rejected generation may be held by someone outside the space, and the exposure rule already makes the next admin rotate away from it.

**M18. No clock in control bodies.**

- A. Control bodies carry no clock; key 2 stays reserved. *(recommended)*
- B. Control bodies carry the author's hybrid logical clock, as payloads do.

Recommendation: A. B shows the relay when each admin acted, offline work included, which RFC 0002's D6 kept from it for every change. The audit keeps the order of membership changes and the relay's time of acceptance.

**M19. Pushing again a change the relay refused.**

- A. A member pushes a refused change to that relay again, once, whenever its own fold newly accepts the change after a control change arrived. *(recommended)*
- B. Keep RFC 0002's D12 A: never push it to that relay again.

Recommendation: A. Under B, a change that a concurrent revocation made the relay refuse, and that the members later accept, never reaches the members that sync only through the relay. A amends D12 only for that case. It cannot loop, because the member pushes again only after a new control change made its fold accept the change.

**M20. Irreversible effects wait until a change is stable.**

- A. Destroying a person key, its "deleted" acknowledgement and the relay's deletion of its record wait until the shred is stable; the view erases the fields at once. *(recommended)*
- B. They happen as soon as the shred is accepted.

Recommendation: A. Under B, a shred that a concurrent revocation later rejects has already destroyed keys that cannot come back. The cost of A: one offline admin delays the destruction until it returns or is removed, and RFC 0007 must delete records later than in the append's transaction.

**M21. How long a shred may wait, and the escalation.**

- A. The SDK raises an alarm on every admin device when a shred has waited 7 days. It names the blocking admin devices, and an admin demotes or removes them. *(recommended)*
- B. As A, and after 21 days an admin's SDK demotes the blocking admin devices to writer on its own.
- C. Destroy the key and acknowledge at once on acceptance; only the relay's deletion waits.

Recommendation: A. It leaves well inside the one-month deadline of a data-subject request for a person to act, and it never changes a role without a person. B meets the deadline without anyone acting, but it lowers rights automatically, which the owner should allow explicitly. C makes erasure fast, and a shred that a concurrent revocation later rejects has then destroyed keys for good (see M20).

**M22. The wire layout of the control bodies.** The entry types 0 to 6, the common keys (1 entry type, 2 reserved for a clock, 3 control dependencies), and the field numbers from 8 up for each entry type, as in [The control bodies](#the-control-bodies). They are part of the wire format, so they cannot change once shipped.

- A. As specified. *(recommended)*
- B. One entry type per action (add, set role, remove as separate types).

Recommendation: A. One membership entry with an action keeps the rules about grants and revocations in one place. Field numbers from 8 up leave the keys RFC 0002's payload map uses free for meanings shared with it.

**M23. Two rotations that name the same generation id.**

- A. Both are rejected, and the SDK reports it. *(recommended)*
- B. The one that comes first in the order keeps the id; the other is rejected.

Recommendation: A. Under B, a member that grinds a colliding id can win against an honest rotation that comes later in the order. Under A, it only blocks both, and with derived ids that needs about 2⁴⁸ tries (see [Generation ids](#generation-ids)). Changes sealed under a rejected generation keep their verdict by M17.

**M24. A creator that writes two geneses.**

- A. Both stay in the log; the one with the lower change id is the genesis, and the other folds nothing. *(recommended)*
- B. Both are rejected, and the space is unusable.

Recommendation: A. RFC 0002 keeps and folds both changes of every equivocation, which a space cannot do with two first changes. A keeps both, as RFC 0002 wants, and picks one deterministically. Only the creator's own device can cause it, by a backup restore or a clone.

**M25. Who signs a sub-agent's certificate.** This follows RFC 0003 (decision 15) and is key handling, so it is the owner's.

- A. The parent agent's member signing key, under its own context string; key 1 carries the parent's account key, and members check the chain through the parent's certificate. *(recommended, as RFC 0003)*
- B. The account key, as for every other kind (RFC 0008 draft 0.2).

Recommendation: A. A parent agent never holds the account key and must not (RFC 0003, "Sub-agents get an ad-hoc space"), so B cannot work without handing it the account key. The chain check reads only the space's control changes, because the parent's certificate is in the genesis or add entry that made the parent a member.

The formats of the member certificate, the account sign-off and the key-chain subkey are RFC 0003's decision 15, not this RFC's.

## Conflicts with GENESIS.md and the other RFCs

This RFC agrees with GENESIS.md and RFC 0001: the relay checks membership without reading content; every member, an agent included, is visible in the member list with its own key and role; rights are per space; a removed member keeps its copy; and the rules converge in any order. It differs from them, or from the open drafts, in these points. pm flags each to the owner or routes it to the RFC's ticket.

1. **Groups** (GENESIS.md, "Spaces"; RFC 0001, "The design in one page"). Both say groups can be members of many spaces. This RFC defers them and offers lists the SDK expands instead (M10).
2. **RFC 0002, what the relay sees.** RFC 0002 says the relay cannot see which changes a change builds on. That stays true for sealed changes. For control changes, the relay sees it under M11. RFC 0002's list of what the relay sees, and the FAQ, should add the items in [How the relay applies the rules](#how-the-relay-applies-the-rules).
3. **RFC 0002, D3.** Space ids are derived, not drawn at random (M7). This reopens an owner decision.
4. **RFC 0002, header key 4.** The key generation gets a structure: ordinal and tag (M6). The format does not change.
5. **RFC 0002, equivocation.** RFC 0002 keeps and folds both changes of an equivocation. For the genesis, both are kept but only the one with the lower change id is the genesis. For a revoked member, equivocations inside its cutoff are accepted (M3).
6. **RFC 0002, payload keys.** RFC 0002 gives every payload a clock and dependencies (keys 2 and 3), and says key 1 of a control body names the entry type. The control body keeps key 1 as the entry type and key 3 as dependencies, and leaves key 2, the clock, absent (M18).
7. **RFC 0002 Amendment 1, D12.** A member does not push a refused change to that relay again. This RFC asks for one exception (M19).
8. **RFC 0003, concurrent rotations.** RFC 0003 picks "the generation whose entry has the lower hash". This RFC adds derived generation ids, exposure and the merge rotation (M6, M15).
9. **RFC 0003, the sign-off.** Narrowed to an account with an admin member (M9).
10. **RFC 0003, formats and deferrals.** RFC 0003 now defines the member certificate, the account sign-off and the key-chain subkey (decision 15), and this RFC uses them, including the sub-agent certificate signed by its parent. Revoking a member certificate is deferred to a later RFC about accounts; in v1, removal from every space is the revocation. Deleting a synced space is deferred to a later RFC (see [What this RFC does not cover](#what-this-rfc-does-not-cover)).
11. **RFC 0003 and RFC 0007, the person-key entry.** RFC 0003 has it carry a commitment, RFC 0007 a record hash and a generation. This RFC carries both, and RFC 0003 and RFC 0007 should point to it.
12. **RFC 0007, the membership table and its checks.** Its backend stores, per member, the positions of addition and removal. This RFC needs roles, grants with from generations, revocations with cutoffs and the generation graph. RFC 0007 does not check author sequences; this RFC needs it to, for sealed changes. Its rule that a person-key record replaces one of "the same or a newer generation" becomes "the same or a descendant generation". It deletes a record in the append's transaction; this RFC waits for stability (M20). Its R7 A becomes the cutoff (M3).
13. **RFC 0006, the control table.** RFC 0006's `control` table lists membership entries, key chain entries, held generations, shreds and acknowledgements. It also needs the genesis, the person-key entries, the cutoffs and grants, and the generation graph, as [The function](#the-function) lists them. RFC 0006 un-folds only sealed changes. Control changes are recomputed through `resolve`, and their irreversible effects wait until they are stable.
14. **RFC 0005, D13.** RFC 0005 names no reserved collection or sealed place for the model id. This RFC offers a field in the genesis, recommends against using it (M8), and leaves the place to D13.
15. **Principle 3 and the team lead's erasure.** GENESIS.md promises that erasure destroys a key. Under M20 the key is destroyed only when the shred is stable, which an idle admin device delays. The view erases the fields at once, and M21 sets the alarm and the escalation, but until then the key exists on members. RFC 0001's list of what erasure does not reach should name the wait.
16. **RFC 0004, add wins.** Membership resolves the other way, so the revocation wins (M2). Membership is not a field type of the schema, so RFC 0004's rule for sets stays as it is.

## Summary of the choices

| Question | This RFC's answer |
| --- | --- |
| How does a space start? | With a genesis by its creator, naming the creator's keys and certificate and the first generation; the space id derives from the creator and a nonce. |
| Which roles exist? | Reader, writer and admin, per space. |
| Who may change membership? | Admins. Every member may leave. |
| Who signs off an erasure? | Another account that holds an admin member in the space. |
| What do control changes carry? | An entry type, the control dependencies and the fields of their type, readable by the relay; no clock. |
| What may the fold read? | Control bodies and headers only. Wrong keys and failed decryption are local reports. |
| How does the membership state converge? | One order computed from the set: causal order, revocations first, edges added by seniority, then the rules applied in that order. |
| Concurrent grant and revocation? | The revocation wins; the grant is lost and visible. |
| Two rotations at once? | Two generations with derived, distinct ids; the next admin that sees exposure rotates again. |
| A removed member that keeps writing? | Its changes above the cutoff are rejected by every member and refused by the relay; equivocations inside the cutoff are reported, not rejected. |
| A member raised again? | Only its changes under a fresh generation count. |
| What waits? | Destroying a person key, its "deleted" acknowledgement and the relay's deletion of its record, until the shred is stable. |
| How does the relay apply the rules? | The same code over the control changes it holds; where it holds less, the members decide. |
| Groups? | Deferred; the SDK expands a group into one entry per member. |
