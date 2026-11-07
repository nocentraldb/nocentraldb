# RFC 0008: Membership, roles and the first change of a space

REQUEST FOR COMMENTS · DRAFT 0.1 · NOVEMBER 2026 · ticket NOC-23

This RFC defines who belongs to a space and what each member may do: the first change of a space, the roles and their rights, the bodies of the control changes, and the rules by which every member and the relay fold those control changes into one membership state, the same on every copy, in any order of arrival.

It builds on [RFC 0001](0001-nocentraldb.md), above all [the design in one page](0001-nocentraldb.md#the-design-in-one-page) and [what it is not](0001-nocentraldb.md#what-it-is-not), on [RFC 0002](0002-change-and-log-format.md), which reserves control changes as [kind 1](0002-change-and-log-format.md#key-generation-suite-and-kind) and leaves "membership changes" to a later RFC, and on the principles in [GENESIS.md](../../factory/GENESIS.md). It uses four RFCs that are open drafts at the time of writing, by name and without links: RFC 0003 (keys, NOC-7, PR #27), which names the control changes and their purpose; RFC 0004 (merge rules, NOC-8, PR #26), whose style it follows; RFC 0006 (the local view, NOC-12, PR #36), which folds control changes into a per-space control state that this RFC defines; and RFC 0007 (the relay, NOC-13, PR #37), which applies the membership rules with the data it can see. It also uses the "rejected" log state that NOC-22 adds to RFC 0002.

**Status.** A draft for the owner's decision. Membership rules decide how permissions behave, which GENESIS.md asks us to tell developers plainly, and the control-change bodies are part of the wire format. Both are reserved for the owner. Nothing here is built. The open decisions are in [Decisions for the owner](#decisions-for-the-owner), each with options and a recommendation. Until the owner decides, the design below describes the recommended option of each decision.

## What this RFC does not cover

- **Work orders**, beyond who may sign off an irreversible one. A later RFC defines how a work order is claimed, performed and validated.
- **Cryptography.** The primitives, the key envelope's HPKE construction and the person-key commitment are RFC 0003's. This RFC places their outputs in the control bodies and says who may write them.
- **Invitations and public API names.** How a new member's public identity reaches the adding member (RFC 0003, "Sharing a space key with a new member", step 1) and what the SDK calls are named. The role names below are working names (see decision M12).
- **Groups as members**, which this RFC names and defers (see [Groups](#groups)).
- **Compaction.** A later RFC. This RFC says what compaction must keep: every control change, because the membership state is computed from all of them.
- **Code.** Nothing in `crates/` or `packages/` changes.

## Terms

| Term | Meaning in this RFC |
| --- | --- |
| Space, member, change, header, payload, frame, log | As in RFC 0002. |
| Control change | A change of kind 1 (RFC 0002). Its payload is a control body the relay can read. It carries no content. |
| Sealed change | A change of kind 0 (RFC 0002): content or a work order, encrypted. |
| Genesis | The first change of a space. A control change. |
| Control DAG | The control changes of a space, linked by their control dependencies. |
| Control state | What a member computes from all control changes it holds: who is a member, with which role, which key generations exist, which person keys are shredded. RFC 0006 stores it in its `control` table. |
| Role | A named set of rights in one space: reader, writer or admin. |
| Grant | A membership entry that gives a member a role, or a higher one. |
| Revocation | A membership entry that takes rights away: it lowers a member's role or removes the member. |
| Cutoff | The last change of the revoked member that the revoking member had seen. A revocation names it. |
| Generation id | The 64-bit number in RFC 0002's header key 4 that names one space key generation. |
| Seniority | The order of admins by when they became admin; the creator is the most senior. Used only to break cycles of concurrent revocations. |

## The design in one page

- **A space starts with a genesis.** The creator writes it. It names the creator's public keys and certificate and the first key generation. The space id is derived from the creator's member id and a random nonce, so nobody else can write a genesis for that space.
- **Three roles.** A reader holds the keys and reads everything. A writer also writes content and person keys and performs work orders. An admin also adds, removes and changes the roles of members, rotates the space key, and signs off irreversible work orders. Rights apply to the whole space (RFC 0001: no row-level rights).
- **Every control change names what its author had seen.** Its control dependencies are the heads of the control DAG in the author's log. They are readable by the relay, so the relay can apply the same rules as the members.
- **A revocation names a cutoff.** Removing a member, or lowering its role, names the last change of that member the revoking admin had seen. The member's later changes, and those the admin had not seen, lose the revoked right on every member and on the relay. A removed member that keeps writing offline therefore writes nothing anyone accepts.
- **Revocations win.** A revocation wins over a concurrent grant. A removed member comes back only through an add whose author had seen the removal. When two admins revoke each other at the same time, the more senior one's revocation stands.
- **The membership state is a function of the set.** Every member puts its control changes into one order computed from the changes alone, and applies them in that order. The order of arrival never matters.
- **Concurrent rotations merge.** A generation id carries a random tag, so two rotations on two islands never share an id. When the current generation is known to a removed member, the next admin that notices rotates again.
- **The relay runs the same code.** It applies the rules to the control changes it holds, and checks sealed changes against the cutoffs by their author sequence. Where it sees less than the members, the members decide, and [How the relay applies the rules](#how-the-relay-applies-the-rules) lists each such case.

## The first change of a space

### Who creates it

Any member creates a space by writing its genesis, on its own device, offline if it likes. The creator becomes the space's first member and its first admin. A relay accepts the genesis only from a creator its operator allows (RFC 0007, decision R6); members do not check that, because it is a rule of the relay, not of the space.

### What it names

The genesis is a control change whose body (see [Genesis](#genesis)) names:

- **The space id**, through a nonce. The space id is the first 16 bytes of `H("nocentraldb/space-id" || creator member id || nonce)`, where the nonce is 16 random bytes in the body and H is SHA-256 (RFC 0003). Like RFC 0002's [object ids](0002-change-and-log-format.md#object-ids), it is 128 bits nobody can predict and holds no time. Unlike them, it binds the space to its creator (decision M7).
- **The creator**: its signing key, its agreement key and its member certificate (RFC 0003). Its member id is the header's author member id (RFC 0002, header key 2), which every reader recomputes from the two keys.
- **The first key generation**: its generation id (see [Generation ids](#generation-ids)). The genesis carries no envelope. The creator writes a key envelope for itself as its second change, so that its other devices and the relay's `keys` request find the key like any member's.
- **The suite**, in the header (RFC 0002, header key 8). The genesis's suite is the space's suite. A later suite would arrive with a key chain entry for a new generation; v1 has one suite.

The genesis does **not** name the space's name, its description or the model it uses. Those are content, and the relay must not read them (principle 1). RFC 0005 (decision D13 A) asks for the model id "in the first control entry"; this RFC recommends a sealed place for it instead (decision M8). The creator's third change, a sealed change, records the model id as an operation on a reserved collection that RFC 0005 names; members read it, the relay cannot.

### How the relay and members recognise it

A control change is a genesis when its entry type is genesis. It is valid when:

- its author sequence is 1, and its control dependencies are empty;
- the space id in the header equals the space id derived from the author member id and the nonce;
- its signature verifies with the signing key in its body, which hashes, with the agreement key, to the author member id;
- the certificate verifies against the account key it names (RFC 0003).

A space has exactly one valid genesis. Two different valid geneses for one space id can come only from the creator itself, writing twice with the same nonce, which is an equivocation (RFC 0002). Every member then takes the one with the lower change id and rejects the other; a member that holds only one takes that one. Nobody else can make a competing genesis, because their member id derives another space id.

The relay creates a space's log when it accepts a valid genesis for a space id it does not hold, and answers `space_exists` otherwise (RFC 0007). Every other control change has the genesis in its causal past: its control dependencies, followed back, reach the genesis. A control change whose dependencies are empty and that is not a genesis is malformed.

## Roles and rights

Rights are per space. Every member of a space holds its keys and a full copy (RFC 0001), so there is no right to read less than all of it. A member holds exactly one role:

| Right | Reader | Writer | Admin |
| --- | --- | --- | --- |
| Receive key envelopes and read the whole space | yes | yes | yes |
| Write acknowledgements of shred entries | yes | yes | yes |
| Leave the space, or lower its own role | yes | yes | yes |
| Write sealed changes: content and work orders | | yes | yes |
| Write person-key entries and person-key records | | yes | yes |
| Perform a work order, and write the shred entry of an erasure | | yes | yes |
| Add members, and raise or lower another member's role | | | yes |
| Remove members | | | yes |
| Rotate the space key: write key chain entries and key envelopes | | | yes |
| Sign off an irreversible work order, with its account key | | | yes, as an account (see below) |

- **Who may write what is checked per change.** Each control change type needs one right (see [The control bodies](#the-control-bodies)). A sealed change needs the right to write sealed changes. Both members and the relay check it.
- **The sign-off of an irreversible work order** comes from an account, not a member (RFC 0003). The account must hold, in the space, a member whose role is admin, and must not be the account of the member that performs the work order. This narrows RFC 0003, which asks only for "an account that has a certified member in the space" (decision M9).
- **A reader cannot write content**, but it can read everything, and its device can leak what it read. The reader role limits writes, not knowledge (principle 4). Typical readers are adapters (RFC 0007), dashboards and auditors.
- **Agents hold roles like every member.** The SDK adds an agent as a writer unless the app asks for another role. A sub-agent in an ad-hoc space (RFC 0003) is a writer there, and its parent is the admin.
- **A space can lose its last admin**, when the last admin leaves or two admins remove each other. The SDK refuses to let the last admin leave on its own device, but it cannot prevent the concurrent case. A space without an admin keeps working for content, but its membership is frozen: nobody can add, remove or rotate (decision M13).

What the roles do **not** do:

- They give no rights inside the space's data. Every writer may change every object of the space. Finer rights come from more spaces (RFC 0001, "What it is not").
- A revocation does not take back what a member already read (RFC 0003, "What rotation does not do").
- A role is checked against the changes a member holds. A member on an island that has not seen a revocation keeps treating the revoked member as it was, until the revocation reaches it.

## The control bodies

Every control change carries its body as RFC 0002's payload (header key 6): a CBOR map in RFC 0002's [canonical profile](0002-change-and-log-format.md#the-cbor-profile), not encrypted. Keys 1 and 2 are the same in every body:

```text
control body (CBOR map; the payload of a kind 1 change)
├── 1 entry type          uint, see the table below
├── 2 control deps        array of change ids, sorted by bytes, no id twice
└── 3...                  the fields of the entry type
```

| Entry type | Name | Right its author needs |
| --- | --- | --- |
| 0 | Genesis | none: it creates the space |
| 1 | Membership entry | admin; or any member, for its own leave or lower role |
| 2 | Key envelope | admin |
| 3 | Key chain entry | admin |
| 4 | Person-key entry | writer |
| 5 | Shred entry | writer, plus an admin account's sign-off |
| 6 | Acknowledgement | reader |

Other entry types are reserved. A member that meets one keeps the change, parked, as RFC 0002 does for every [change it does not understand](0002-change-and-log-format.md#versioning-old-clients-keep-what-they-do-not-understand). Keys 3 to 63 of a body are "must understand", keys from 64 up "may ignore", as for RFC 0002's maps.

**Control dependencies** (key 2) are the change ids of the heads of the author's control DAG when it wrote: the control changes no other control change in its log depends on yet. They also cover the author's own previous change in the space, of either kind, when the heads do not already cover it, so that the author's changes form one chain (see [A member removed while offline who keeps writing](#a-member-removed-while-offline-who-keeps-writing)). A sealed change keeps its dependencies inside its payload, as RFC 0002 defines; they reach control changes too, since the author's log holds both kinds.

**The header** of a control change has the same keys as every change (RFC 0002). Its key generation (header key 4) is the generation the author writes sealed changes with at that moment, or the first generation, for the genesis.

**Control changes carry no clock.** The relay sees when it accepted each one, and the members see when they received it; nobody sees when it was written. The audit of a space's membership has an order (see [Folding the membership state](#folding-the-membership-state)) but no author time. A clock in the control body would show the relay when admins act, offline work included, as RFC 0002 decided against for every change (D6).

### Genesis

```text
genesis (entry type 0)
├── 3 nonce               16 random bytes; the space id derives from it
├── 4 signing key         32 bytes, the creator's Ed25519 public key
├── 5 agreement key       32 bytes, the creator's X25519 public key
├── 6 certificate         the creator's member certificate (below)
└── 7 first generation    uint, the generation id of generation 1
```

### Member certificate

RFC 0003 defines what a member certificate says: "member id M belongs to account A, as a device" or "as an agent", and, for a sub-agent, which parent it serves. This RFC fixes its fields, so that the relay and every member can check it:

```text
member certificate (CBOR map)
├── 1 account key         32 bytes, the account's Ed25519 public key
├── 2 member id           32 bytes
├── 3 kind                uint: 1 device, 2 agent, 3 sub-agent
├── 4 parent member id    32 bytes, for a sub-agent only
└── 5 signature           the account key's signature over
                          H("nocentraldb/member-cert" || canonical map of keys 1 to 4)
```

### Membership entry

```text
membership entry (entry type 1)
├── 3 action              uint: 1 add, 2 set role, 3 remove
├── 4 member id           32 bytes, the member the entry is about
├── 5 role                uint: 1 reader, 2 writer, 3 admin (absent for remove)
├── 6 signing key         32 bytes   ┐
├── 7 agreement key       32 bytes   │ add only
├── 8 certificate         map        │
├── 9 from generation     uint       ┘ the generation id the member starts at
└── 10 cutoff             {1: author sequence, 2: change id}, or null
                          set role and remove only: the subject's last change
                          the author had seen; null if it had seen none
```

- **Add** grants a role to a member that is not a member in the author's control state. Its author checks the certificate first (RFC 0003). It also writes a key envelope for the new member, for the generation named in key 9. The new member's sealed changes count only from that generation on.
- **Set role** raises or lowers a member's role. When it lowers it, it is a revocation of the rights the member loses, and names a cutoff. When it raises it, it is a grant, and its cutoff is null.
- **Remove** is a revocation of every right, with a cutoff. An admin removes; any member may remove itself (leave), naming its own last change as the cutoff. Every removal is followed by a rotation (see [Key chain entry](#key-chain-entry)). A member that leaves cannot rotate, because it must not know the next key; the next admin that sees the removal rotates for it.
- **Re-adding a removed member** is an add whose author had seen the removal: the removal is in the add's causal past. Its from generation must have a higher ordinal than every generation the member held an envelope for, so that what the member wrote while removed, under the old generations, never counts. An admin that re-adds a member whose removal no rotation has answered yet rotates first. An add that did not see a removal of the same member does not undo it (see [Concurrent add and remove of one member](#concurrent-add-and-remove-of-one-member)).

### Key envelope

RFC 0003 fixes its content: one generation's space key, wrapped with HPKE to one member's agreement key, with an `info` that binds the space id, the generation and the recipient's member id. Its fields:

```text
key envelope (entry type 2)
├── 3 recipient           32 bytes, the recipient's member id
├── 4 generation          uint, the generation id of the wrapped key
├── 5 encapsulated key    bytes, HPKE's enc
└── 6 ciphertext          bytes, the HPKE ciphertext of the 32-byte key
```

The recipient and the generation sit outside the wrapping, so the relay can serve each member its own envelopes (RFC 0007, `keys`). The author must be an admin that holds the generation. The recipient must be a member in the control state at the envelope's place in the order below; an envelope to anyone else is rejected, and the relay refuses it.

### Key chain entry

RFC 0003 fixes its content: the previous generation's key, encrypted under the new generation, so that the holder of the new key can read the whole history. With concurrent rotations, a generation can have more than one previous generation, so the entry lists them:

```text
key chain entry (entry type 3)
├── 3 generation          uint, the new generation id
├── 4 previous            array of {1: generation id, 2: the previous key, AEAD-encrypted
│                         under the new generation's key-chain subkey (RFC 0003)}
└── 5 answers             array of change ids: the revocations this rotation answers
```

The author must be an admin that holds every previous generation it lists. The new generation's ordinal is one more than the highest ordinal among them (see [Generation ids](#generation-ids)). After a key chain entry the author writes a key envelope for every member of the control state, itself included, and none for a removed member. It also re-wraps the person-key records under the new generation (RFC 0003, step 3).

### Person-key entry

RFC 0003 and RFC 0007 each define part of it. RFC 0003 has the entry carry the person-key id and a commitment to the key; RFC 0007 has it announce each person-key record by generation and hash, so that the relay can accept a record only with its entry. This RFC carries both:

```text
person-key entry (entry type 4)
├── 3 person-key id       16 random bytes (RFC 0003)
├── 4 commitment          32 bytes, H("nocentraldb/person-key-commit" || space id ||
│                         person-key id || key) (RFC 0003)
├── 5 generation          uint, the generation id the record is wrapped under
└── 6 record hash         32 bytes, SHA-256 of the wrapped record bytes (RFC 0007)
```

The first entry for a person-key id creates the person key; every later one announces a re-wrapped record and must repeat the same commitment. An entry for a shredded person-key id is rejected.

### Shred entry

```text
shred entry (entry type 5)
├── 3 person-key id       16 bytes
└── 4 sign-off            {1: account key, 2: signature over
                          H("nocentraldb/shred-sign-off" || space id || person-key id ||
                          performer member id)}
```

The performer is the shred entry's author, a writer. The sign-off's account must hold an admin member in the space and must not be the account of the performer's certificate. The shred entry names no work order: the relay must not learn which sealed change was a work order (RFC 0001). The work order, inside its sealed change, names the shred entry instead.

Two shred entries for one person-key id are both accepted; the first in the order below takes effect, the second changes nothing. A shred wins over a concurrent person-key entry for the same id: after the shred, every person-key entry and record for it is rejected.

### Acknowledgement

```text
acknowledgement (entry type 6)
└── 3 shred               change id of the shred entry
```

Any member writes one when it has deleted its copy of the key (RFC 0003, "Shredding a person's key", step 3). A second acknowledgement of the same shred by the same member changes nothing. A removed member's acknowledgement is refused like any change after its cutoff; the audit then shows the member as never having confirmed.

### Generation ids

RFC 0002's header key 4 is a uint that names the key generation. RFC 0003 numbers generations and admits that two islands can rotate at the same time. Two rotations that both produce "generation 5" would make every change under generation 5 ambiguous: a member could not tell which key to use, and RFC 0003 forbids trying several. This RFC therefore gives the uint a structure:

```text
generation id (uint, 64 bits)
├── upper 24 bits   ordinal: 1 for the genesis, one more than the previous highest per rotation
└── lower 40 bits   tag: random, drawn by the member that creates the generation
```

Two concurrent rotations have the same ordinal and, but for a chance of 2⁻⁴⁰ per pair, different tags. Generations are compared by ordinal. If two key chain entries do name the same generation id, the one earlier in the order below keeps it and the other is rejected; changes sealed under the rejected one fail to decrypt and are kept as rejected. This keeps RFC 0002's format and refines its meaning (decision M6).

## Folding the membership state

The control state must be the same on every member that holds the same control changes, whatever order they arrived in (principle 2), and the relay must reach it too. This section defines it, as RFC 0004 defines a field's state, as a function of the set.

### The function

Let S be the set of control changes of a space that a member holds, each with a valid signature and a canonical body, after the genesis is chosen. The control state is `resolve(S)`:

1. **Causal order.** A control change comes after every control change in its control dependencies. A dependency the member does not hold is a gap: the SDK reports it and asks for it (RFC 0002), and the order ignores it until it arrives.
2. **Revocations first.** A revocation comes before every change of its subject that it excludes (see [The cutoff](#the-cutoff)). This never contradicts the causal order: an excluded change is outside the revocation's causal past, so the revocation does not depend on it.
3. **Cycles.** Rule 2 can form a cycle when revocations are concurrent and exclude each other, as when two admins remove each other. In each cycle, the revocation whose author is the most junior admin, by seniority in the state of that revocation's own causal past, loses its "revocations first" edge, and the cycle is broken. Repeat until no cycle is left.
4. **Ties.** Among the changes that may come next, the one whose author is the most senior in the state so far comes first, then the one with the lower change id, as bytes.
5. **Apply in that order.** Start from the genesis's state: its author is the only member, an admin, at the first generation. Take each next change and accept it when:
   - its author holds the right its entry type needs (see [The control bodies](#the-control-bodies)) in the state so far, and also in the state of its own causal past, that is in `resolve` of the changes it depends on;
   - no accepted revocation of its author excludes it;
   - its own rules hold: an add's subject is not a member, or was removed and the add saw the removal; an envelope's recipient is a member; a key chain entry's previous generations exist; a shred's sign-off is valid; and so on.

   An accepted change updates the state. A rejected change is kept in the log in RFC 0002's rejected state with its reason, and changes nothing.

**Seniority.** The creator is the most senior admin. Other admins rank by the grant that made them admin, in the order above: an earlier grant ranks higher. A member that was granted admin twice ranks by its earlier grant. Seniority matters only in rules 3 and 4.

**Roles.** A membership entry that sets a role r (an add, a set role, or a remove, which sets "none") is a grant of r and caps every grant of the same member that did not see it at r. A member's role is the highest role among its accepted grants, each lowered to the lowest cap of the accepted entries that did not see that grant. A grant therefore loses to a concurrent removal or demotion, and keeps the rights it gives over every entry it saw: a re-add that saw the removal stands.

**The state** holds, per member id: its public keys and certificate, its role, each of its grants with its from generation, and each accepted revocation with its cutoff. Per generation: its id, its previous generations, and which members hold an accepted envelope for it. Per person-key id: its commitment, its newest record's generation and hash, and whether a shred entry destroyed it, with its acknowledgements. It is canonical: equal states encode to equal bytes, so the convergence harness can compare them (RFC 0004, C1).

**Sealed changes** are checked against the final state, not against the order. A sealed change by author X under generation g is accepted when X holds a grant of a role that writes, with a from generation of at most g's ordinal, and no accepted revocation that did not see that grant excludes the change. A sealed change that fails is rejected with its reason, as RFC 0006 says, and waits when a later control change could still accept it.

### Why it converges

Every step depends only on S. The causal order comes from the control dependencies, which are inside the signed changes. The cutoffs are inside the revocations. Seniority comes from the order itself, and ties fall to change ids, which every member computes from the bytes (RFC 0002). No step reads the order of arrival, a clock or anything outside the changes. Two members that hold the same S therefore compute the same order and accept the same changes, and the sealed changes are checked against the same final state. The same holds for every subset, which is what "in the state of its own causal past" uses.

The fold does not need to be incremental. A space holds few control changes, hundreds rather than millions, so a member recomputes `resolve(S)` whenever a control change arrives, and RFC 0006 re-checks and un-folds the sealed changes whose verdict changed. That is RFC 0006's "un-folding": a revocation that arrives late can reject a change the member had already folded.

### The cutoff

A revocation names its subject's last change that its author had seen: the subject's author sequence and that change's id, or null if the author had seen none of the subject's changes. Every change of the subject is either inside the cutoff or excluded:

- **Inside**: the change is in the revocation's causal past, so the revoking admin had seen it. Members check this from the control dependencies and from the dependencies they decrypt in sealed changes. The subject's changes form a chain, because every change depends on its author's previous change (RFC 0002, [dependencies](0002-change-and-log-format.md#dependencies), and the control dependencies above), so the inside changes are the cutoff change and the subject's changes before it.
- **Excluded**: everything else, whether written after the revocation or concurrently with it, or written earlier but never seen by the revoking admin.

The cutoff itself is the summary the relay needs, because it cannot read the dependencies of sealed changes: it checks the author sequence against the cutoff's (see [How the relay applies the rules](#how-the-relay-applies-the-rules)). A sealed change whose chain back to the revocation is still incomplete, because a change in between has not arrived, waits, as RFC 0006 defines.

An excluded change loses only the rights the revocation took away. A writer demoted to reader keeps acknowledging shreds; a removed member keeps nothing.

The cutoff is what makes the rule safe against a member that lies. A member can choose its control dependencies freely and claim it never saw its own revocation. It cannot change which of its changes the revoking admin had seen. A change it writes after the revocation has a higher author sequence than the cutoff, or, if it reuses an old sequence number, is an equivocation that is not in the revocation's causal past. Either way it is excluded.

### Concurrent add and remove of one member

Alice removes Mallory's laptop, which was stolen. At the same time, on another island, Bob, who has not seen the removal, raises the laptop's role from writer to admin. Both entries are valid in their authors' views.

Every member puts Alice's removal before the laptop's changes that Alice had not seen. Bob's entry is a grant that did not see the removal, so it does not undo it: a removal is undone only by an add that saw it. On every copy, the laptop is removed, and every change it made after Alice's cutoff is rejected, whether it reached a member before or after the removal. Bob's grant stays in the log, accepted but without effect, and the SDK shows it as a **lost grant**, the way RFC 0004 shows a lost removal.

This is the opposite of RFC 0004's sets, where an addition wins over a concurrent removal. For data, a lost addition is data that is gone. For membership, a lost removal is a stolen device that keeps writing, so the removal wins, and the lost grant is cheap to repeat once Bob has seen the removal (decision M2). Concurrent role changes follow the same rule: when two admins set a member's role at the same time, the member ends with the lower role.

### Two removals that rotate the key at once

Alice removes Xavier and rotates the key; at the same time, on another island, Bob removes Yvonne and rotates. Alice's new generation has an envelope for Yvonne, Bob's has one for Xavier. When the islands meet, every member holds both removals and both key chain entries:

1. Both removals are accepted; neither excludes the other. Xavier and Yvonne are removed on every copy.
2. Both generations exist, with the same ordinal and different tags. Neither is safe: each is held by a removed member.
3. The **current generation**, the one members write sealed changes with, is a head of the generation graph: a generation no accepted key chain entry lists as previous. Among the heads, it is the one with the highest ordinal, then the one whose key chain entry comes first in the order above. Every member picks the same one. A member that holds no envelope for it writes with the newest generation it holds, and the SDK reports that it waits for an envelope.
4. A generation is **exposed** when a removed member holds an accepted envelope for it, or for a later generation whose key chain leads back to it. When the current generation is exposed, an admin writes a **merge rotation**: a key chain entry that lists every head as previous, with envelopes for the current members only. Any admin that sees an exposed current generation does so, after a random delay of up to a minute, unless it sees another admin's merge rotation first.
5. Two merge rotations written at the same time are two concurrent rotations again, and step 4 repeats. Each round needs two admins to act within the same minute without seeing each other, so the repetition ends quickly.

**What this does not do.** Between the meeting of the islands and the merge rotation, members write under an exposed generation, which Xavier or Yvonne could read if they obtained those changes. The SDK reports the exposure until it ends. If no admin is online, it lasts until one is. The same rule covers a member that left (it cannot rotate for itself) and a removal whose author never rotated, for example because its device crashed between the two changes.

### Two admins who remove each other

Alice removes Bob, and at the same time Bob, or whoever holds Bob's device, removes Alice. Each removal's cutoff excludes the other, so rule 2 forms a cycle. Rule 3 breaks it in favour of the senior admin: if Alice became admin first, her removal comes before Bob's, Bob is removed, and Bob's removal of Alice is excluded by Alice's cutoff. Every member computes the same outcome.

Seniority is not a judgement of who is right. It is a deterministic way to keep one of the two removals, so that a compromised admin cannot cancel its own removal by removing the remover at the same moment (decision M4). If the junior admin was the honest one, a senior admin, or the creator, must remove the compromised admin again once the honest admin reports it.

### A member removed while offline who keeps writing

Carol's laptop is offline for a week. Meanwhile Alice removes it, naming as cutoff the last change of the laptop she had seen, its author sequence 40. The laptop keeps writing, sequences 41 to 60, under the old generation, the only one it holds.

- **On the relay.** When the laptop reconnects, it pulls first (RFC 0007, island mode), sees its removal, and the SDK reports it. If it pushes anyway, the relay refuses sequences 41 to 60: they are above the cutoff.
- **On members that received them directly**, on the laptop's island: when Alice's removal reaches them, the fold rejects sequences 41 to 60 and un-folds what they had applied (RFC 0006). Their state then equals every other member's.
- **On the laptop itself.** It folds its own removal like any member, and its own changes after the cutoff become rejected in its log. Its copy keeps everything it received before; the SDK tells the app that the changes made since the cutoff will not be accepted by anyone.

Changes the laptop wrote before Alice's removal but after sequence 40, which Alice had not seen, are rejected too. The removal's author decides by what it had seen, because the relay and the members cannot tell an honest late change from a backdated one: the clock is encrypted, and a removed member can write anything under the old generation. The SDK warns an admin who removes a member whose recent changes it has not received yet (decision M3).

Changes at or below the cutoff that arrive late are accepted everywhere, also by the relay. This is the improvement over RFC 0007's decision R7 A, which refuses every change of a removed member from the removal on.

### What this does not do

- **A member that has not seen a revocation** treats the revoked member as it was. It accepts the member's changes until the revocation arrives, and then rejects those beyond the cutoff (RFC 0006, un-folding). The app sees those changes appear and disappear.
- **A revoked member keeps its copy.** Revocation and rotation keep it from writing and from reading what comes later. It still holds what it received (RFC 0003).
- **Grants a revoked admin made before its cutoff stay.** If a stolen device added a member before the removing admin had seen it, that member stays until an admin removes it too. The SDK lists the members a removed admin added, so that the remover can review them.
- **Nothing is enforced at write time.** Two admins can make decisions that contradict each other; the rules decide afterwards, the same way on every copy.

## How the convergence harness checks it

The control state is treated like a field type under RFC 0004's plugin contract: its state is `resolve(S)`, its apply adds one control change to S, its read is the membership the SDK shows, and its outcome per control change is **accepted**, **rejected** with a reason, or **lost** for a grant a concurrent revocation overrode. The convergence harness (NOC-9) and its plugin-contract checks (NOC-19) run it with these generators and checks:

| # | Property | How the harness checks it |
| --- | --- | --- |
| M-C1 | Converges in any order: equal sets of control changes give equal control state bytes. | Simulated admins, writers and readers on several islands write membership entries, envelopes, rotations, person-key entries, shreds and acknowledgements, each from its own view. The harness delivers the union to at least three copies in random orders and compares the canonical state bytes. |
| M-C2 | Idempotent. | Duplicates in every delivery; the whole set applied twice. |
| M-C3 | Deterministic natively and in WebAssembly. | The same seed on both targets (NOC-10). |
| M-C4 | Nothing lost silently: every control change and every sealed change has one verdict, the same on every copy. | Compares the verdicts and reasons of every change, sealed changes included, across copies. |
| M-C5 | Robust against hostile members. | Removed members that keep writing, equivocations below the cutoff, control dependencies that lie, mutual and ring-shaped removals, concurrent rotations, envelopes to non-members, shreds without a valid sign-off. |
| M-C6 | The checks fire. | Broken fixtures that the harness must reject: add wins for membership; arrival order instead of the computed order; a revocation without its cutoff; seniority by arrival; two concurrent rotations under one generation id. |
| M-C7 | Safety holds on every copy. | After every delivery: no accepted change of a member lies beyond an accepted revocation's cutoff; a removed member is never a recipient of an accepted envelope for the current generation once a merge rotation is accepted; every accepted shred has a sign-off from another account with an admin member. |
| M-C8 | The relay agrees. | A simulated relay receives the same changes in its own order, applies the relay's checks, and re-accepts what members push again. After the pushes settle, its membership table equals `resolve` of its own log, and every change it refused is rejected by the members, except the cases listed in [How the relay applies the rules](#how-the-relay-applies-the-rules). |

## How the relay applies the rules

The relay holds no key, but it holds every accepted control change of a space, and their bodies, cutoffs and control dependencies are readable (RFC 0002, kind 1). It therefore runs the same `resolve`, from the same Rust code that the members run, over the control changes in its log. Its membership table (RFC 0007, "The backend interface") is `resolve` of its log, recomputed and stored with each append of a control change.

**On a push**, the relay checks each change against its current state (RFC 0007, "What the relay checks", plus):

- **A control change** is accepted when `resolve` over the relay's log plus that change accepts it. Dependencies the relay does not hold count as gaps, as on a member.
- **A sealed change** is accepted by the members' rule, with the author sequence in place of the causal past: its author holds an accepted grant of a writing role whose from generation is at most the change's generation ordinal, and its author sequence is at or below the cutoff of every accepted revocation of writing that did not see that grant.
- **The uploader** must be a current member (RFC 0007).
- **A person-key record** needs its person-key entry (RFC 0007, R12), and a shred entry's sign-off is checked against the certificates in the membership table.

Where the relay and the members can disagree, and who decides:

| Case | Relay | Members | What follows |
| --- | --- | --- | --- |
| A removed member reuses an author sequence at or below the cutoff (an equivocation). | Accepts it: it sees only the sequence number. | Reject it: it is not in the revocation's causal past, which they check from the decrypted dependencies. | The bytes stay in the relay's log; every member rejects them. The members decide. |
| A revocation arrives after the relay accepted a change it excludes. | Has accepted it, and never removes an entry from its log. | Reject it when the revocation arrives. | The relay's membership table follows the revocation; the stale entry stays as bytes. The members decide. |
| The relay refused a change because of a revocation that a more senior concurrent revocation later cancels. | Refused it at the time. | Accept it, once they hold both revocations. | A member whose fold accepts a change the relay refused pushes it again, and the relay accepts it then. This amends RFC 0007, which says never to retry `author_not_member` and `membership_rule`. |
| Changes that never reach the relay, for example exchanged only on an island. | Does not know them. | Fold them. | Members that hold different sets may differ, as RFC 0002 states. |
| A sealed change does not decrypt, or a key envelope or person-key record holds a wrong key. | Cannot tell. | Reject it, and the signature names the author. | The members decide (RFC 0007). |
| A space is created by a creator the relay operator does not allow. | Refuses the genesis (RFC 0007, R6). | Do not check it. | The space works in island mode and on relays that allow its creator. |

The relay therefore decides nothing the members cannot check. It can be more permissive than the members, never stricter for long: what it refuses wrongly, members push again.

**What the relay learns from this RFC**, in addition to RFC 0002's and RFC 0007's lists:

- Each member's **role**, and when it changes.
- The **cutoffs**: for each revocation, the last change of the subject the revoking admin had seen.
- The **control dependencies**: which control changes each admin had seen when it wrote, so how up to date each admin was, and which admins acted on islands.
- The **account** behind each member certificate, and so which members belong to one person or organisation, and which of them are agents or sub-agents of which parent.
- Which **accounts sign off** erasures, and the structure of the generation graph: when concurrent rotations happened.

It still learns no content, no space name and no model id (decision M8). The relay needs every item above to apply the rules, which is why they are outside the encryption (decision M11).

## Groups

GENESIS.md says that "groups can be members of many spaces, so rights change without moving data". A group would be one principal, such as "the design team", that is a member of many spaces; adding a person to the group would give them every one of those spaces at once.

This RFC **defers groups** to a later RFC, for three reasons:

- **A group's membership lives outside the space.** If a space's control state depended on a group's membership, which changes in another log, then folding one space would depend on the order of another space's changes. RFC 0002 keeps every change, and every fold, inside one space. Crossing spaces needs its own convergence argument.
- **Removing someone from a group rotates every space of the group.** With per-member envelopes (RFC 0003, decision 1), that is one rotation per space and one envelope per remaining member in each. A group key would reduce the envelopes but adds a second layer of keys and rotations to design.
- **The relay would have to check membership through the group**, which needs the group's control state on every relay that holds any of its spaces.

Until then, the SDK offers a group as a list it expands: adding a group to a space writes one add entry per member of the group, and removing a person from a group writes one removal in each space the SDK added them to through it. Rights still change without moving data; what v1 loses is a single entry that changes many spaces at once, and a group that exists in the control state (decision M10). This partly conflicts with GENESIS.md, see [Conflicts with GENESIS.md and the other RFCs](#conflicts-with-genesismd-and-the-other-rfcs).

## Rejected options

| Option | Why not |
| --- | --- |
| Add wins for membership, as RFC 0004's sets | A concurrent grant would undo the removal of a stolen device. |
| The order of arrival, or the relay's order, decides | Differs between members and islands, so copies diverge (principle 2). |
| The relay's membership table as the truth | Members could not check it, and island mode would stop all membership changes. |
| Refusing every change of a removed member from its removal on (RFC 0007, R7 A) | Loses honest changes the remover had already seen, with no gain: the cutoff refuses the same backdated changes. |
| Revoking by time | The clock is encrypted, and a removed member can write any clock. |
| Members judge by the cutoff's author sequence alone, as the relay does | A removed member could reuse old sequence numbers; the causal past closes that. |
| Re-adding a removed member id with any add | A stale add from an island would bring back a removed device. |
| Never re-adding a member id | A member key is per device, across all spaces; a removal by mistake would force a new device key everywhere. |
| A total order of membership changes, as MLS commits | Needs one party to order them, which island mode cannot have (RFC 0003). |
| A quorum of admins for each membership change | Needs several admins online; a small team's space would stop when one is on holiday. |
| Both of two mutual removals take effect | A stolen admin device takes an honest admin down with it, and can leave the space without an admin. |
| Neither of two mutual removals takes effect | A compromised admin cancels its own removal by removing the remover at the same moment. |
| One generation per ordinal; a second rotation with the same ordinal is rejected | Changes written on the losing island would become undecryptable. |
| The relay or a member picks the current generation by arrival | Members would write under different generations and disagree on which changes are exposed. |
| A random space id, and the first genesis wins | Anyone could write a competing genesis and, with a lower change id, take over the space on members that hold both. |
| The model id in plain text in the genesis | The relay would learn which app every space belongs to; RFC 0002 lists the model as something it cannot see. |
| Free capability lists per member instead of three roles | More for every member and the relay to check and get wrong; three roles cover the use cases, and capabilities can be added later as new roles. |
| Rights per collection or per object | Needs a reader that enforces them on content, which the relay cannot be (RFC 0001). More spaces give the same effect. |
| Hiding the control dependencies from the relay | The relay could not apply the rules, so anyone with a valid key could fill a space with control changes only members reject. |

## Decisions for the owner

GENESIS.md reserves the wire format, how permissions behave and public API names for the owner. Each decision below names the options and a recommendation.

**M1. Roles.**

- A. Reader, writer and admin, per space. *(recommended)*
- B. Writer and admin only.
- C. A set of capabilities per member.

Recommendation: A. Readers cover adapters, dashboards and auditors, which should not write. C adds a check per capability on every member and the relay for cases nobody has asked for yet; a new role can be added later.

**M2. A grant concurrent with a revocation of the same member.**

- A. The revocation wins; the grant stays visible as lost. *(recommended)*
- B. The grant wins, as an addition does in RFC 0004's sets.

Recommendation: A. A lost grant is cheap to repeat; a lost removal leaves a stolen device writing.

**M3. Which changes of a revoked member count.**

- A. Those inside the cutoff the revocation names: the subject's changes the revoking admin had seen. *(recommended)*
- B. Those the relay accepted before the revocation (RFC 0007, R7 A).
- C. Those under key generations before the rotation (RFC 0007, R7 B).

Recommendation: A. B depends on the relay's order, which members on an island do not have, and refuses changes the remover had seen. C lets a removed member keep writing under the old generation forever. A needs RFC 0007 to check sealed changes against the cutoff's author sequence, and to let members push again what the relay refused.

**M4. Two admins who revoke each other at the same time.**

- A. The revocation of the more senior admin stands. *(recommended)*
- B. Both stand.
- C. Neither stands.

Recommendation: A. B lets a compromised admin take an honest one down with it. C lets it cancel its own removal.

**M5. Re-adding a removed member.**

- A. Allowed by an add that saw the removal, from a generation newer than the rotation. *(recommended)*
- B. Never; the member needs a new member key.

Recommendation: A. A member key is shared by all spaces of a device, so B costs a new key everywhere for one mistake.

**M6. Concurrent rotations.**

- A. Generation ids of a 24-bit ordinal and a 40-bit random tag, a deterministic current generation, and a merge rotation by the next admin that sees an exposed generation. *(recommended)*
- B. Generation numbers only, and members write with the generation whose key chain entry has the lower hash (RFC 0003's sketch).
- C. A 32-byte generation id in the header, such as the key chain entry's change id.

Recommendation: A. B leaves two keys under one number, so changes under it are ambiguous, and both keys are known to a removed member. C changes RFC 0002's header format. A keeps the format and only gives the number a structure.

**M7. The space id.**

- A. Derived from the creator's member id and a random nonce. *(recommended)*
- B. 128 random bits, as RFC 0002 decided for object ids, and the first genesis a member or the relay sees wins.

Recommendation: A. B lets anyone who learns a space id write a competing genesis. A keeps 128 unpredictable bits and binds them to the creator.

**M8. Where the model id goes (RFC 0005, decision D13).**

- A. In the creator's first sealed change, as an operation on a reserved collection. *(recommended)*
- B. In plain text in the genesis.
- C. In the genesis as `H(space id || model id)`.

Recommendation: A. RFC 0002 promises the relay cannot see the model. B breaks that. C can be reversed by anyone who can guess model ids, which are names chosen by developers. A gives members everything RFC 0005 needs.

**M9. Who signs off an irreversible work order.**

- A. An account with an admin member in the space, other than the performer's account. *(recommended)*
- B. Any account with a member in the space, other than the performer's (RFC 0003).

Recommendation: A. An erasure cannot be undone, and B lets a reader's account approve one.

**M10. Groups.**

- A. Deferred to a later RFC; the SDK expands a group into one entry per member. *(recommended)*
- B. Designed now, as a principal with its own control state.

Recommendation: A, for the reasons in [Groups](#groups). It needs the owner's ruling because GENESIS.md names groups.

**M11. Control bodies and dependencies readable by the relay.**

- A. Readable, so that the relay applies the same rules. *(recommended)*
- B. Encrypted, so that only members apply the rules, and the relay checks only signatures.

Recommendation: A. B lets anyone with a valid key in the space fill its log with changes that every member rejects, and the relay could not stop a removed member. A shows the relay roles, cutoffs and what admins had seen, which [How the relay applies the rules](#how-the-relay-applies-the-rules) lists.

**M12. Public names.** Reader, writer and admin, and the outcome "lost grant", are working names. Recommendation: keep them here and decide the public names in the SDK's API review, as RFC 0004 does for its names.

**M13. A space without an admin.**

- A. Its membership is frozen; content goes on. *(recommended)*
- B. The most senior writer becomes admin automatically.

Recommendation: A. B makes someone admin who was never chosen for it, which the developer would have to explain. The SDK refuses to let the last admin leave, so A arises only from concurrent removals.

## Conflicts with GENESIS.md and the other RFCs

This RFC agrees with GENESIS.md and RFC 0001: the relay checks membership without reading content; every member, an agent included, is visible in the member list with its own key and role; rights are per space; a removed member keeps its copy; and the rules converge in any order. It differs from them, or narrows them, in these points. pm flags each to the owner.

1. **Groups** (GENESIS.md, "Spaces"; RFC 0001, "The design in one page"). Both say groups can be members of many spaces. This RFC defers them and offers SDK-expanded lists instead (M10).
2. **The relay sees roles, cutoffs and control dependencies.** RFC 0002 says the relay cannot see which changes a change builds on. That stays true for sealed changes; for control changes the relay now sees it (M11). RFC 0002's list of what the relay sees, and the FAQ, should add the items in [How the relay applies the rules](#how-the-relay-applies-the-rules).
3. **RFC 0002, header key 4.** The key generation gets a structure: ordinal and tag (M6). The format is unchanged.
4. **RFC 0002, D3.** Space ids are derived from the creator and a nonce, not drawn at random (M7). They keep D3's properties.
5. **RFC 0003, concurrent rotations.** RFC 0003 picks "the generation whose entry has the lower hash". This RFC adds generation ids, the exposed generation and the merge rotation (M6).
6. **RFC 0003, the sign-off.** Narrowed to an account with an admin member (M9).
7. **RFC 0003 and RFC 0007, the person-key entry.** RFC 0003 has it carry a commitment, RFC 0007 a record hash and generation. This RFC carries both, and RFC 0003 and RFC 0007 should point to it.
8. **RFC 0007, decision R7 and retries.** Late changes of a removed member are judged by the cutoff, not by the removal's position (M3), and a member pushes again a change the relay refused when its own fold accepts it.
9. **RFC 0005, decision D13 A.** The model id goes into a sealed change, not the first control entry (M8).
10. **RFC 0004, add wins.** Membership resolves the other way, remove wins (M2). It is not a field type of the schema, so RFC 0004's rule for sets stays as it is.

## Summary of the choices

| Question | This RFC's answer |
| --- | --- |
| How does a space start? | With a genesis by its creator, which names the creator's keys and certificate and the first generation; the space id derives from the creator and a nonce. |
| Which roles exist? | Reader, writer, admin, per space. |
| Who may add and remove members? | Admins. Every member may leave. |
| Who signs off an erasure? | Another account that holds an admin member in the space. |
| What do the control changes carry? | An entry type, the control dependencies, and the fields of their type; the bodies are readable by the relay. |
| How does the membership state converge? | It is a function of the set: one order computed from the changes, revocations first, seniority for cycles, then the rules applied in that order. |
| Concurrent add and remove? | The removal wins; the grant is lost and visible. |
| Two rotations at once? | Two generations with distinct ids; the next admin that sees one exposed rotates again. |
| A removed member that keeps writing? | Its changes beyond the cutoff are rejected by every member and refused by the relay. |
| How does the relay apply the rules? | The same code over the control changes it holds; members decide where it sees less. |
| Groups? | Deferred; the SDK expands a group into one entry per member. |
