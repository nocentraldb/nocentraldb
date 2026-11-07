# RFC 0002: The change and the log format

REQUEST FOR COMMENTS · DRAFT 0.1 · NOVEMBER 2026 · ticket NOC-6

This RFC defines what a change is, how it is encoded byte for byte, and how a space's log stores it, on a member's device and on the relay. Every other part of the core builds on it: the merge rules, the signatures, the relay and the fold into the local view.

It builds on [RFC 0001](0001-nocentraldb.md), above all [the design in one page](0001-nocentraldb.md#the-design-in-one-page), and on the principles in [GENESIS.md](../../factory/GENESIS.md). Where this RFC and either of them disagree, the conflict is named in [Conflicts with GENESIS.md and RFC 0001](#conflicts-with-genesismd-and-rfc-0001).

**Status.** A draft for the owner's decision. The wire and on-disk formats are reserved for the owner, so nothing here is final until the owner approves this RFC. The open decisions are listed in [Decisions for the owner](#decisions-for-the-owner), each with options and a recommendation. Until then, the design below describes the recommended option of each decision.

## What this RFC does not cover

- **Cryptography and keys.** The signature scheme, the AEAD, the hash function, key ids, key epochs and key rotation belong to RFC 0003 (NOC-7). This RFC says only where a "signature", an "AEAD ciphertext" and a "hash" sit in the format, and what each must cover.
- **Merge rules.** How each field type merges concurrent changes belongs to RFC 0004 (NOC-8). This RFC defines the ordering metadata those rules can use, and nothing about the rules themselves.
- **The payloads of rich text and streams.** This RFC leaves room for a Yrs update and for a stream chunk as the value of an operation. It does not design either type.
- **Compaction and the archive, membership changes and work orders.** This RFC reserves their place in the log and in the envelope. Their formats follow in later RFCs.
- **Code.** Nothing in `crates/` or `packages/` changes. The implementation is NOC-14.

## Terms

| Term | Meaning in this RFC |
| --- | --- |
| Space | The unit of sharing, membership and keys. Every object belongs to exactly one space, and every change belongs to exactly one space. |
| Member | A device, agent or server that holds a key in a space and a full copy of its log. |
| Author | The member whose key signed a change. |
| Change | One signed, encrypted unit that a member appends to a space's log: one local write transaction. |
| Envelope | The part of a change the relay can read: enough to route, check and store the change. |
| Payload | The part of a change only members can read: what was changed, and the ordering metadata. |
| Operation | One edit inside a payload: one operation on one field of one object. |
| Log | The append-only sequence of changes of one space, on one member or on the relay. |
| Fold | Applying the changes of the log to the local SQLite view. |
| Cursor | A position in the relay's log of a space, from which a member asks for the changes after it. |

## A change at a glance

A change has three layers. The outer **frame** never changes, so that every client, however old, can store, forward, identify and verify any change. Inside it, the signed **envelope** carries what the relay needs. Inside the envelope, the **payload** is an AEAD ciphertext that only members of the space can open.

```text
frame (CBOR array, fixed forever)
├── 0 frame version            uint, 1
├── 1 signed envelope bytes    bstr: the canonical encoding of the envelope map
└── 2 signature                bstr: the author's signature (RFC 0003)

envelope (CBOR map, readable by the relay)
├── 1 space id                 16 bytes
├── 2 author key id            bytes, defined by RFC 0003
├── 3 author sequence          uint, 1, 2, 3, ... per author key and space
├── 4 key epoch                uint: which space key sealed the payload (RFC 0003)
├── 5 kind                     uint: 0 sealed, 1 control
├── 6 payload                  bstr: AEAD ciphertext (kind 0), or the control body (kind 1)
└── 7 envelope format          uint, 1

payload (CBOR map, after decryption; only members can read it)
├── 1 payload format           uint, 1
├── 2 clock                    uint: a hybrid logical clock, 64 bits
├── 3 dependencies             array of change ids: the author's heads when it wrote
├── 4 operations               array of operations, applied in order
└── 5 padding                  bstr of zero bytes, optional

operation (CBOR map)
├── 1 object id                16 bytes
├── 2 collection               text, the collection's name in model.lock.json
├── 3 field                    text, the field's name in model.lock.json
├── 4 op                       uint: the operation, as the field type defines it
└── 5 value                    any CBOR value the field type defines
```

The rest of this RFC explains each part and why it looks this way.

## The parts of a change

### Space

Every change names its space in the envelope (key 1). The relay needs it to route the change, check membership and append it to the right log. A space id is 16 bytes, generated like an object id (see [Object ids](#object-ids)). A space's id never changes.

### Author key and author sequence

The author key id (key 2) names the key that signed the change. Its form is RFC 0003's. The relay needs it to verify the signature and to check that the key is a member of the space.

The author sequence (key 3) counts the author's changes in this space: 1 for its first change, then one higher for each next change. It lets a member see at once whether it has every change of an author up to some point. A gap means a change is still missing. The relay already sees how many changes each key writes, so the sequence reveals nothing new to it.

Two different changes with the same author key, space and sequence are an **equivocation**: the author's device was restored from a backup, cloned, or is misbehaving. They do not break the log, because a change's identity is its hash and not its sequence (see [Change ids](#change-ids)). Both changes are kept and both are folded. The member marks the author key as having equivocated, and the SDK reports it, because it means one key now writes from two places.

### Key epoch and kind

The key epoch (key 4) says which generation of the space's key sealed the payload. A member needs it to pick the right key. The epochs and their rotation are RFC 0003's.

The kind (key 5) has two values:

- **0, sealed.** The payload is an AEAD ciphertext. Content changes and work orders both use this kind, so the relay cannot tell them apart. RFC 0001 promises that the relay sees a work order only as an opaque envelope, and a separate kind would break that.
- **1, control.** The payload is a body the relay must read: membership changes and key rotations, because the relay checks membership. The body's format is RFC 0003's. A control change carries no content.

Other values are reserved. A member that meets an unknown kind keeps the change and folds nothing from it (see [Versioning](#versioning-old-clients-keep-what-they-do-not-understand)).

### The payload

The payload (envelope key 6) is the AEAD ciphertext of the canonical encoding of the payload map. RFC 0003 chooses the AEAD and its nonce. This RFC requires two things of it:

- **It binds the payload to its envelope.** The associated data covers the frame version and the envelope fields 1 to 5 and 7, so that the relay cannot move a payload to another space, another author or another sequence number.
- **It is signed as ciphertext.** The signature covers the envelope, ciphertext included, so the relay can verify a change without any key. RFC 0003 decides whether the payload also carries the author's identity inside the encryption.

Inside the payload, the ordering metadata comes first (see [Ordering metadata](#ordering-metadata)), then the operations.

### Operations: object, field and value

A change carries one or more operations, applied in the order of the array. A change is atomic: a member folds all of its operations or none of them. A change never spans two spaces, so a write transaction cannot span two spaces either, as RFC 0001 says in [What it is not](0001-nocentraldb.md#what-it-is-not).

Each operation names:

- **The object** (key 1), by its 16-byte id. Its space is the change's space.
- **The collection and the field** (keys 2 and 3), by their names in `model.lock.json`. Names are stable, because schema changes are additive and a field is never renamed. Names also let a client that does not know the field yet keep the operation and fold it after an upgrade.
- **The operation** (key 4), a small integer whose meaning the field type defines. RFC 0004 defines it for basic values, sets and links. Value 0 is reserved for "create the object", which takes no field (key 3 is absent) and puts the object into the collection.
- **The value** (key 5), any CBOR value of the profile below. The field type defines its shape.

The format leaves room for the field types that are not designed yet:

- **Rich text.** The value is a byte string holding a Yrs update. A focused edit is a small update, so the change stays small too.
- **Streams.** The value carries one chunk and its index in the stream. Each chunk is its own change, or several chunks share one change, as the stream type will decide.
- **Personal fields.** The value is itself an AEAD ciphertext under the person's key (RFC 0003), wrapped in the payload. Destroying the person's key makes that value unreadable, while the signature over the whole change stays valid, because the signature covers the bytes and not the plaintext. This is what [shred and audit](0001-nocentraldb.md#the-design-in-one-page) needs.
- **Custom field types.** A plugin defines its own operation numbers and value shapes, inside the same container.

### What the change does not carry

- **No wall-clock time in the envelope.** The relay learns when it receives a change, but not when it was written. The clock sits inside the payload.
- **No dependencies in the envelope.** The relay cannot see which changes a change builds on, so it cannot tell which members saw which changes, or which changes are concurrent.
- **No object, collection or field names in the envelope.** The relay cannot tell which objects or fields a change touches, or whether two changes touch the same one.

## Ids without a coordinator

No member asks anyone for an id. Every id is generated locally, offline, and still never collides with another member's.

### Object ids

An object id is 16 bytes (128 bits) from the platform's cryptographically secure random generator: `getrandom` natively, `crypto.getRandomValues` in the browser. Space ids are generated the same way.

**Why two members cannot collide.** A collision needs two members to draw the same 128 random bits. Even after 2⁴⁸ (about 280 trillion) objects in one space, the chance that any two of them collide is about one in 8 billion (the birthday bound, n²/2¹²⁹ = 2⁻³³). An id is generated from a secure generator, so no member can predict another's ids, and no member's clock or counter is involved.

A member that wants to create two objects with the same id on purpose can do so: it is the author, and it can write what it likes. The fold then sees two "create" operations for one id and treats them as concurrent creates, the way RFC 0004 defines. It cannot affect objects in a space it is not a member of.

| Rejected | Why |
| --- | --- |
| UUIDv7 (48-bit time plus random) | Puts the creation time into every id, and ids travel into links, URLs and exports; members learn when every object was made. The index locality it buys matters little at the size of one space. |
| Derived from the creating change (hash of change id and position) | The ids are needed inside the payload before the change exists and is hashed, so the derivation is circular. |
| Derived from author key and author sequence | Collides after a device is restored from a backup and reuses a sequence number. |
| Sequential ids from the relay | Needs a coordinator, so it breaks island mode. |

### Change ids

A change id is the hash of the frame's first two elements: the frame version, as one byte, and the signed envelope bytes, with a fixed context string in front so that the hash of a change can never be confused with any other hash in the system:

```text
change id = H("nocentraldb/change-id" || frame version || signed envelope bytes)
```

H is the hash function RFC 0003 picks; this RFC assumes 32 bytes of output. The signature is not part of the id, so the id does not depend on the signature scheme, and a scheme with randomised signatures would not change it.

**Why two members cannot collide.** Two different changes have different envelope bytes, because their author, sequence, payload ciphertext or space differs. Two equal ids then need a collision of the hash function, which a 32-byte cryptographic hash makes infeasible. The id is computed from the bytes, so every member and the relay compute the same id for the same change, without being told. A member that receives a change twice, from the relay and from a peer in island mode, recognises it as the same change.

The id does not change when a member forwards a change, and it identifies the exact bytes: a member can check that the change it holds is the change another member refers to.

### Operation ids

An operation is identified by its change's id and its position in the change's operation array, counted from 0. Field types use it where they need to name one operation, for example as the tag of a set addition (RFC 0004). It is unique because change ids are, and every member computes it the same way.

| Rejected | Why |
| --- | --- |
| Author key plus author sequence | Not unique when a device is restored from a backup and writes again under a sequence number it already used. |
| A random id chosen by the author | Does not bind the id to the content, so two different changes can claim one id, and a member cannot check a reference by recomputing it. |
| An id assigned by the relay | Needs the relay, so it breaks island mode, and the relay could hand out the same id twice. |

## Ordering metadata

The merge rules of RFC 0004 need two things from every change: a total order that every member computes the same way, for "the last writer wins", and the causal order, to tell an edit that saw another edit from an edit made at the same time without it. The payload carries both.

### The clock

Every payload carries a **hybrid logical clock** (key 2): one unsigned 64-bit integer. The upper 48 bits hold milliseconds since the Unix epoch, which lasts until the year 10889. The lower 16 bits hold a counter.

When a member writes, it sets the clock to the larger of its own wall clock and one more than the highest clock it has seen in the space. When it receives a change, it raises its "highest seen" to that change's clock. The clock therefore never runs backwards, and a change always has a higher clock than every change its author had seen. When the counter would overflow, the member waits for its wall clock to reach the next millisecond.

The clock is the **stamp** that RFC 0004 orders writes by. RFC 0004 defines the full order and its tie-breaks (on equal clocks, the author key and then the operation id, as bytes). This RFC guarantees what that order needs: every operation has an id no other operation shares, every member reads the same clock, author key and id from the same change, and a change's clock is higher than the clock of every change its author had seen.

### Dependencies

Every payload lists its **dependencies** (key 3): the change ids of the space's heads in the author's log at the moment it wrote. A head is a change that no other change in the author's log depends on yet. The list is sorted by bytes and holds no id twice. It always covers the author's own previous change in the space, directly or through another dependency.

The dependencies make the changes of a space a directed acyclic graph. A change A happened before a change B when B depends on A, directly or through other changes. Two changes are **concurrent** when neither happened before the other. A member computes this from its own copy of the log, without asking anyone. Usually the list holds one or two ids. It grows only while several members write without seeing each other, and shrinks again with the first change that sees them all.

A member folds a change only after it holds all of the change's dependencies. Until then the change waits in the log as pending. The relay hands out changes in the order it received them, which can put a change before its dependencies, for example after a member forwards changes it received in island mode.

### A late change stays identifiable

Principle 3 of GENESIS.md asks that a change which arrives late and loses to a newer change stays identifiable as such. With the clock and the dependencies, every member identifies it the same way, whatever order the changes arrived in:

- When one change overwrites another and **saw it** (the winner depends on the loser), the loser was simply overwritten. Its author's intent was replaced knowingly.
- When the winner **did not see** the loser (the two are concurrent), the loser is a **concurrent loser**. The fold records it as such, next to the field, and the SDK can show it. The change stays in the log; nothing is dropped.

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
- **Floating-point numbers** use the shortest of the 16-, 32- and 64-bit forms that keeps the value exactly, as §4.2.1 says. Every NaN is encoded as the single quiet NaN `0xf97e00`. Negative zero stays negative zero. The envelope and the ordering metadata never contain a float.
- **Text strings** are valid UTF-8, stored as given. The format does not normalise Unicode. A field type or the SDK may normalise before writing (RFC 0004 has the SDK normalise strings to NFC), but the encoding never changes the bytes it is given.
- **No tags**, and no simple values other than the three above. Future versions may allow specific tags.
- **Integers** stay within 64 bits.

A decoder accepts only canonical bytes. After decoding, the core encodes the value again and compares the bytes. A change that is not canonical is rejected, and never "repaired", because a repaired change would have another id and an invalid signature. The core keeps the exact bytes it received and never re-encodes a stored change.

The implementation (NOC-14) commits test vectors: changes as bytes, with their ids, checked by the native core and by the WebAssembly build alike.

| Rejected | Why |
| --- | --- |
| postcard | Compact and fast, but not self-describing: a reader needs the exact schema to skip a field. An old client could not skip a field it does not know, so it could not keep data it does not understand yet. Its encoding also follows Rust types, which the TypeScript side would have to mirror by hand. |
| A hand-written binary format | Needs its own specification, its own encoders in two languages and its own decoder hardening, all for what deterministic CBOR already gives. The risk of a subtle difference between the two encoders is the very risk this RFC must avoid. |
| JSON with a canonical form (RFC 8785) | No byte strings, so ciphertexts, ids and Yrs updates need base64 and grow by a third. Number handling differs between JavaScript and Rust. |
| Protocol Buffers | Its encoding is not deterministic across implementations, by its own documentation. |

### Versioning: old clients keep what they do not understand

The format changes only by adding. Each layer carries its own version, and an old client knows what to do with every layer it cannot read:

- **The frame never changes.** Its version stays 1. Every client can therefore store, forward and identify any change, and verify its signature, as long as the signature scheme is one it knows (RFC 0003).
- **Envelope keys 1 to 7 keep their meaning forever.** A new envelope format may add keys. Keys 1 to 63 are "must understand": a client that meets one it does not know stores the change but does not fold it. Keys from 64 up are "may ignore": a client that does not know them folds the change as if they were absent. The same rule holds for the payload map and the operation map.
- **A higher envelope or payload format number** than the client knows means "stored, not folded".
- **An operation on a collection or field the client's `model.lock.json` does not know** is stored with the change, and the rest of the change is folded. When the client upgrades and the field becomes known, the core folds the stored operations, in the total order, as if they had just arrived.

A change that a client keeps without folding is **parked**. Parked changes stay in the log, are forwarded and are counted. The SDK reports how many changes are parked and why, so that "keep data you do not understand" never turns into "silently ignore it" (principle 3).

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
├── state            pending, folded or parked
└── frame            the exact bytes of the change, as received or written
```

Appending a change and folding it happen in one SQLite transaction, so after a crash the log and the view always agree: a change is in both or in neither. Every other table, such as an index from author key and sequence to change id, can be rebuilt from the frames. The frame is the only source of truth.

The state of an entry is local and may change: pending becomes folded when the dependencies arrive, parked becomes folded after an upgrade. The frame never changes.

A change the member writes itself enters the log at once, folded and without a relay position. It is in the member's view before it leaves the device, and the member keeps working without a relay (island mode). When the relay accepts it, the member records the relay position.

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

The relay accepts a change when the frame is well-formed and canonical, the signature is valid for the author key, and the author key is a member of the space at the change's key epoch. It does not need to know who uploaded the change: any member may forward another member's changes, which island mode needs. The relay never decrypts, folds or reorders anything.

**The cursor** is a relay position. A member asks the relay for "the changes of this space after position p", and the relay answers in position order, a page at a time. Positions are dense, so a member sees at once whether a page is complete. A cursor belongs to one relay: if a space moves to another relay, members start from position 0 there, and recognise the changes they already hold by their ids.

The relay's log is a transport, not the truth. Every member holds the same changes, and the members, not the relay, decide what the space contains.

| Rejected | Why |
| --- | --- |
| Timestamps as cursors | Two changes accepted in the same instant would be ambiguous, and a clock that moves backwards would skip changes. |
| A cursor made of the member's version vector | Needs the relay to read dependencies, which are encrypted on purpose. |
| Positions with gaps (for example from a database sequence) | A member could not tell a gap from a lost change. |

## What the relay can see, and what it cannot

The relay holds no keys. Here is what it learns anyway, stated plainly (principle 4).

The relay **can see**:

- **Which keys are members of which space**, from the control changes it checks, and when the membership and the key epochs change.
- **Who writes, when and how much.** For each change: the space, the author key, the author's sequence number, the key epoch, the time the relay received it, and the size of the ciphertext. The size reveals roughly how much was written. Padding (see [Decisions for the owner](#decisions-for-the-owner), D8) blurs it but does not hide it.
- **The rhythm of work.** A burst of small changes looks like typing; one large change after days of silence looks like a member coming back from offline work.
- **Who reads what, and from where.** Which connection asks for which space from which cursor, and the network addresses of members, unless they connect through something that hides them.
- **The ids of changes**, and that a change was uploaded by a member other than its author, when that happens.

The relay **cannot see**:

- **Any content**: object ids, collection and field names, operations, values, rich text and stream chunks, personal fields.
- **The ordering metadata**: when a change was written by its author's clock, which changes it depends on, and which changes are concurrent.
- **Whether a sealed change is content or a work order.**
- **The model**: which collections and fields a space uses, and which schema version a member runs.

What the relay **can do** despite holding no keys, and what limits it:

- **It cannot forge or alter a change.** Every change is signed, and the signature covers the ciphertext.
- **It cannot move a payload** to another space, author or sequence number, because the AEAD binds the payload to its envelope.
- **It can withhold changes.** A member notices a withheld change when a later change depends on it, or when an author's sequence numbers have a gap. A member does not notice a withheld change that nothing depends on and that is its author's last.
- **It can show different members different subsets** of the log. Members notice it only when they exchange changes directly, as in island mode.
- **It can delete its log or refuse service.** Every member still holds its full copy, and the members can repopulate a new relay.

## Decisions for the owner

These are the decisions this RFC needs from the owner. Each lists the options and a recommendation. The design above describes the recommended option of each.

**D1. Ordering metadata.**

- A. Hybrid logical clock plus a list of dependencies. *(recommended)*
- B. Lamport clock plus a list of dependencies.
- C. Hybrid logical clock plus a version vector.

Recommendation: A. "The last writer wins" matches what users expect in time, and the dependencies give the causal order at a size that does not grow with the number of keys.

**D2. Change ids.**

- A. The hash of the signed envelope bytes. *(recommended)*
- B. Author key plus author sequence.
- C. A random id.

Recommendation: A. It is unique without a coordinator, every party computes it independently, and a backup restore cannot make two changes share it.

**D3. Object and space ids.**

- A. 128 random bits from a secure generator. *(recommended)*
- B. UUIDv7: 48 bits of creation time and 74 random bits.

Recommendation: A. It leaks nothing about when an object was made into links, URLs and exports.

**D4. Encoding.**

- A. CBOR with the deterministic profile above. *(recommended)*
- B. postcard.
- C. A hand-written format.

Recommendation: A. It is deterministic, self-describing (so old clients can keep what they do not understand), and has maintained libraries in Rust and TypeScript.

**D5. How an operation names its field.**

- A. The collection's and field's names from `model.lock.json`. *(recommended)*
- B. Numeric ids that `ncdb model build` assigns.

Recommendation: A. Names never change, need no id registry that two developers could fill differently, and an old client can tell which unknown field an operation is for. They cost a few bytes per operation, inside the ciphertext.

**D6. Where the clock sits.**

- A. Inside the payload, invisible to the relay. *(recommended)*
- B. In the envelope, so that the relay can refuse changes whose clock is far ahead of its own.

Recommendation: A. B would show the relay when every change was written, offline work included. It would also protect only changes that pass through the relay, not those exchanged in island mode.

**D7. Where a member stores its log.**

- A. A table in the SQLite database of the local view. *(recommended)*
- B. Separate append-only segment files.

Recommendation: A. Append and fold in one transaction, and the same storage natively and in the browser.

**D8. Padding the payload.**

- A. Pad every payload to a multiple of 256 bytes. *(recommended)*
- B. No padding.
- C. Pad to the next power of two.

Recommendation: A. It hides the exact length of short values, such as "yes" against "no", at a cost of 128 bytes per change on average. C hides more but doubles the size of large changes such as rich-text pastes. No padding hides the length of a large write. The format allows changing the rule later, because padding is a payload field the reader ignores.

**D9. Changes whose meaning a client does not know.**

- A. Store them as parked, report them in the SDK, and fold them after an upgrade. *(recommended)*
- B. Refuse to sync a space until the client is upgraded.

Recommendation: A. It is what GENESIS.md asks: old clients keep data they do not understand yet. It needs the clarification in the next section.

## Conflicts with GENESIS.md and RFC 0001

This RFC agrees with GENESIS.md and RFC 0001 except in the points below. Each needs the owner's ruling, or a change to the other document.

1. **Convergence and parked changes.** GENESIS.md's principle 2 says the same set of changes yields the same state on every member. A client that parks a change it does not understand (D9) holds the same changes as an upgraded client but shows a different state. This RFC reads principle 2 as "every member that understands all the changes it holds ends in the same state", and makes the parked changes visible. The owner should confirm this reading, or choose D9 B.
2. **What the relay sees is more than RFC 0001 lists.** RFC 0001, GENESIS.md and the FAQ say the relay sees "which key is in which space, and when and how much each key writes". [What the relay can see](#what-the-relay-can-see-and-what-it-cannot) adds the change sizes, the key epochs and their timing, the rhythm of work, who reads which space from where, and that the relay can withhold changes or show members different subsets. None of it contradicts RFC 0001, but a reader of RFC 0001 alone could infer more than the design delivers, which principle 4 forbids. We propose that RFC 0001 and the FAQ link to that section.
3. **Concurrent creates of one object id.** RFC 0001 says every object belongs to exactly one space. A member can deliberately create two objects with the same id in one space (see [Object ids](#object-ids)). It cannot cross spaces, so it does not break that sentence, but RFC 0004 has to define what two creates of one id mean.

## Summary of the choices

| Question | This RFC's answer |
| --- | --- |
| What is a change? | A signed frame around an envelope the relay reads and an AEAD payload it cannot. One change is one atomic write in one space. |
| How are objects identified? | 128 random bits, generated by the member that creates the object. |
| How are changes identified? | The hash of the signed envelope bytes. |
| How are changes ordered? | A hybrid logical clock for the total order, and dependencies on the author's heads for the causal order. |
| How does a late change stay identifiable? | A loser that its winner never saw is a concurrent loser, the same on every member. |
| How is a change encoded? | Deterministic CBOR, restricted further, and checked by re-encoding. |
| How do old clients keep new data? | Fixed frame, "must understand" and "may ignore" keys, and parked changes that fold after an upgrade. |
| How does a member store its log? | Append-only, in the SQLite database of the local view, as exact frames. |
| What does the relay hand out? | Changes after a cursor: a dense position per space. |
| What does the relay see? | Membership, who writes when and how much, sizes and rhythm, who reads what. No content and no ordering metadata. |
