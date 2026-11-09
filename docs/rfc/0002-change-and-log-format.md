# RFC 0002: The change and the log format

REQUEST FOR COMMENTS · DRAFT 0.1 · NOVEMBER 2026 · ticket NOC-6

**Changelog.**

- **2026-11-07** · NOC-6 · The owner accepted D1 to D9 and the readings in [Conflicts with GENESIS.md and RFC 0001](#conflicts-with-genesismd-and-rfc-0001).
- **2026-11-07** · NOC-22 · [Amendment 1: the atomic unit and rejected changes](#amendment-1-atomic-unit-and-rejected-changes) settles which unit is atomic, adds the log state `rejected` and says what happens when the relay refuses a change the member already folded. It proposed D10 to D12, which the owner decided as recommended (Carsten, 2026-11-08). The sentences it changes elsewhere are marked *(Amendment 1)*.
- **2026-11-08** · NOC-38 · [Amendment 2: a payload that decrypts but does not decode](#amendment-2-a-payload-that-decrypts-but-does-not-decode) adds the missing row 5a to the step table, for a payload that decrypts but does not decode. It proposed D13, which the owner decided as recommended (Carsten, 2026-11-08). The sentences it changes elsewhere are marked *(Amendment 2)*.
- **2026-11-08** · NOC-44 · [Amendment 3: following RFC 0008](#amendment-3-following-rfc-0008) brings this RFC in line with the owner's decisions in [RFC 0008](0008-membership.md): derived space ids, the structure of the key generation, the control body, equivocations of the genesis and of revoked members, what the relay sees of control changes, and when a member pushes a refused change again. It decides nothing new. The sentences it changes elsewhere are marked *(Amendment 3)*.
- **2026-11-09** · NOC-47 · [Amendment 4: following RFC 0007](#amendment-4-following-rfc-0007) brings this RFC in line with the owner's rulings on [RFC 0007](0007-relay.md): the list of what the relay sees links RFC 0007's, and the relay requires the uploader of a change to be an authenticated current member of the space. It decides nothing new. The sentences it changes elsewhere are marked *(Amendment 4)*.
- **2026-11-09** · NOC-40 · [Amendment 5: the writer's model in the payload](#amendment-5-the-writers-model-in-the-payload) records the payload key that [RFC 0005](0005-schema-and-model-lock.md)'s D10 and D13 decided: every sealed change carries its writer's `model.id` and model version in payload key 64, and a member parks a change of another model, whole. It reopens no decision. The sentences it changes elsewhere are marked *(Amendment 5)*.

This RFC defines what a change is, how it is encoded byte for byte, and how a space's log stores it, on a member's device and on the relay. Every other part of the core builds on it: the merge rules, the signatures, the relay and the fold into the local view.

It builds on [RFC 0001](0001-nocentraldb.md), above all [the design in one page](0001-nocentraldb.md#the-design-in-one-page), and on the principles in [GENESIS.md](../../factory/GENESIS.md). It also answers what the parallel RFC 0003 (keys) and RFC 0004 (merge rules) need from it. Where this RFC disagrees with any of them, the conflict is named in [Conflicts with GENESIS.md and RFC 0001](#conflicts-with-genesismd-and-rfc-0001).

**Status.** Decided. The wire and on-disk formats are reserved for the owner. The owner accepted every recommendation in [Decisions for the owner](#decisions-for-the-owner) and the readings proposed in [Conflicts with GENESIS.md and RFC 0001](#conflicts-with-genesismd-and-rfc-0001) (Carsten, 2026-11-07). The design below describes the decided options. The RFC is final when this pull request is merged. The owner decided [Amendment 1](#amendment-1-atomic-unit-and-rejected-changes) (D10 to D12, NOC-22) and [Amendment 2](#amendment-2-a-payload-that-decrypts-but-does-not-decode) (D13, NOC-38) as recommended, by merging #38 and #54 (Carsten, 2026-11-08); the sentences they mark describe the decided options. [Amendment 3](#amendment-3-following-rfc-0008) decides nothing: it follows the owner's decisions in RFC 0008 (NOC-44). [Amendment 4](#amendment-4-following-rfc-0007) decides nothing either: it follows the owner's rulings on RFC 0007 (NOC-47). [Amendment 5](#amendment-5-the-writers-model-in-the-payload) records the payload key of RFC 0005's decisions D10 and D13 (NOC-40); the owner signs off its number and its encoding by merging it.

## What this RFC does not cover

- **Cryptography and keys.** The signature scheme, the AEAD, the hash function, member ids, person-key ids, space key generations, the suite id and key rotation belong to RFC 0003 (NOC-7). This RFC says only where a "signature", an "AEAD ciphertext" and a "hash" sit in the format, and what each must cover.
- **Merge rules.** How each field type merges concurrent changes belongs to RFC 0004 (NOC-8). This RFC defines the ordering metadata those rules can use, and nothing about the rules themselves.
- **The payloads of rich text and streams.** This RFC leaves room for a Yrs update and for a stream chunk as the value of an operation. It does not design either type.
- **Compaction and the archive, membership changes and work orders.** This RFC reserves their place in the log and in the header. Their formats follow in later RFCs.
- **Code.** Nothing in `crates/` or `packages/` changes. The implementation is NOC-14.

## Terms

| Term | Meaning in this RFC |
| --- | --- |
| Space | The unit of sharing, membership and keys. Every object belongs to exactly one space, and every change belongs to exactly one space. |
| Member | A device, agent or server that holds a key in a space and a full copy of its log. |
| Author | The member whose key signed a change. |
| Change | One signed, encrypted unit that a member appends to a space's log: one local write transaction. |
| Header | The part of a change the relay can read: enough to route, check and store the change. RFC 0003 uses the same word. Not to be confused with RFC 0003's *key envelope*, a space key wrapped for one member. |
| Payload | The part of a change only members can read: what was changed, and the ordering metadata. |
| Operation | One edit inside a payload: one operation on one field of one object. |
| Log | The append-only sequence of changes of one space, on one member or on the relay. |
| Fold | Applying the changes of the log to the local SQLite view. |
| Cursor | A position in the relay's log of a space, from which a member asks for the changes after it. |
| Parked | Kept in the log but not folded, because the client does not understand it yet, and folded after an upgrade; or because another model wrote it, and then kept for good *(Amendment 5)*. A change or a single operation can be parked. |
| Rejected | Kept in the log but not folded, because it failed a check: signature, membership or decryption. Checked again when a control change of its space arrives. *(Amendment 1.)* |

## A change at a glance

A change has three layers. The outer **frame** never changes, so that every client, however old, can store, forward, identify and verify any change. Inside it, the signed **header** carries what the relay needs. Inside the header, the **payload** is an AEAD ciphertext that only members of the space can open.

```text
frame (CBOR array, fixed forever)
├── 0 frame version            uint, 1
├── 1 signed header bytes      bstr: the canonical encoding of the header map
└── 2 signature                bstr: the author's signature (RFC 0003)

header (CBOR map, readable by the relay)
├── 1 space id                 16 bytes
├── 2 author member id         bytes: RFC 0003's member id, a hash of the author's public keys
├── 3 author sequence          uint, 1, 2, 3, ... per author and space
├── 4 key generation           uint: which space key generation sealed the payload (RFC 0003)
├── 5 kind                     uint: 0 sealed, 1 control
├── 6 payload                  bstr: AEAD ciphertext (kind 0), or the control body (kind 1)
├── 7 header format            uint, 1
└── 8 suite id                 uint: the set of cryptographic primitives (RFC 0003)

payload (CBOR map, after decryption; only members can read it)
├── 1 payload format           uint, 1
├── 2 clock                    uint: a hybrid logical clock, 64 bits
├── 3 dependencies             array of change ids: the author's heads when it wrote
├── 4 operations               array of operations, applied in order
├── 5 padding                  bstr of zero bytes, optional
└── 64 model                   map: the writer's model.id and model version (Amendment 5)

operation (CBOR map)
├── 1 object id                16 bytes
├── 2 collection               text, the collection's name in model.lock.json
├── 3 field                    text, the field's name in model.lock.json
├── 4 op                       uint: the operation, as the field type defines it
└── 5 value                    any CBOR value the field type defines,
                               or a sealed personal value: {1: person-key id, 2: ciphertext}
```

The rest of this RFC explains each part and why it looks this way.

## The parts of a change

### Space

Every change names its space in the header (key 1). The relay needs it to route the change, check membership and append it to the right log. A space id is 16 bytes. It is derived from its creator's member id and a random nonce in the space's first change, the genesis, so nobody else can write a first change for it ([RFC 0008, "What it names"](0008-membership.md#what-it-names)). *(Amendment 3.)* A space's id never changes.

### Author and author sequence

The author member id (key 2) names the member that signed the change. It is RFC 0003's member id: the SHA-256 hash of the member's two public keys, in every suite, the id the relay, the member list and the SDK show. The relay needs it to find the signing key, verify the signature and check that the member belongs to the space.

The author sequence (key 3) counts the author's changes in this space: 1 for its first change, then one higher for each next change. It lets a member see at once whether it has every change of an author up to some point. A gap means a change is still missing. The relay already sees how many changes each key writes, so the sequence reveals nothing new to it.

Two different changes with the same author, space and sequence are an **equivocation**: the author's device was restored from a backup, cloned, or is misbehaving. They do not break the log, because a change's identity is its hash and not its sequence (see [Change ids](#change-ids)). Both changes are kept and both are folded. The member marks the author as having equivocated, and the SDK reports it, because it means one member key now writes from two places. This holds for a member whose rights were revoked too: both changes of an equivocation at or below the cutoff its revocation names are accepted and reported, and its changes above the cutoff lose the revoked right ([RFC 0008, "The cutoff"](0008-membership.md#the-cutoff)). The genesis, a space's first change, is the one exception: a space cannot have two, so both stay in the log, the one with the lower change id is the genesis, and the other folds nothing ([RFC 0008, "How the relay and members recognise it"](0008-membership.md#how-the-relay-and-members-recognise-it)). *(Amendment 3.)*

### Key generation, suite and kind

The key generation (key 4) says which generation of the space's key sealed the payload. A member needs it to pick the right key; every ciphertext names exactly one key, as RFC 0003 requires. The generations and their rotation are RFC 0003's. Its value is RFC 0008's generation id: the upper 16 bits are an ordinal, the lower 48 bits a tag derived from the rotation that created the generation, so two concurrent rotations never name the same generation ([RFC 0008, "Generation ids"](0008-membership.md#generation-ids)). The key stays one unsigned integer, so the format does not change. *(Amendment 3.)*

The suite id (key 8) names the set of cryptographic primitives the change uses, so that a future suite, such as a post-quantum one, is a new suite id and not a format break (RFC 0003). Suite 1 is RFC 0003's first suite. A member that does not know a change's suite stores the change and folds nothing from it.

The kind (key 5) has two values:

- **0, sealed.** The payload is an AEAD ciphertext. Content changes and work orders both use this kind, so the relay cannot tell them apart. RFC 0001 promises that the relay sees a work order only as an opaque envelope, and a separate kind would break that.
- **1, control.** The payload is a body the relay must read, because the relay checks membership, serves each member its own key envelopes and deletes shredded person-key records. RFC 0008 defines these bodies: the genesis, membership entries, key envelopes, key chain entries, person-key entries, shred entries and acknowledgements. They are log entries like every other change: signed, identified by their hash, stored and forwarded the same way. A control body is a CBOR map whose key 1 names the entry type and whose key 3 lists its control dependencies; key 2, the clock, is absent, so the relay does not learn when an admin acted ([RFC 0008, "The control bodies"](0008-membership.md#the-control-bodies)). *(Amendment 3.)* A control change carries no content.

Other values are reserved. A member that meets an unknown kind keeps the change and folds nothing from it (see [Versioning](#versioning-old-clients-keep-what-they-do-not-understand)).

### The payload

The payload (header key 6) is the AEAD ciphertext of the canonical encoding of the payload map. RFC 0003 chooses the AEAD and its nonce. This RFC requires two things of it:

- **It binds the payload to its header.** RFC 0003's associated data is every header field except the payload itself: space id, author member id, author sequence, key generation, kind, header format and suite id. A payload therefore cannot be moved to another space, author, sequence number, generation or kind, even by a member that re-signs it.
- **It is signed as ciphertext.** The order is encrypt-then-sign, as RFC 0003 defines it: the signature covers the header, ciphertext included, so the relay can verify a change without any key, and the signature stays valid after a person key is destroyed.

Inside the payload, the ordering metadata comes first (see [Ordering metadata](#ordering-metadata)), then the operations, and last the model that wrote them (key 64, see [Amendment 5](#amendment-5-the-writers-model-in-the-payload)). *(Amendment 5.)*

### Operations: object, field and value

A change carries one or more operations, applied in the order of the array. A change is atomic in its checks: a member accepts all of its operations or none of them, because the signature, the membership check and the decryption cover the change as a whole. The fold has one exception: an operation the member's `model.lock.json` does not know is parked on its own, and the other operations of the change are folded (see [Versioning](#versioning-old-clients-keep-what-they-do-not-understand)). *(Amendment 1, [D10](#the-atomic-unit).)* A change never spans two spaces, so a write transaction cannot span two spaces either, as RFC 0001 says in [What it is not](0001-nocentraldb.md#what-it-is-not).

Each operation names:

- **The object** (key 1), by its 16-byte id. Its space is the change's space.
- **The collection and the field** (keys 2 and 3), by their names in `model.lock.json`. Names are stable, because schema changes are additive and a field is never renamed. Names also let a client that does not know the field yet keep the operation and fold it after an upgrade.
- **The operation** (key 4), a small integer whose meaning the field type defines. RFC 0004 defines it for basic values, sets and links. Value 0 is reserved for "create the object", which takes no field (key 3 is absent) and puts the object into the collection.
- **The value** (key 5), any CBOR value of the profile below. The field type defines its shape.

The format leaves room for the field types that are not designed yet:

- **Rich text.** The value is a byte string holding a Yrs update. A focused edit is a small update, so the change stays small too.
- **Streams.** The value carries one chunk and its index in the stream. Each chunk is its own change, or several chunks share one change, as the stream type will decide.
- **Personal fields.** The value is a sealed personal value: a map of the person-key id (key 1) and the AEAD ciphertext of the field's value under that person key (key 2), inside the payload. RFC 0003 defines the encryption and its associated data (space, object, field and person-key id); the field is named by collection and field name (D5). Destroying the person's key makes that value unreadable, while the signature over the whole change stays valid, because the signature covers the bytes and not the plaintext. This is what [shred and audit](0001-nocentraldb.md#the-design-in-one-page) needs.
- **Custom field types.** A plugin defines its own operation numbers and value shapes, inside the same container.

### What the change does not carry

- **No wall-clock time in the header.** The relay learns when it receives a change, but not when it was written. The clock sits inside the payload.
- **No dependencies in the header.** For a sealed change, the relay cannot see which changes it builds on, so it cannot tell which members saw which changes, or which changes are concurrent. A control change lists the control changes it builds on in its readable body, so for those the relay can tell ([RFC 0008, "How the relay applies the rules"](0008-membership.md#how-the-relay-applies-the-rules)). *(Amendment 3.)*
- **No object, collection or field names in the header.** The relay cannot tell which objects or fields a change touches, or whether two changes touch the same one.

## Ids without a coordinator

No member asks anyone for an id. Every id is generated locally, offline, and still never collides with another member's.

### Object ids

An object id is 16 bytes (128 bits) from the platform's cryptographically secure random generator: `getrandom` natively, `crypto.getRandomValues` in the browser. Space ids are derived instead, as [Space](#space) says (D3, revised for space ids by RFC 0008's decision M7). *(Amendment 3.)*

**Why two members cannot collide.** A collision needs two members to draw the same 128 random bits. Even after 2⁴⁸ (about 280 trillion) objects in one space, the chance that any two of them collide is about one in 8 billion (the birthday bound, n²/2¹²⁹ = 2⁻³³). An id is generated from a secure generator, so no member can predict another's ids, and no member's clock or counter is involved.

A member that wants to create two objects with the same id on purpose can do so: it is the author, and it can write what it likes. The fold then sees two "create" operations for one id and treats them as RFC 0004 defines in "Two creates of one object id": one object, whose collection is a converged value with the other create as a readable conflict. It cannot affect objects in a space it is not a member of.

| Rejected | Why |
| --- | --- |
| UUIDv7 (48-bit time plus random) | Puts the creation time into every id, and ids travel into links, URLs and exports; members learn when every object was made. The index locality it buys matters little at the size of one space. |
| Derived from the creating change (hash of change id and position) | The ids are needed inside the payload before the change exists and is hashed, so the derivation is circular. |
| Derived from author member id and author sequence | Collides after a device is restored from a backup and reuses a sequence number. |
| Sequential ids from the relay | Needs a coordinator, so it breaks island mode. |

### Change ids

A change id is the hash of the frame's first two elements: the frame version, as one byte, and the signed header bytes, with a fixed context string in front so that the hash of a change can never be confused with any other hash in the system:

```text
change id = H("nocentraldb/change-id" || frame version || signed header bytes)
```

H is SHA-256, with 32 bytes of output, as RFC 0003 names it. Change ids use SHA-256 whatever the change's suite, so that a client that does not know a newer suite can still identify the change; a different hash for change ids would need a new frame version. The same holds for the author member id in header key 2: RFC 0003 keeps member ids on SHA-256 in every suite, so an old client can still attribute a change from a suite it does not know. The signature is not part of the id, so the id does not depend on the signature scheme, and a scheme with randomised signatures would not change it.

**Why two members cannot collide.** Two different changes have different header bytes, because their author, sequence, payload ciphertext or space differs. Two equal ids then need a collision of the hash function, which a 32-byte cryptographic hash makes infeasible. The id is computed from the bytes, so every member and the relay compute the same id for the same change, without being told. A member that receives a change twice, from the relay and from a peer in island mode, recognises it as the same change.

The id does not change when a member forwards a change, and it identifies the exact bytes: a member can check that the change it holds is the change another member refers to.

### Operation ids

An operation is identified by its change's id and its position in the change's operation array, counted from 0. Field types use it where they need to name one operation, for example as the tag of a set addition (RFC 0004). It is unique because change ids are, and every member computes it the same way.

| Rejected | Why |
| --- | --- |
| Author member id plus author sequence | Not unique when a device is restored from a backup and writes again under a sequence number it already used. |
| A random id chosen by the author | Does not bind the id to the content, so two different changes can claim one id, and a member cannot check a reference by recomputing it. |
| An id assigned by the relay | Needs the relay, so it breaks island mode, and the relay could hand out the same id twice. |

## Ordering metadata

The merge rules of RFC 0004 need two things from every change: a total order that every member computes the same way, for "the last writer wins", and the causal order, to tell an edit that saw another edit from an edit made at the same time without it. The payload carries both.

### The clock

Every sealed payload carries a **hybrid logical clock** (key 2): one unsigned 64-bit integer. The upper 48 bits hold milliseconds since the Unix epoch, which lasts until the year 10889. The lower 16 bits hold a counter. A control body carries no clock (see [Key generation, suite and kind](#key-generation-suite-and-kind)). *(Amendment 3.)*

When a member writes, it sets the clock to the larger of its own wall clock and one more than the highest clock it has seen in the space. When it receives a change, it raises its "highest seen" to that change's clock. The clock therefore never runs backwards, and a change always has a higher clock than every change its author had seen. When the counter would overflow, the member waits for its wall clock to reach the next millisecond.

The clock is the **stamp** that RFC 0004 orders writes by. RFC 0004 defines the full order and its tie-breaks (on equal clocks, the author member id and then the operation id, as bytes). This RFC guarantees what that order needs: every operation has an id no other operation shares, every member reads the same clock, author member id and operation id from the same change, and a change's clock is higher than the clock of every change its author had seen.

### Dependencies

Every sealed payload lists its **dependencies** (key 3): the change ids of the space's heads in the author's log at the moment it wrote. A control body uses key 3 for its control dependencies, the heads among the control changes only, readable by the relay ([RFC 0008, "The control bodies"](0008-membership.md#the-control-bodies)). *(Amendment 3.)* A head is a change that no other change in the author's log depends on yet. The list is sorted by bytes and holds no id twice. It always covers the author's own previous change in the space, directly or through another dependency.

The dependencies make the changes of a space a directed acyclic graph. A change A happened before a change B when B depends on A, directly or through other changes. Two changes are **concurrent** when neither happened before the other. A member computes this from its own copy of the log, without asking anyone. Usually the list holds one or two ids. It grows only while several members write without seeing each other, and shrinks again with the first change that sees them all.

**Causal delivery is not guaranteed.** The relay hands out changes in the order it received them, which can put a change before its dependencies, for example after a member forwards changes it received in island mode, and compaction removes old changes from the log. A member folds every change as soon as it arrives, whether or not it holds the change's dependencies, and the field types of RFC 0004 converge in any order of arrival. A dependency the member does not hold is a **gap**: the SDK reports it, and the member asks the relay or a peer for it. A member never waits for a gap to close, because a gap that never closes, for example when the relay withholds a change, would otherwise stop the member's whole space.

### A late change stays identifiable

Principle 3 of GENESIS.md asks that a change which arrives late and loses to a newer change stays identifiable as such. With the clock and the dependencies, every member identifies it the same way, whatever order the changes arrived in:

- When one change overwrites another and **saw it** (the winner depends on the loser), the loser was simply overwritten. Its author's intent was replaced knowingly.
- When the winner **did not see** the loser (the two are concurrent), the loser is a **concurrent loser** (RFC 0004 calls this outcome *lost*). The fold records it as such, next to the field, and the SDK can show it. The change stays in the log; nothing is dropped.

How each field type tells the two cases apart is RFC 0004's: a write may name the writes it replaces, and the dependencies give every field type the causal order of the whole space.

The classic late change is a laptop that edits a field while offline for weeks. When it reconnects, its change is concurrent with every change made in the meantime. If a newer change wins, the laptop's change is a concurrent loser. If the laptop's clock was ahead and its change wins instead, the changes it overwrote without seeing them are the concurrent losers. Both cases stay identifiable on every member.

"Concurrent loser" depends only on the changes, never on when they arrived, so every member computes the same set, and convergence (principle 2) holds for it too. How long a change took to arrive is still visible as a diagnostic: its clock against the time the relay received it. That diagnostic depends on the relay and is not part of the converged state.

**What the clock does not do.** A member whose wall clock runs ahead writes changes with later clocks, so for "the last writer wins" its writes win until the other clocks catch up, and every member's clock follows it forward. Convergence holds, but "last" then means "latest by that member's clock". The core warns when a received clock is more than a minute ahead of the local wall clock. It does not reject the change, because rejecting it on some members and not on others would break convergence. Nor does the writing core refuse to create a change: it cannot tell that its own clock is wrong, only that it disagrees with another member's. This answers RFC 0004's question whether far-future stamps are refused: they are not, and the limit is stated here and there.

| Rejected | Why |
| --- | --- |
| Lamport clocks alone | Converge, but "last" then means "most causally advanced", not "latest in time": a member that wrote a lot wins over a member that wrote later. Users expect the later edit to win. |
| Version vectors on every change | Grow with every key that ever wrote in the space, and devices, agents and ad-hoc sub-agent spaces add keys all the time. The dependency list stays at one or two ids in the common case, and a member can still compute a version vector locally when a field type needs one. |
| Wall-clock time alone | Two writes in the same millisecond tie, and a clock that jumps backwards makes a member's own later write lose to its earlier one. |
| The relay's order as the total order | Differs from the order peers see in island mode, so members would not converge. |

## The canonical encoding

Every part of a change has exactly one byte representation. Two members that encode the same change produce the same bytes, natively and in WebAssembly alike, so the same change has the same id and the same signature everywhere (principle 7).

### The CBOR profile

Changes are encoded as CBOR ([RFC 8949](https://www.rfc-editor.org/rfc/rfc8949)) with the core deterministic encoding of §4.2.1, restricted further:

- **Lengths are definite and minimal.** No indefinite-length items. Every integer and every length uses its shortest form.
- **Maps have unsigned integer keys only**, sorted by the bytes of their encoding, each key at most once. Values of a field type may use text keys, sorted the same way.
- **Allowed major types:** unsigned and negative integers, byte strings, text strings, arrays, maps, and the simple values `false`, `true` and `null`.
- **Floating-point numbers** use the shortest of the 16-, 32- and 64-bit forms that keeps the value exactly, as §4.2.1 says. Every NaN is encoded as the single quiet NaN `0xf97e00`. Negative zero stays negative zero. The header and the ordering metadata never contain a float.
- **Text strings** are valid UTF-8, stored as given. The format does not normalise Unicode. A field type or the SDK may normalise before writing (RFC 0004 has the SDK normalise strings to NFC), but the encoding never changes the bytes it is given.
- **No tags**, and no simple values other than the three above. Future versions may allow specific tags.
- **Integers** stay within 64 bits.

A decoder accepts only canonical bytes. After decoding, the core encodes the value again and compares the bytes. A change that is not canonical is rejected, and never "repaired", because a repaired change would have another id and an invalid signature. A payload that is not canonical is never repaired either; the member keeps the frame and parks the change (step 5a of [Amendment 2](#amendment-2-a-payload-that-decrypts-but-does-not-decode)). *(Amendment 2.)* The core keeps the exact bytes it received and never re-encodes a stored change.

The implementation (NOC-14) commits test vectors: changes as bytes, with their ids, checked by the native core and by the WebAssembly build alike.

| Rejected | Why |
| --- | --- |
| postcard | Compact and fast, but not self-describing: a reader needs the exact schema to skip a field. An old client could not skip a field it does not know, so it could not keep data it does not understand yet. Its encoding also follows Rust types, which the TypeScript side would have to mirror by hand. |
| A hand-written binary format | Needs its own specification, its own encoders in two languages and its own decoder hardening, all for what deterministic CBOR already gives. The risk of a subtle difference between the two encoders is the very risk this RFC must avoid. |
| JSON with a canonical form (RFC 8785) | No byte strings, so ciphertexts, ids and Yrs updates need base64 and grow by a third. Number handling differs between JavaScript and Rust. |
| Protocol Buffers | Its encoding is not deterministic across implementations, by its own documentation. |

### Versioning: old clients keep what they do not understand

The format changes only by adding. Each layer carries its own version, and an old client knows what to do with every layer it cannot read:

- **The frame never changes.** Its version stays 1. Every client can therefore store, forward and identify any change, and verify its signature, as long as it knows the change's suite (RFC 0003).
- **Header keys 1 to 8 keep their meaning forever.** A new header format may add keys. Keys 1 to 63 are "must understand": a client that meets one it does not know stores the change but does not fold it. Keys from 64 up are "may ignore": a client that does not know them folds the change as if they were absent. The same rule holds for the payload map. In the operation map, an unknown "must understand" key parks only that operation, as an unknown field does. *(Amendment 1, [D10](#the-atomic-unit).)*
- **A higher header or payload format number** than the client knows means "stored, not folded".
- **A payload that decrypts but does not decode** is stored, not folded, with the reason "unknown payload format", because a member cannot tell a newer payload format from a malformed one ([D13](#amendment-2-a-payload-that-decrypts-but-does-not-decode)). *(Amendment 2.)*
- **An operation on a collection or field the client's `model.lock.json` does not know** is stored with the change, and the rest of the change is folded. When the client upgrades and the field becomes known, the core folds the stored operations, in the total order, as if they had just arrived. This is the one exception to the atomic change (see [Operations](#operations-object-field-and-value)).
- **A change of another model** is stored, not folded, with the reason "another model": its payload key 64 names a `model.id` other than the one of the member's `model.lock.json`. A change without key 64 is folded as before, because the key is "may ignore" ([Amendment 5](#amendment-5-the-writers-model-in-the-payload)). *(Amendment 5.)*

A change that a client keeps without folding is **parked**, and so is a single operation it keeps without folding. Parked changes stay in the log, are forwarded and are counted. The SDK reports how many changes and operations are parked and why, so that "keep data you do not understand" never turns into "silently ignore it" (principle 3). Parked waits for an upgrade of the client; a change of another model stays parked after every upgrade, because a client's `model.id` never changes. *(Amendment 5.)* A change that failed a check is not parked but **rejected**, and waits for a control change instead (see [Rejected changes](#rejected-changes)). *(Amendment 1.)*

## The log on disk

### On a member

Each member keeps one log per space it belongs to. The log is append-only: the core adds entries and never changes or deletes them. The only exception is compaction, a work order that moves entries into a signed archive (a later RFC).

The log lives in the same SQLite database as the local view, in one table per database:

```text
log
├── local position   integer, 1, 2, 3, ... in the order this member appended
├── space id         16 bytes
├── change id        32 bytes, unique per space
├── relay position   integer, or null while the relay has not confirmed the change
├── relay refusal    the relay's error code, or null              (Amendment 1)
├── state            folded, parked or rejected                   (Amendment 1)
├── reason           why parked or rejected, or null              (Amendment 1)
└── frame            the exact bytes of the change, as received or written
```

Appending a change and folding it happen in one SQLite transaction, so after a crash the log and the view always agree: a change is in both or in neither. A parked or rejected change is appended in the same way, without a fold. Every other table, such as an index from author and sequence to change id, the list of gaps, or the list of parked operations of a folded change, can be rebuilt from the frames. The frame is the only source of truth.

The state of an entry is local and may change: parked becomes folded after an upgrade, and rejected becomes folded when the control change it lacked arrives (see [Rejected changes](#rejected-changes)). The state and the reason follow from the frames the member holds and its `model.lock.json`, never from the relay's answer. The frame never changes. *(Amendment 1.)*

A change the member writes itself enters the log at once, folded and without a relay position. It is in the member's view before it leaves the device, and the member keeps working without a relay (island mode). When the relay accepts it, the member records the relay position. When the relay refuses it, the member records the refusal and the change stays folded (see [A change the relay refuses after the member folded it](#a-change-the-relay-refuses-after-the-member-folded-it)). *(Amendment 1.)*

| Rejected | Why |
| --- | --- |
| Separate append-only segment files next to the SQLite view | Needs two kinds of storage to agree after a crash, and the browser has no portable append-only file. One SQLite database works the same natively and in the browser (principle 7). |
| Storing the decrypted payload instead of the frame | The signature and the id need the exact ciphertext. A copy of decrypted content would also be one more copy that erasure has to reach. |

### On the relay

The relay keeps one log per space. Each entry holds:

- **The relay position**, a dense integer per space: 1, 2, 3, ..., without gaps, in the order the relay accepted the changes.
- **The change id**, which the relay computes from the frame. A change the relay already holds is not appended a second time.
- **The time of acceptance**, from the relay's clock.
- **The frame**, as the relay received it.

The relay accepts a change when the frame is well-formed and canonical, the signature is valid for the author member's signing key, and RFC 0008's membership rules accept it over the control changes the relay holds. A removed or demoted member's changes above the cutoff its revocation names are refused, and those at or below it are accepted, whenever they arrive ([RFC 0008, "How the relay applies the rules"](0008-membership.md#how-the-relay-applies-the-rules)). *(Amendment 3.)* The author may be any member, and the uploader must be an authenticated current member of the space ([RFC 0007, "What the relay checks"](0007-relay.md#what-the-relay-checks)), so any current member may forward another member's changes, which island mode needs. *(Amendment 4.)* The relay never decrypts, folds or reorders anything.

**The cursor** is a relay position. A member asks the relay for "the changes of this space after position p", and the relay answers in position order, a page at a time. Positions are dense, so a member sees at once whether a page is complete. A cursor belongs to one relay: if a space moves to another relay, members start from position 0 there, and recognise the changes they already hold by their ids.

The relay's log is a transport, not the truth. Every member holds the same changes, and the members, not the relay, decide what the space contains.

| Rejected | Why |
| --- | --- |
| Timestamps as cursors | Two changes accepted in the same instant would be ambiguous, and a clock that moves backwards would skip changes. |
| A cursor made of the member's version vector | Needs the relay to read dependencies, which are encrypted on purpose. |
| Positions with gaps (for example from a database sequence) | A member could not tell a gap from a lost change. |

## What the relay can see, and what it cannot

The relay holds no keys. Here is what it learns anyway from the changes and the log, stated plainly (principle 4). [RFC 0007, "What the relay sees"](0007-relay.md#what-the-relay-sees) adds what the relay learns from its protocol, such as when each member is online, which client it runs and how far behind its cursor is, and separates all of it from what the server the relay runs on sees. *(Amendment 4.)*

The relay **can see**:

- **Which members are in which space**, from the control changes it checks, and when the membership and the key generations change.
- **The key distribution.** That a key envelope exists for each member id and generation, and the key chain entries. It cannot open them.
- **Erasures.** When a shred entry destroys a person key, and that person key's id, but not whose key it is or what it protected. The acknowledgements show which members confirmed it.
- **Who writes, when and how much.** For each change: the space, the author member id, the author's sequence number, the suite id, the key generation, the time the relay received it, and the size of the ciphertext. The size reveals roughly how much was written. Padding (see [Decisions for the owner](#decisions-for-the-owner), D8) blurs it but does not hide it.
- **The rhythm of work.** A burst of small changes looks like typing; one large change after days of silence looks like a member coming back from offline work.
- **Who reads what, and from where.** Which connection asks for which space from which cursor, and the network addresses of members, unless they connect through something that hides them.
- **The ids of changes**, and that a change was uploaded by a member other than its author, when that happens.
- **What the control changes say**, because the relay applies the membership rules over them: each member's role and when it changes; the cutoff of each revocation, so how many of the subject's changes the revoking admin had received; the control dependencies, so which control changes each admin had seen when it wrote and which admins acted on separate islands; the account key in each member certificate, which links one person's or organisation's devices and agents in every space on that relay; which member is a sub-agent of which; which accounts sign off erasures; and the generation graph, so when concurrent rotations happened and which merge rotations followed ([RFC 0008, "How the relay applies the rules"](0008-membership.md#how-the-relay-applies-the-rules)). *(Amendment 3.)*

The relay **cannot see**:

- **Any content**: object ids, collection and field names, operations, values, rich text and stream chunks, personal fields.
- **The ordering metadata of sealed changes**: when a sealed change was written by its author's clock, which changes it depends on, and which sealed changes are concurrent. Control changes carry no clock, and their control dependencies are readable (see above). *(Amendment 3.)*
- **Whether a sealed change is content or a work order.**
- **The model**: which collections and fields a space uses, and which schema version a member runs. The genesis carries the space's model id sealed, so the relay cannot read it either ([RFC 0008, "Genesis"](0008-membership.md#genesis)). *(Amendment 3.)* Each sealed change carries its writer's model id and model version inside the payload (key 64), so the relay cannot read those either. *(Amendment 5.)*

What the relay **can do** despite holding no keys, and what limits it:

- **It cannot forge or alter a change.** Every change is signed, and the signature covers the ciphertext.
- **It cannot move a payload** to another space, author or sequence number, because the AEAD binds the payload to its header.
- **It can withhold changes.** A member notices a withheld change when a later change depends on it, or when an author's sequence numbers have a gap. A member does not notice a withheld change that nothing depends on and that is its author's last.
- **It can show different members different subsets** of the log. Members notice it only when they exchange changes directly, as in island mode.
- **It can delete its log or refuse service.** Every member still holds its full copy, and the members can repopulate a new relay.

## What RFC 0003 and RFC 0004 need from this RFC

RFC 0003 (keys and cryptography, NOC-7) and RFC 0004 (merge rules, NOC-8) are written in parallel and depend on this RFC. This section answers each of their requests.

### RFC 0004: merge rules

| Request | Answer in this RFC |
| --- | --- |
| **O1.** Every change has an id that no two changes share, and every member computes the same id for the same change. | Met. The change id is the hash of the frame version and the signed header bytes ([Change ids](#change-ids)). An operation id is the change id plus the operation's position ([Operation ids](#operation-ids)). |
| **O2.** The stamp is computed from the change alone and compares the same way on every member and platform. | Met. The stamp is the hybrid logical clock in the payload, an unsigned 64-bit integer compared as a number. The author member id and the operation id for the tie-breaks are in the same change. No float and no platform-dependent value is involved. |
| **O3.** If the author of change b had seen change a, a's stamp is lower than b's. | Met. A member's clock is always higher than the clock of every change it has seen ([The clock](#the-clock)). |
| **O4.** The stamp carries wall-clock time. | Met. The upper 48 bits are milliseconds since the Unix epoch. |
| **O5.** One canonical encoding per value, NaN and negative zero included. | Met. Every NaN is `0xf97e00`; negative zero is encoded as negative zero, positive zero as positive zero, each in its one shortest form ([The CBOR profile](#the-cbor-profile)). RFC 0004 asks for one encoding each, not for the two to be equal, so a set may hold both 0.0 and −0.0. |
| Are stamps far in the future refused? | No. Neither the writing member nor a receiving member nor the relay refuses them; members warn. Refusing on some members and not on others would break convergence, and the relay cannot see the clock (D6). |
| Causal delivery is not guaranteed (RFC 0004, decision 6). | Confirmed. Members fold changes as they arrive, in any order, and report missing dependencies as gaps ([Dependencies](#dependencies)). The dependencies give the causal order for diagnostics and for field types that want it; no field type may rely on them having arrived. |

### RFC 0003: keys and cryptography

| Request | Answer in this RFC |
| --- | --- |
| Room for the suite id. | Header key 8, signed, readable by the relay, covered by the associated data. |
| Room for the space key generation. | Header key 4, the key generation. |
| The person-key id on each personal field. | A personal field's value is a sealed personal value: the person-key id and the ciphertext ([Operations](#operations-object-field-and-value)). |
| The author's member id, a hash of its public keys. | Header key 2. |
| Encrypt-then-sign: the signature covers header and ciphertext, so the relay verifies without decrypting and signatures survive a shred. | Met. The signature covers the signed header bytes, which contain the ciphertext. The change id is computed from the same bytes, so a shred changes neither the id nor the signature. |
| Key envelopes, membership entries and shred entries are log entries too. | Met. They are control changes (kind 1), with the same frame, signature, id and log as every other change. RFC 0003 defines their bodies. |

## Decisions for the owner

These are the decisions this RFC needed from the owner. Each lists the options and a recommendation. The owner accepted every recommendation (Carsten, 2026-11-07), and the design above describes them. [Amendment 1](#amendment-1-atomic-unit-and-rejected-changes) adds D10 to D12, and [Amendment 2](#amendment-2-a-payload-that-decrypts-but-does-not-decode) adds D13; the owner decided all four as recommended (Carsten, 2026-11-08).

**D1. Ordering metadata.**

- A. Hybrid logical clock plus a list of dependencies. *(recommended)*
- B. Lamport clock plus a list of dependencies.
- C. Hybrid logical clock plus a version vector.

Decided: A (Carsten, 2026-11-07). Recommendation: A. "The last writer wins" matches what users expect in time, and the dependencies give the causal order at a size that does not grow with the number of keys.

**D2. Change ids.**

- A. The hash of the signed header bytes. *(recommended)*
- B. Author member id plus author sequence.
- C. A random id.

Decided: A (Carsten, 2026-11-07). Recommendation: A. It is unique without a coordinator, every party computes it independently, and a backup restore cannot make two changes share it.

**D3. Object and space ids.**

- A. 128 random bits from a secure generator. *(recommended)*
- B. UUIDv7: 48 bits of creation time and 74 random bits.

Decided: A (Carsten, 2026-11-07). Recommendation: A. It leaks nothing about when an object was made into links, URLs and exports.

Superseded for space ids by RFC 0008's decision M7 (Carsten, 2026-11-08): a space id is derived from its creator's member id and a random nonce, because a random space id does not bind the space to its creator ([RFC 0008, "What it names"](0008-membership.md#what-it-names)). Object ids keep D3 A. *(Amendment 3.)*

**D4. Encoding.**

- A. CBOR with the deterministic profile above. *(recommended)*
- B. postcard.
- C. A hand-written format.

Decided: A (Carsten, 2026-11-07). Recommendation: A. It is deterministic, self-describing (so old clients can keep what they do not understand), and has maintained libraries in Rust and TypeScript.

**D5. How an operation names its field.**

- A. The collection's and field's names from `model.lock.json`. *(recommended)*
- B. Numeric ids that `ncdb model build` assigns.

Decided: A (Carsten, 2026-11-07). Recommendation: A. Names never change, need no id registry that two developers could fill differently, and an old client can tell which unknown field an operation is for. They cost a few bytes per operation, inside the ciphertext.

**D6. Where the clock sits.**

- A. Inside the payload, invisible to the relay. *(recommended)*
- B. In the header, so that the relay can refuse changes whose clock is far ahead of its own.

Decided: A (Carsten, 2026-11-07). Recommendation: A. B would show the relay when every change was written, offline work included. It would also protect only changes that pass through the relay, not those exchanged in island mode.

**D7. Where a member stores its log.**

- A. A table in the SQLite database of the local view. *(recommended)*
- B. Separate append-only segment files.

Decided: A (Carsten, 2026-11-07). Recommendation: A. Append and fold in one transaction, and the same storage natively and in the browser.

**D8. Padding the payload.**

- A. Pad every payload to a multiple of 256 bytes. *(recommended)*
- B. No padding.
- C. Pad to the next power of two.

Decided: A (Carsten, 2026-11-07). Recommendation: A. It hides the exact length of short values, such as "yes" against "no", at a cost of 128 bytes per change on average. C hides more but doubles the size of large changes such as rich-text pastes. No padding hides the length of a large write. The format allows changing the rule later, because padding is a payload field the reader ignores.

**D9. Changes whose meaning a client does not know.**

- A. Store them as parked, report them in the SDK, and fold them after an upgrade. *(recommended)*
- B. Refuse to sync a space until the client is upgraded.

Decided: A (Carsten, 2026-11-07). Recommendation: A. It is what GENESIS.md asks: old clients keep data they do not understand yet. It needs the clarification in the next section.

## Conflicts with GENESIS.md and RFC 0001

This RFC agrees with GENESIS.md, RFC 0001, RFC 0003 and RFC 0004 except in the points below. The owner accepted the reading or proposal of each (Carsten, 2026-11-07). RFC 0003 and RFC 0004 have adopted this RFC's terms, the whole-header associated data and SHA-256, and RFC 0004 defines two creates of one object id, so no conflict with either remains.

1. **Convergence and parked changes.** GENESIS.md's principle 2 says the same set of changes yields the same state on every member. A client that parks a change it does not understand (D9) holds the same changes as an upgraded client but shows a different state. This RFC reads principle 2 as "every member that understands all the changes it holds ends in the same state", and makes the parked changes visible. The owner confirmed this reading together with D9 A (Carsten, 2026-11-07).
2. **What the relay sees is more than RFC 0001 lists.** RFC 0001, GENESIS.md and the FAQ say the relay sees "which key is in which space, and when and how much each key writes". [What the relay can see](#what-the-relay-can-see-and-what-it-cannot) adds the change sizes, the key generations and their timing, the rhythm of work, who reads which space from where, and that the relay can withhold changes or show members different subsets. None of it contradicts RFC 0001, but a reader of RFC 0001 alone could infer more than the design delivers, which principle 4 forbids. We propose that RFC 0001 and the FAQ link to that section. The owner agreed (Carsten, 2026-11-07); pm's follow-up ticket makes that change to RFC 0001 and the FAQ.
3. **"Envelope" means two things.** RFC 0001 says the relay sees a work order as "an opaque envelope", and RFC 0003 calls a wrapped space key a "key envelope". This RFC therefore calls the relay-readable part of a change its **header**, as RFC 0003 does, and uses "envelope" only for RFC 0003's key envelopes.

## Amendment 1: atomic unit and rejected changes

Added on 2026-11-07 for NOC-22, after this RFC was merged. pm's review of RFC 0003 (keys), RFC 0005 (schema), RFC 0006 (the local view) and RFC 0007 (the relay) found two gaps in this RFC:

1. **Two readings of "atomic".** [Operations](#operations-object-field-and-value) said a member folds all operations of a change or none. [Versioning](#versioning-old-clients-keep-what-they-do-not-understand) said an operation on an unknown collection or field is stored, and the rest of the change is folded. RFC 0005 and RFC 0006 build on the second reading.
2. **No state for a change that fails a check.** The member log knew only `folded` and `parked`. RFC 0003 keeps a change that fails a check "marked as rejected". RFC 0007 does the same for a change the relay refuses after the member already folded it. RFC 0006 could not say what the view does with either.

The log is an on-disk format, so the owner decided: D10 to D12 as recommended, by merging #38 (Carsten, 2026-11-08). The sentences this amendment changes elsewhere in the RFC are marked *(Amendment 1)* and describe the decided options. Nothing else in the RFC changes.

### What a member does with a change it receives

A member takes every change through the same steps, in this order. A change stops at the first step it does not pass, and its state says where it stopped.

| Step | Fails when | State, and its reason |
| --- | --- | --- |
| 1. Decode | The frame is not well-formed or not canonical ([The CBOR profile](#the-cbor-profile)). | Not appended: without canonical bytes there is no stable change id. The member reports it, with the relay or peer that sent it. |
| 2. Understand the header | The suite, the kind, the header format or a "must understand" header key is unknown. | Parked: unknown suite, kind or format. |
| 3. Verify the signature | The author's signing key is not known yet, because no membership entry the member holds names the author; or the signature does not verify. | Rejected: unknown author, or bad signature. |
| 4. Check membership | The control changes the member holds do not give the author the right to write the change under its key generation and author sequence ([RFC 0008, "Sealed changes"](0008-membership.md#sealed-changes)). *(Amendment 3.)* | Rejected: not a member. |
| 5. Decrypt | The member holds no key envelope for the change's key generation; or the AEAD fails. | Rejected: no key, or undecryptable. |
| 5a. Decode the payload | The payload decrypts, but the plaintext is not a payload map this core can read: a value nests deeper than the decoder's limit, the bytes are not canonical, a key that payload format 1 requires is missing or has the wrong type, or it is a newer payload format that format 1's decoder cannot read. | Parked: unknown payload format. *(Amendment 2, [D13](#amendment-2-a-payload-that-decrypts-but-does-not-decode).)* |
| 6. Understand the payload | The payload format or a "must understand" payload key is unknown; or payload key 64 names a `model.id` other than the one of the member's `model.lock.json`, or does not hold a model id and a model version ([Amendment 5](#amendment-5-the-writers-model-in-the-payload)). | Parked: unknown format; or parked: another model. *(Amendment 5.)* |
| 7. Fold, operation by operation | An operation names a collection or field the member's `model.lock.json` does not know, or carries an unknown "must understand" key. | The change is folded; that operation is parked, with its reason. |

Steps 2 to 6, step 5a included, decide for the change as a whole. *(Amendment 2.)* Step 7 decides per operation. A control change (kind 1) stops after step 4: it has no payload to decrypt, and RFC 0003 and the membership RFC say what it does.

### The atomic unit

The change is the atomic unit of everything that checks it: the signature, the membership and the decryption cover the change as a whole, so a member accepts all of its operations or none of them. The one exception is the fold of an operation the member's model does not know yet: that operation is parked on its own, the others are folded, and the parked one is folded after an upgrade, in the total order, as if it had just arrived.

**What this does not do.** An old client can show part of one write transaction. When a newer app sets `title` and a new field `dueDate` in one transaction, an old client shows the new title without the due date, because it does not know the field. An invariant of the app that spans a known and an unknown field does not hold on the old client until it upgrades. After the upgrade it reaches the same state as every other client, because each field's state is a function of the set of its operations (RFC 0004). This is the reading of principle 2 the owner already accepted for parked changes ([Conflicts](#conflicts-with-genesismd-and-rfc-0001), point 1).

**D10. The atomic unit.**

- A. The change is atomic for every check; an operation the client's model does not know is parked on its own and the rest of the change is folded. *(recommended)*
- B. The change is atomic for everything: one unknown operation parks the whole change.

Decided: A (Carsten, 2026-11-08). Recommendation: A. Under B, every change a newer app writes to a new field together with known ones would hide the known edits from old clients too, so old clients would fall further behind with every schema addition. RFC 0005 relies on A to keep old clients working for weeks. A cannot weaken a check, because no check can pass for part of a change.

### Rejected changes

A change is **rejected** when it failed step 3, 4 or 5: it is kept in the log, with its reason, and not folded. Rejected is distinct from parked. A parked change waits for an upgrade of the client, which teaches it what the change means. A rejected change waits for a control change of its space, which gives it the author's keys, the membership or the key it lacked, or it waits forever.

**When a rejected change is checked again.** Whenever a member appends a control change of a space (a membership entry, a key envelope or a key chain entry, RFC 0003), it checks the rejected changes of that space again, from step 3, in the same transaction. A change that now passes is folded as if it had just arrived. The result of each check depends only on the change and the control changes the member holds, never on the order in which they arrived or on the time. So a change that arrives before its author's membership entry or before its key envelope is folded as soon as that entry arrives, and two members that hold the same changes hold the same rejected changes and the same view (principle 2). A bad signature or an undecryptable payload under a key the member holds never passes later; the core may skip those when it checks again.

**A change that passed and fails later.** A control change that arrives later can make a passed check fail, for example a removal whose cutoff lies below changes the member already folded. RFC 0008 lists every way this can happen ([What a new change can change](0008-membership.md#what-a-new-change-can-change)). *(Amendment 3.)* The change then moves from folded to rejected in the transaction that appends the control change, and the fields it touched are recomputed from their remaining operations. The view then equals the view of a member that never folded the change.

**What a rejected change keeps.** It stays in the log, counts towards its author's sequence and its gaps, and is forwarded like every other change, because a member that holds more control changes may accept it. The SDK reports how many changes are rejected in each space and why. A rejected change is never dropped (principle 3) and never folded "for now".

**D11. A state for changes that fail a check.**

- A. A third log state, rejected, with a reason. A change that lacks a membership entry or a key envelope is rejected too, and every rejected change is checked again when a control change of its space arrives. *(recommended)*
- B. Rejected only for checks that can never pass (bad signature, undecryptable). A change that lacks a membership entry or a key envelope is parked, with the reason "waiting".
- C. No new state: a change that fails a check is dropped and reported.

Decided: A (Carsten, 2026-11-08). Recommendation: A. Each state then has one trigger: an upgrade for parked, a control change for rejected. B needs a member to tell "not yet" from "never", and it cannot: an author whose membership entry has not arrived looks exactly like a stranger. C loses changes that arrive before their key envelope, which in island mode is common, and two members would end in different states depending on the order of arrival (principles 2 and 3).

### A change the relay refuses after the member folded it

A member folds its own change before it sends it (see [On a member](#on-a-member)). The relay may then refuse it (RFC 0007): most often because the author was removed from the space while it worked offline, or because a control change breaks the membership rules.

The relay's refusal does not change the member's log state. The member records the relay's error code in the entry's `relay refusal`, keeps the change folded, and the SDK reports it to the app as refused by the relay. The member does not push that change to that relay again, with one exception: when a control change arrives and the member's own fold newly accepts the change, it pushes it to that relay once more, because a concurrent change may have made the revocation behind the refusal ineffective (RFC 0008, decision M19, in [How the relay applies the rules](0008-membership.md#how-the-relay-applies-the-rules)). *(Amendment 3.)* It then checks the change again from step 3 with the control changes it holds: if its own check fails, for example because it now holds the membership entry that removed its author, the change becomes rejected like any other change; otherwise it stays folded.

**What this does not do.** Members that sync only through the relay never receive a refused change. Its author sees the write in its view, and they do not. A member that receives it directly from a peer, in island mode, folds it if its own checks pass. The SDK tells the author's app, which is the only place that can tell its user. The core never undoes the write on its own; the app can write a new change that reverts it.

**D12. A folded change the relay refuses.**

- A. It stays folded; the member records and reports the refusal, and only the member's own check can make it rejected. *(recommended)*
- B. It becomes rejected, and the member recomputes its view without it.
- C. The member keeps pushing it, until the relay accepts it.

Decided: A (Carsten, 2026-11-08). Recommendation: A. Under B, the state would depend on the relay's answer, which no other member sees: a peer that received the change directly keeps it folded, so two members holding the same changes would differ (principle 2), and the relay could make a member drop any of its own changes by refusing it. C never ends for a removed author. RFC 0008's decision M19 adds the one exception described above: a member pushes a refused change again once its own fold newly accepts it ([RFC 0008, "How the relay applies the rules"](0008-membership.md#how-the-relay-applies-the-rules)). *(Amendment 3.)*

### What this means for the RFCs built on this one

These RFCs are open pull requests. pm aligns them once the owner has decided; this amendment changes none of them.

- **RFC 0003 (keys).** "Kept and marked as rejected" is the state rejected, as D11 defines it.
- **RFC 0005 (schema).** Its parking of single operations is D10 A.
- **RFC 0006 (the local view).** Its fold step 2 ends in rejected. Its step 3 ends in rejected with the reason "no key" when the key envelope is missing, not in parked; an unknown suite or format stays parked. Its step 4 is step 7 above.
- **RFC 0007 (the relay).** For `bad_signature`, `author_not_member` and `membership_rule`, "keep the change marked as rejected" becomes "record the relay's refusal and check the change again", as D12 says. A change the member's own check rejects is rejected as D11 says.

## Amendment 2: a payload that decrypts but does not decode

Added on 2026-11-08 for NOC-38. [Amendment 1's step table](#what-a-member-does-with-a-change-it-receives) gives a state to every step a change can fail, except one: the payload decrypts, so the signature and the AEAD show that a member of the space wrote it, but the plaintext does not decode as a payload map. Two agents found the gap independently: NOC-28, which builds the member log in ncdb-core, and NOC-37, which says how RFC 0004's field operations travel in a change.

The log is an on-disk format, so the owner decided: D13 A, parked as "unknown payload format", by merging #54 (Carsten, 2026-11-08). This amendment adds row 5a to the step table, and the sentences it changes elsewhere are marked *(Amendment 2)*. They describe the decided option. Nothing else in the RFC changes.

### What a member cannot tell apart

A plaintext that decrypts can fail to decode for four causes:

- **Too deep.** A value nests deeper than the decoder allows: 32 levels counted from the frame, so the payload map sits at level 3 (decision log, NOC-14; pending the owner in NOC-24).
- **Not canonical.** The bytes break [the CBOR profile](#the-cbor-profile): an integer or length not in its shortest form, map keys out of order or twice, an indefinite length, a tag, invalid UTF-8, or bytes after the map.
- **Wrong shape.** The map decodes, but a key that payload format 1 requires is missing or holds the wrong type, for example a clock that is not an unsigned integer.
- **A newer payload format.** A later format may drop a key that format 1 requires, or use something the profile of today forbids, such as a tag ("Future versions may allow specific tags") or a deeper nesting.

The first three are malformed: a buggy or hostile author wrote them. The fourth is valid data this core does not understand yet. A member cannot always tell them apart. The payload format number is key 1 of the payload map, so a member can read it only when the map decodes. A newer format that relaxes the profile or the depth limit looks to an older core exactly like malformed bytes.

What every member agrees on is the plaintext. The header is canonical, so the change has a stable id and a valid signature. The AEAD gives every member that holds the key the same plaintext, or fails for all of them. Decoding is a function of those bytes and of the core's version. So under each option below, two members on the same core version that hold the same changes give the change the same state. The options differ in what happens between core versions, and on an upgrade.

### The options and how each keeps convergence

- **A. Parked as "unknown payload format", whatever the cause.** The state is a function of the plaintext bytes and the core version, like every parked state. A newer core either reads the payload and folds it, or parks it with the same reason. A member that has not upgraded yet shows a different view from one that has, which is the reading of principle 2 the owner accepted for parked changes ([Conflicts](#conflicts-with-genesismd-and-rfc-0001), point 1). A puts no constraint on future formats: a raised depth limit, an allowed tag or a new payload format is one more thing an upgrade can fold.
- **B. Rejected, with a new reason "undecodable payload".** On one core version every member agrees. But a rejected change waits for a control change ([D11](#rejected-changes)), and no control change makes bytes decode. If a later core reads what an earlier one could not, a member that receives the change after its upgrade folds it, while a member that rejected it before its upgrade never checks it again. Two upgraded members holding the same changes would then differ, which principle 2 forbids. B converges only with one of two additions: changes rejected for this reason are checked again after every upgrade, which makes them parked in all but name and gives the state rejected two triggers; or the payload profile and the depth limit never change under header format 1, so that a newer core that relaxes them also writes a new header format, which older cores park at step 2.
- **C. Split by cause: a newer format parked, a malformed payload rejected.** The member parks a payload whose map decodes and whose format number it does not know, as step 6 does today, and rejects every other payload that does not decode, with the reason "malformed payload". The parked half converges as under A. The rejected half converges only under B's second addition: a member cannot read the format number of a payload that does not decode, so a newer format that relaxes the profile would be rejected forever by every older core. C therefore freezes the payload profile and the depth limit for header format 1. Within that rule, "malformed" is final: no core version under header format 1 reads the payload, so every member keeps it rejected.

### What an upgrade does

- **Under A.** After an upgrade, the core takes every parked change through the steps again, as it does for every parked change ([Versioning](#versioning-old-clients-keep-what-they-do-not-understand)). The log keeps only the frame, so the core decrypts the payload again. A payload the new core reads is folded, in the total order, as if it had just arrived, and its operations go through step 7. A payload that no version reads stays parked, with the same reason, on every member.
- **Under B.** Nothing, unless the first addition is chosen; then as under A, with the state rejected instead of parked.
- **Under C.** A parked newer format is folded as under A. A rejected malformed payload stays rejected. A core that relaxes the profile writes a new header format, which older cores park at step 2 and fold once they upgrade.

### What option A does not do

- **"Unknown payload format" may mean malformed.** The SDK counts the change as parked for an unknown payload format. Neither the SDK nor the app can tell an author on a newer core from a buggy or hostile one, and an upgrade may never fold the change. A space that holds one never reaches zero parked changes. The reason does not claim more than the member knows: it does not know the format.
- **A member gains no power by writing one.** The change stays in the log, is forwarded and counted, and touches no field. A member that may write in the space can already write any value into any field, so a payload that does not decode gives it nothing new.
- **The bytes are never repaired.** As with a frame, the core never re-encodes a stored payload, so a non-canonical payload stays as its author wrote it.

**D13. A payload that decrypts but does not decode.**

- A. Parked, with the reason "unknown payload format", whatever the cause. *(recommended)*
- B. Rejected, with a new reason "undecodable payload".
- C. Split by cause: a payload whose format number the member reads and does not know is parked as "unknown payload format"; any other payload that does not decode is rejected as "malformed payload".

Decided: A (Carsten, 2026-11-08). Recommendation: A. It is the only option that keeps convergence without freezing the payload profile and the depth limit, which the owner has not decided yet (NOC-24). It needs no new state and no new reason, and NOC-28 builds it already. B adds a reason that cannot be checked again by a control change, so it either breaks convergence after an upgrade or becomes parked under another name. C names malformed payloads truthfully, but only by making every relaxation of the profile a new header format. pm recommends A as the reading that loses nothing and stays convergent.

### If the owner chooses B or C

NOC-28's code parks such a change as "unknown payload format", as A says. If the owner chooses B or C, pm opens a follow-up ticket that changes ncdb-core's receive step and its tests to the chosen option, and names that ticket in this amendment's pull request. Under C, the same ticket adds the rule that a change to the payload profile or the depth limit comes with a new header format.

### What this means for RFC 0004 and RFC 0006

- **RFC 0004 (merge rules).** NOC-37 says that a payload holding a value nested too deep does not decode, and leaves its log state to this amendment. Under A it is parked.
- **RFC 0006 (the local view).** Its fold step 3 parks an unknown payload format. Under A, a payload that does not decode is parked with that same reason, so RFC 0006 needs no change. Under B or C, pm aligns it.

## Amendment 3: following RFC 0008

Added on 2026-11-08 for NOC-44. [RFC 0008](0008-membership.md) defines membership, roles and the first change of a space, and the owner accepted every one of its decisions, M1 to M26 (Carsten, 2026-11-08). Its section [Conflicts with GENESIS.md and the other RFCs](0008-membership.md#conflicts-with-genesismd-and-the-other-rfcs) names where this RFC said something else. This amendment changes those sentences, marked *(Amendment 3)* above. The owner has decided every point, so it adds no decision. The format of a change does not change.

- **Space ids are derived** (conflict 3, decision M7). D3 drew space ids at random, like object ids. A random space id does not bind the space to its creator: anyone who learns it could write a competing first change. A space id is now the first 16 bytes of a hash over the creator's member id and a random nonce in the genesis. It keeps what D3 chose it for: 128 bits nobody can predict, with no time in them. Object ids keep D3 ([RFC 0008, "What it names"](0008-membership.md#what-it-names)).
- **The key generation has a structure** (conflict 4, decision M6). Header key 4 stays one unsigned 64-bit integer. Its upper 16 bits are an ordinal and its lower 48 bits a tag that every member and the relay recompute from the key chain entry, so two concurrent rotations name different generations ([RFC 0008, "Generation ids"](0008-membership.md#generation-ids)).
- **Equivocations of the genesis and of revoked members** (conflict 5, decisions M24 and M3). This RFC keeps and folds both changes of every equivocation. For the genesis, both stay in the log, but only the one with the lower change id is the genesis, and the other folds nothing ([RFC 0008, "How the relay and members recognise it"](0008-membership.md#how-the-relay-and-members-recognise-it)). For a revoked member, both changes of an equivocation at or below its cutoff are accepted and reported, as for every other member; its changes above the cutoff lose the revoked right ([RFC 0008, "The cutoff"](0008-membership.md#the-cutoff)).
- **The control body** (conflict 6, decisions M18 and M22). Key 1 is the entry type and key 3 the control dependencies, as in a sealed payload. Key 2, the clock, is absent, so the relay does not learn when an admin acted, which D6 already kept from it for every change ([RFC 0008, "The control bodies"](0008-membership.md#the-control-bodies)).
- **What the relay sees** (conflict 2, decision M11). The relay applies the membership rules, so it reads the control bodies and their control dependencies. For control changes it therefore sees which changes a change builds on, and it learns roles, cutoffs, who saw what, the account behind each member across spaces, the sub-agent tree, the erasure approvers and the generation graph. For sealed changes, everything this RFC says the relay cannot see stays true ([RFC 0008, "How the relay applies the rules"](0008-membership.md#how-the-relay-applies-the-rules)).
- **Which changes the relay refuses** (decision M3). The relay refuses a revoked member's changes above the cutoff its revocation names, not every change from the moment the revocation arrives, so the verdict does not depend on the relay's order ([RFC 0008, "A member removed while offline who keeps writing"](0008-membership.md#a-member-removed-while-offline-who-keeps-writing)).
- **Pushing a refused change again** (conflict 7, decision M19). D12 stays as decided, with one exception: a member pushes a refused change to that relay once more when a control change arrives and its own fold newly accepts the change. It cannot loop, because only a new control change can make the fold accept it ([RFC 0008, "How the relay applies the rules"](0008-membership.md#how-the-relay-applies-the-rules), and [decision M19](0008-membership.md#decisions-for-the-owner)).

With this amendment, Amendments 1 and 2 also read as decided: the owner signed them off by merging #38 and #54, D10 to D12 and D13 each as recommended, D13 as A, a payload that decrypts but does not decode is parked as "unknown payload format" (Carsten, 2026-11-08).

## Amendment 4: following RFC 0007

Added on 2026-11-09 for NOC-47. [RFC 0007](0007-relay.md) defines the relay's protocol and its AWS backend, and the owner ruled on every point of its section [Conflicts with GENESIS.md and RFC 0001](0007-relay.md#conflicts-with-genesismd-and-rfc-0001) (Carsten, 2026-11-08). Two of them concern this RFC. This amendment changes the sentences they need, marked *(Amendment 4)* above. The owner has decided both, so it adds no decision. The format of a change does not change.

- **What the relay sees** (RFC 0007's conflict 1). The list in [What the relay can see, and what it cannot](#what-the-relay-can-see-and-what-it-cannot) names what the relay learns from the changes and the log. RFC 0007's [What the relay sees](0007-relay.md#what-the-relay-sees) adds what it learns from the protocol: when each member is online and from which network address, which client it runs, which spaces it follows from which cursor, and the sizes of large uploads. It also separates what the relay learns from what the server it runs on sees, on AWS with the setting each item depends on. The list now links RFC 0007's, so that neither is read as complete on its own.
- **Who uploads** (RFC 0007's conflict 4). This RFC left the uploader of a change unchecked. RFC 0007 requires the uploader to be an authenticated current member of the space ([What the relay checks](0007-relay.md#what-the-relay-checks)). The author may still be any member whose change the membership rules accept, so a current member can forward a change that another member wrote, in island mode or after that member left. This narrows who may upload, not who may author, and does not contradict this RFC.

## Amendment 5: the writer's model in the payload

Added on 2026-11-09 for NOC-40. [RFC 0005](0005-schema-and-model-lock.md) gives every app a `model.lock.json` with a `model.id`, 128 random bits that name the model's lineage, and a `model.version`, one higher for each build that changes the model. The owner decided two of its decisions as recommended (Carsten, 2026-11-08; [RFC 0005, "Decisions for the owner"](0005-schema-and-model-lock.md#decisions-for-the-owner)):

- **RFC 0005's D10 A.** The writer's model version travels in the payload as a "may ignore" key, so that an old client can report which model version it needs to read its parked changes.
- **RFC 0005's D13 C.** Both of its parts. Part A: a space records the `model.id` it was created with in its first control entry, and the SDK refuses to write into a space of another model; RFC 0008 carries it out in the genesis's key 13 ([RFC 0008, "Genesis"](0008-membership.md#genesis)). Part B: each change carries its writer's `model.id` with the model version of D10, in the same "may ignore" key, and a member parks every change of another model, whole. This amendment carries out part B.

RFC 0002 defines the payload, so the key needs this amendment. It records the key's number, its canonical CBOR and what a member does with it, and reopens no decision. The key's number and encoding are a wire format, which GENESIS.md reserves for the owner, so the owner signs them off by merging this amendment. The sentences it changes elsewhere are marked *(Amendment 5)*. (RFC 0005's D10 and D13 are not this RFC's [D10](#the-atomic-unit) and [D13](#amendment-2-a-payload-that-decrypts-but-does-not-decode), which Amendments 1 and 2 numbered.)

### The key

Payload key 64 holds the writer's model:

```text
model (CBOR map, payload key 64)
├── 1 model id                 16 bytes: the writer's model.id, its 32 hex digits as raw bytes
└── 2 model version            uint: the writer's model.version, 1 and up
```

- **Number 64.** It is the first key of the "may ignore" range, as RFC 0005's D10 asks: a core that does not know it folds the change as if it were absent ([Versioning](#versioning-old-clients-keep-what-they-do-not-understand)), so no client written before this amendment parks anything because of it.
- **One map, not two payload keys.** The model id and the model version name one thing, the lock that wrote the change, so they are present or absent together. The map has the shape of every other map of the format, such as the sealed personal value, and a later field of the model can be a new key of it.
- **Encoding.** The map follows [the CBOR profile](#the-cbor-profile). The model id is the 16 raw bytes of `model.id`, as the plaintext of the genesis's key 13 is. The version is an unsigned integer. Key 64 sorts after keys 1 to 5 of the payload, because its encoding `0x1840` sorts after `0x01` to `0x05`.
- **Who writes it.** The core writes key 64 into every sealed payload, from the `model.lock.json` it opened the database with. A control body carries none: it holds no content, and the space's model is in the genesis.
- **Size.** Key 64 adds 23 bytes to the payload before padding, inside the ciphertext. Padding to a multiple of 256 bytes (D8) keeps most changes in the same size class.

### What a member does with it

A member reads key 64 at step 6 of [Amendment 1's step table](#what-a-member-does-with-a-change-it-receives), "Understand the payload", after the payload decoded at step 5a:

- **No key 64.** The change goes on to step 7, as before this amendment. The key is "may ignore", and changes written before it have none.
- **The member's own model.** Key 64 holds a 16-byte model id equal to the `model.id` of the member's `model.lock.json`, and an unsigned model version. The change goes on to step 7, whatever the version. An operation on a collection or field the member's lock does not know is parked there on its own, as before.
- **Another model.** Key 64 names another model id, or its value is not a map of a 16-byte model id and an unsigned model version. The change is parked, whole, with the reason "another model". A value of the wrong shape does not show that the member's model wrote the change, so it is parked for the same reason.

The step table keeps its steps. Steps 2 to 6, step 5a included, still decide for the change as a whole, and step 7 still decides per operation. Step 6 gains a cause and the reason "another model". A change that fails an earlier step never reaches key 64: a rejected change stays rejected for the reason it has, and a payload that does not decode stays parked as "unknown payload format", because a member reads key 64 only from a payload map that decoded. A control change stops after step 4 and has no key 64.

**Why the member's own lock.** A member folds every operation by the rules of its own `model.lock.json`. RFC 0005's D13 B parks a change of another model so that no member folds it by the wrong rule, and the wrong rule is the rule of the lock the member folds with. Every member that may write into the space runs a lock whose `model.id` is the one the genesis records, because the SDK refuses to write into a space of another model (D13 A). So every one of them parks the same changes, and the state of a change stays a function of its bytes and the member's lock, as step 7's state already is.

**Why the whole change.** RFC 0005's D13 B parks every change of another model whole, as the owner decided. A field name of another model may name a field of another type. Folding the operations whose names the member knows would fold them by the wrong rule, and parking only the others would split one write transaction by a rule the member cannot know.

### What the model version is for

The model version never parks a change by itself. A change of the member's own model with a higher model version goes on to step 7, where an operation on a field the member's lock does not know yet is parked on its own. The SDK reports, per space, the highest model version among its parked changes and operations of the member's own model, so that an app can say "needs model version 7" ([RFC 0005, "Data from a newer model"](0005-schema-and-model-lock.md#data-from-a-newer-model), point 6). It counts the changes parked as "another model" under that reason, apart from them: their model version belongs to another lineage, and no upgrade of this app reaches it.

### What this does not do

- **It stops honest apps and buggy ones, not a hostile member.** A client that writes no key 64, or writes the member's own model id, has its change folded by the rules of the member's lock. A member that may write in the space can already write any value into any field (as [What option A does not do](#what-option-a-does-not-do) says), so it gains nothing new. Key 64 makes a change of another lineage park the same way on every member instead of folding by the wrong rule.
- **A change of another model stays parked for good.** The SDK refuses a lock with another `model.id` (RFC 0005, [Two checks: at build and on the device](0005-schema-and-model-lock.md#two-checks-at-build-and-on-the-device)), so no upgrade folds it. It stays in the log, is forwarded and is counted, as every parked change is.
- **No core has folded a change of another model before.** No version of the core that folds changes was released before this amendment, so no member holds such a change folded by a core that did not read key 64. Every released core reads it.
- **The relay sees nothing new.** Key 64 lives inside the sealed payload. The relay cannot read the model id or the model version, as it cannot read the space's model id in the genesis ([What the relay can see, and what it cannot](#what-the-relay-can-see-and-what-it-cannot)). Only the 23 bytes add to the ciphertext's length, which padding blurs as it blurs every length.

### Byte-exact examples

The examples are written by `docs/rfc/vectors/0002/generate.py` with Python's cbor2 6.2.1, the generator RFC 0004's examples use, independently of the core and of the tooling test's reader. `scripts/rfc-0002-model.test.mjs` checks that each decodes under the profile, re-encodes to the same bytes and equals its committed vector. The model is the one of RFC 0005's example lock: `model.id` `6f1c0e2a9b7d4c3e8a5f0b1d2c3e4f5a`, `model.version` 2.

**M1. The value of key 64.** `{1: h'6f1c0e2a9b7d4c3e8a5f0b1d2c3e4f5a', 2: 2}`, 21 bytes.

```text
a201506f1c0e2a9b7d4c3e8a5f0b1d2c3e4f5a0202
```

**M2. A payload that carries it.** Payload format 1; the clock of 2026-11-09T00:00:00Z with counter 0; one dependency, change id A (32 bytes of `0xaa`); one operation, [RFC 0004's example E1](0004-merge-rules-basic-types.md#byte-exact-examples) (a `set` of `tasks.title` on object T1); no padding; and M1 as key 64, the payload's last key.

```text
a50101021b01a1bd9ac800000003815820aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa0481a5015011111111111111111111
11111111111102657461736b7303657469746c65040105a20167536869702076
3102801840a201506f1c0e2a9b7d4c3e8a5f0b1d2c3e4f5a0202
```

## Summary of the choices

| Question | This RFC's answer |
| --- | --- |
| What is a change? | A signed frame around a header the relay reads and an AEAD payload it cannot. One change is one write in one space, checked as a whole; only an operation the client's model does not know is parked on its own (Amendment 1). |
| What happens to a change that fails a check? | It is kept as rejected, with its reason, and checked again when a control change of its space arrives (Amendment 1). |
| How are objects identified? | 128 random bits, generated by the member that creates the object. |
| How are changes identified? | The hash of the signed header bytes. |
| How are changes ordered? | A hybrid logical clock for the total order, and dependencies on the author's heads for the causal order. |
| How does a late change stay identifiable? | A loser that its winner never saw is a concurrent loser, the same on every member. |
| How is a change encoded? | Deterministic CBOR, restricted further, and checked by re-encoding. |
| How do old clients keep new data? | Fixed frame, "must understand" and "may ignore" keys, and parked changes that fold after an upgrade. |
| Which model wrote a change? | Payload key 64 names the writer's `model.id` and model version; a member parks a change of another model, whole (Amendment 5). |
| How does a member store its log? | Append-only, in the SQLite database of the local view, as exact frames. |
| What does the relay hand out? | Changes after a cursor: a dense position per space. |
| What does the relay see? | Membership, who writes when and how much, sizes and rhythm, who reads what, and what the control changes say: roles, cutoffs, control dependencies and accounts (Amendment 3). No content, and no ordering metadata of sealed changes. |
