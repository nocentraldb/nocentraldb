# RFC 0007: The relay protocol and its AWS backend

REQUEST FOR COMMENTS · NOVEMBER 2026 · ticket NOC-13

This RFC defines the relay: the protocol members use to push and pull a space's changes and to follow them live, the checks the relay runs before it appends a change, the interface every relay backend implements, and the first backend, serverless on AWS with DynamoDB and S3.

It builds on [RFC 0001](0001-nocentraldb.md), above all [the design in one page](0001-nocentraldb.md#the-design-in-one-page), on the principles in [GENESIS.md](../../factory/GENESIS.md), and on [use cases](../use-cases.md) 2, 4, 5 and 7. It uses the change, the log and the cursor of [RFC 0002](0002-change-and-log-format.md) (NOC-6, decided) and the keys, membership entries, key envelopes and person-key store of [RFC 0003](0003-keys-and-cryptography.md) (NOC-7, decided), and the genesis, roles, cutoffs and person-key entry of [RFC 0008](0008-membership.md) (NOC-23, decided), without redefining them. Where this RFC needs something from either, it says so in [What this RFC needs from RFC 0002, RFC 0003 and RFC 0008](#what-this-rfc-needs-from-rfc-0002-rfc-0003-and-rfc-0008).

**Status.** Decided. GENESIS.md reserves the wire format for the owner, and anything that writes to AWS or raises the bill. The owner ruled on decisions R1 to R11 and on every point in [Conflicts](#conflicts-with-genesismd-and-rfc-0001), and this RFC was merged with the owner's approval in #37 (Carsten, 2026-11-08); the owner decided R12, the keys of each message, as recommended (Carsten, 2026-11-10). The design below describes the decided options, and [Decisions](#decisions) records each one. The relay core and a SQLite backend for local runs are built in `crates/ncdb-relay` (NOC-87); the AWS backend is not built or deployed. [History](#history) says when each part was written.

**How to give feedback.** This RFC has no discussion of its own yet: start one in the category [RFC feedback](https://github.com/nocentraldb/nocentraldb/discussions/categories/rfc-feedback), named after the RFC, or open a pull request against this file to change its text. [Giving feedback](README.md#giving-feedback) says how, and what happens to your point.

## What this RFC does not cover

- **The bytes of a change and of the log.** RFC 0002. This RFC moves frames as opaque bytes, except for the header fields the relay reads.
- **Cryptography and keys.** [RFC 0003](0003-keys-and-cryptography.md). This RFC uses the member key to authenticate a connection, with its own context string, and invents no primitive.
- **Membership rules and roles.** Who may add or remove whom is [RFC 0008](0008-membership.md). The relay applies those rules, from the same code the members run; this RFC only says where.
- **Compaction and the archive.** A later RFC. This RFC reserves the error a member gets when its cursor points before a compacted range.
- **Peer discovery in island mode.** How two members on one network find each other is a later RFC. This RFC says which messages they exchange once connected.
- **Moving a space from one relay to another.** Use case 5 leaves it open, and so does this RFC; [Island mode](#island-mode) says what already works.
- **Adapters**, such as the EventBridge adapter of use case 7. An adapter is a member of the space, not part of the relay.
- **Code and infrastructure.** No crate, no template, no AWS resource. The implementation follows in later tickets.

## Terms

| Term | Meaning in this RFC |
| --- | --- |
| Relay | The service that stores each space's log of changes and hands it out. It holds no key that decrypts content. |
| Backend | The storage a relay runs on: SQLite, Postgres, or DynamoDB with S3. |
| Member, space, change, header, payload, frame | As in RFC 0002. |
| Membership entry, key envelope, key chain entry, shred entry, person-key store | As in RFC 0003. All but the person-key store are control changes (RFC 0002, kind 1). |
| Position | RFC 0002's relay position: a dense integer per space, 1, 2, 3, ..., in the order the relay accepted the changes. |
| Cursor | A position. "After cursor p" means the changes at positions p + 1 and up. |
| Head | The highest position in a space's log on the relay; 0 for an empty log. |
| Relay epoch | A random 16-byte id of the relay's current history. Positions are valid only within one epoch. The relay draws a new epoch whenever its stored log may have lost appends, such as after a restore from backup. |
| Connection | One authenticated session between a member and the relay. |
| Uploader | The member whose connection pushes a change. Often the author, but any member may forward another member's change. |

## The relay in one page

- **The relay is a transport, not the truth.** Every member holds the full log. The relay appends changes in the order it accepts them, numbers them densely, and hands them out after a cursor. Members decide what a space contains; the relay decides nothing about content.
- **One connection, three things to do.** A member opens one connection, proves which member key it holds, and then pushes changes, pulls from a cursor and subscribes to spaces, all over the same connection.
- **Every check uses public data.** The relay checks that a frame is well-formed, that its signature is valid for the author's public key, and that the author and the uploader are members. Public keys and membership come from control changes in the log, which are signed but not encrypted. No check needs a key that decrypts anything.
- **Backends are interchangeable.** All checks live in one relay core. A backend only stores: it appends at the next position if nobody else did, reads a range, keeps the membership table, and puts a person-key record only if the stored record is still the one the core checked it against. SQLite, Postgres and DynamoDB with S3 all implement the same interface and pass the same test suite.
- **On AWS, it costs nothing while nobody writes.** API Gateway WebSockets, Lambda, DynamoDB and S3. A small team pays about 10 dollars a month. 1,000 agents that each read 1,000 times a minute pay nothing for the reads, because reads are local; their writes cost about 800 dollars a month at one write per agent per minute.
- **Without the relay, members keep working.** Writes go into the local log at once. On reconnect, a member pulls what it missed and pushes what the relay has not confirmed. The relay recognises changes it already holds by their ids. A relay restored from a backup announces a new epoch, so no member trusts a position the restore undid.

## The protocol

### Transport and encoding

A member talks to the relay over one **WebSocket** connection (RFC 6455) with TLS. Every message is one binary WebSocket message holding one CBOR map, encoded with RFC 0002's deterministic profile. A message's key 1 names its type, and key 2 holds a request id that the member chooses, so that answers can arrive out of order. Messages the relay sends on its own, such as new entries for a subscription, carry no request id. [The keys of each message](#the-keys-of-each-message) gives every field its key.

WebSockets work in every browser we target, natively on every platform, and through API Gateway on AWS, so the same protocol runs everywhere (principle 7). See decision R1 for the alternatives.

**Limits that hold on every backend.** A message is at most 128 KiB, because that is API Gateway's limit, and a protocol that behaves differently by backend would make backends not interchangeable. A change whose frame is larger travels through an upload (see [Large changes](#large-changes)). A push carries at most 32 changes. These numbers are protocol constants, announced in the `welcome` message, so that a future relay can raise them.

### The keys of each message

The sections below write a message with its field names, such as `push {req, space, frames}`, for reading. On the wire every field has an integer key, by one rule (decision [R12](#r12-the-keys-of-each-message)):

- **Key 1** holds the message's type, as text: `"push"`, `"pushed"`, `"error"`.
- **Key 2** holds the request id. Key 2 is absent when a message has none: `hello`, `challenge`, `auth`, `welcome`, and `entries` sent live for a subscription.
- **Every other field** takes the keys 3, 4, … in the order this RFC lists it in the message.
- **A nested map**, such as a push result or a log entry, numbers its fields from 1, in the order this RFC lists them.
- **Message types and error codes are text**, never numbers.

| Message | Direction | Keys after key 1 |
| --- | --- | --- |
| `hello` | member → relay | 3 versions, 4 client |
| `challenge` | relay → member | 3 version, 4 relay id, 5 nonce, 6 limits |
| `auth` | member → relay | 3 signing key, 4 agreement key, 5 signature |
| `welcome` | relay → member | 3 member id, 4 relay epoch, 5 spaces |
| `push` | member → relay | 2 req, 3 space, 4 frames |
| `pushed` | relay → member | 2 req, 3 results |
| `pull` | member → relay | 2 req, 3 space, 4 epoch, 5 after, 6 limit |
| `entries` | relay → member | 2 req, 3 space, 4 entries, 5 head |
| `subscribe` | member → relay | 2 req, 3 space, 4 epoch, 5 after |
| `unsubscribe` | member → relay | 2 req, 3 space |
| `keys` | member → relay | 2 req, 3 space |
| `keys` | relay → member | 2 req, 3 space, 4 envelopes, 5 chain |
| `person-keys` | member → relay | 2 req, 3 space |
| `person-keys` | relay → member | 2 req, 3 space, 4 records |
| `put-person-keys` | member → relay | 2 req, 3 space, 4 records |
| `ok` | relay → member | 2 req |
| `upload` | member → relay | 2 req, 3 space, 4 size |
| `upload` | relay → member | 2 req, 3 id, 4 url |
| `ping` | member → relay | 2 req |
| `pong` | relay → member | 2 req |
| `error` | relay → member | 2 req, 3 code, 4 retry after |

| Map | Keys | Where |
| --- | --- | --- |
| `limits` | 1 largest message, 2 changes per push, 3 largest frame | `challenge`, key 6 |
| `upload reference` | 1 upload id | An element of `push`'s frames, in place of a frame's bytes |
| `result` | 1 accepted position, 2 duplicate position, 3 rejected code | An element of `pushed`'s results; it holds exactly one of the three keys |
| `entry` | 1 position, 2 accepted at, 3 frame or download | An element of `entries`' entries, and of `keys`' envelopes and chain |
| `download` | 1 url, 2 size | An entry's key 3, in place of a frame's bytes |
| `record` | 1 person-key id, 2 generation, 3 writer member id, 4 wrapped, 5 signature | An element of `person-keys`' and `put-person-keys`' records |

So a push of two frames, the second through an upload, is `{1: "push", 2: 7, 3: space id, 4: [frame bytes, {1: upload id}]}`, and its answer `{1: "pushed", 2: 7, 3: [{1: 41}, {3: "bad_signature"}]}`.

### Opening a connection

```text
member → relay   hello      {versions: [1], client: "ncdb-rs/0.3"}
relay  → member  challenge  {version: 1, relay id, nonce: 32 random bytes, limits}
member → relay   auth       {signing key, agreement key, signature}
relay  → member  welcome    {member id, relay epoch, spaces: [space ids this member belongs to]}
```

- `hello` lists the protocol versions the member speaks. The relay picks the highest it also speaks, or answers `unsupported_version` and closes.
- `hello` may carry a client string, such as `ncdb-rs/0.3`, to help debug version problems. It is optional, and the relay sees it (see [What the relay sees](#what-the-relay-sees)).
- `challenge` carries the relay id (a stable random id of this relay deployment), a fresh nonce and the limits above.
- `auth` carries the member's two public keys and an Ed25519 signature, with its member signing key, over `H("nocentraldb/relay-auth" || relay id || nonce)`. This is RFC 0003's relay authentication, its [decision 13](0003-keys-and-cryptography.md#13-how-a-member-authenticates-to-the-relay): the context string keeps this signature from ever being mistaken for the signature of a change, and binding the relay id keeps one relay from replaying a member's answer against another. A separate authentication key is the alternative RFC 0003 names there.
- The relay computes the member id from the two public keys, as RFC 0003 defines it, and verifies the signature. From then on the connection acts as that member id, and only as it.
- `welcome` carries the relay epoch. A member that stored positions under another epoch of this relay treats them all as unconfirmed (see [Restores and the relay epoch](#restores-and-the-relay-epoch)).

A connection proves possession of a member key, nothing more. Whether that member may read or write a space is checked per request, against the space's membership, every time.

The member key is the only authentication the protocol requires, and the only one our AWS backend uses (decision R2). A relay backend is an adapter, and a developer who builds their own adapter on top of ours can put an authentication step of their own in front of the connection, for example Cognito or their company's single sign-on. That step decides who may open a connection at all; the challenge above still decides which member the connection acts as, and the membership checks still decide what it may do. Such a step adds a party that learns who connects, which the developer then names in their own list of what their server sees.

### Push: append signed, encrypted changes

```text
member → relay   push    {req, space, frames: [frame | {upload: id}, ...]}
relay  → member  pushed  {req, results: [result, ...]}

result = {accepted: position}
       | {duplicate: position}     the relay already holds this change id
       | {rejected: error code}
```

The relay checks each frame (see [What the relay checks](#what-the-relay-checks)) and appends the accepted ones in the order of the array, at the next free positions. One result per frame, in the same order. The frames of one push are checked one by one: a rejected frame does not stop the frames after it, because the changes are independent and convergence never depends on the relay's order.

A **duplicate** is not an error. A member that pushes a change the relay already holds, for example one it received from a peer in island mode, learns the change's position and records it.

A push is **idempotent**: if the connection drops before `pushed` arrives, the member pushes the same frames again and gets `duplicate` for the ones that made it.

### Pull: read from a cursor

```text
member → relay   pull     {req, space, epoch, after: position, limit}
relay  → member  entries  {req, space, entries: [entry, ...], head}

entry = {position, accepted at, frame | {download: url, size}}
```

The relay answers with the entries after the cursor, in position order, up to `limit` and up to the message size. Positions are dense, so the member sees at once whether the page is complete: the last entry's position is the next cursor, and the member pulls again until it reaches `head`. `accepted at` is the relay's clock when it appended the change; RFC 0002 uses it only as a diagnostic.

A cursor is valid only in the epoch it was taken in. A `pull` or `subscribe` that names another epoch gets `epoch_changed`, never entries. A member that has never pulled a space starts at cursor 0. A member that joins a space pulls the whole log, as RFC 0003 requires, so that it can walk the key chain and decrypt the full history.

### Subscribe: follow a space live

```text
member → relay   subscribe    {req, space, epoch, after: position}
relay  → member  entries      {req, space, entries, head}        catch-up pages, as for a pull
relay  → member  entries      {space, entries, head}             later, whenever changes are appended
member → relay   unsubscribe  {req, space}
```

A subscription first catches the member up from its cursor, like a pull, and then sends every newly appended entry as it is appended. Live entries carry the frame when it fits in one message and a download link otherwise. Each live message carries the space's head, so a member that missed a message, for example because the relay dropped it, sees the gap and pulls the missing range. Nothing polls (use case 7).

A subscription lives as long as its connection. When the relay appends a membership entry that removes a member, it ends that member's subscriptions to the space with `removed_member`.

### Keys and the person-key store

Key envelopes and key chain entries are control changes, so they arrive with the log. Two requests let a member find its keys without reading the whole log first, and keep the person-key store that RFC 0003 puts on the relay:

```text
member → relay   keys              {req, space}
relay  → member  keys              {req, space, envelopes: [entry], chain: [entry]}
                                   the key envelopes addressed to this member id, and the key chain entries

member → relay   person-keys       {req, space}
relay  → member  person-keys       {req, space, records: [record, ...]}
member → relay   put-person-keys   {req, space, records: [record, ...]}
relay  → member  ok                {req}

record = {person-key id, generation, writer member id, wrapped, signature}
```

The store follows RFC 0003's [decision 14](0003-keys-and-cryptography.md#14-integrity-of-the-person-key-store). The relay cannot see inside a wrapped person key, so it relies on what sits outside the wrapping and on the log:

- **The log names every person key.** A member that creates or re-wraps a person-key record appends a **person-key entry**, a control change. It holds the person-key id, the commitment to the key (RFC 0003), the generation the record is wrapped under and the SHA-256 hash of the record. [RFC 0008](0008-membership.md#person-key-entry) defines the entry.
- **Every record is signed.** A record carries the person-key id, the generation it is wrapped under and the member id of its writer in the clear. The writer signs it with its member key, with the context string `"nocentraldb/person-key-record"`.
- **The relay accepts a record only under four conditions.** It comes from a current member. Its signature verifies against the writer's key. Its person-key id is named by a person-key entry in the log whose generation and record hash match it, and destroyed by no shred entry. Its generation is the stored record's generation or descends from it in the generation graph ([RFC 0008](0008-membership.md#how-the-relay-applies-the-rules)). Otherwise the relay answers `bad_record`. The check and the write are one step: the backend stores the records only if the stored records are still the ones they were checked against, so two members that put a record of one person key at the same time cannot both replace the generation each of them checked against ([The backend interface](#the-backend-interface)).
- **Members check the commitment.** A member accepts a record only if the signature verifies and the unwrapped key matches the commitment in the log. A relay that swaps records is caught, and the signature names who wrote each record.
- **Nothing deletes a record except a shred entry.** There is no delete request. The relay deletes a record only after it appended a verified shred entry for its person-key id. The entry must be signed by a current member and signed off by the account key of a second account (RFC 0003, narrowed by RFC 0008). The relay deletes the record when the shred entry becomes stable, by the same rule as on a member, not in the transaction that appends it ([RFC 0008, M20](0008-membership.md#effects-that-wait-until-a-change-is-stable)). Until then a shred that a concurrent revocation rejects can still be undone.

**What this does not prevent.** A member of the space can still upload a record whose wrapped key does not match the commitment. Every member then rejects that record and reports its writer, and any member that still holds the key writes a correct one. Until then, new members cannot open that person's fields. A member could equally have leaked the person key itself (RFC 0003).

### Large changes

A change whose frame does not fit into one message, such as a large paste into a rich-text field, travels through object storage:

```text
member → relay   upload    {req, space, size}
relay  → member  upload    {req, id, url}               a one-time upload URL, valid for 15 minutes
member → (url)   PUT the frame bytes
member → relay   push      {req, space, frames: [{upload: id}]}
```

The relay reads the uploaded bytes and checks them like any other frame. On a pull, a large entry carries a short-lived download URL instead of the frame. The largest frame the relay accepts is a protocol limit too (decision R5).

### Liveness

The member sends `ping {req}` when its connection has been idle for 5 minutes, and the relay answers `pong {req}`. A relay may close any connection at any time (API Gateway closes every connection after 2 hours); the member reconnects, authenticates again and resubscribes from its cursors. Reconnects back off with jitter, so 1,000 agents do not reconnect in the same second after an outage.

### The errors

Every error is a CBOR map `{1: "error", 2: req, 3: code, 4: retry after (seconds, optional)}`, or a `rejected` result inside `pushed`. An error never carries a frame, a payload or any content; it names the code, and for a rejected frame its position in the push.

| Code | When | What the member does |
| --- | --- | --- |
| `unsupported_version` | The relay speaks none of the member's protocol versions. | Upgrade. Keep working in island mode. |
| `unauthenticated` | A request arrived before a valid `auth`, or the signature was wrong. | Reconnect and authenticate. |
| `not_member` | The connection's member id is not a current member of the space. | Stop syncing the space; the SDK reports it. |
| `removed_member` | The member was removed from the space; sent once, then the subscription ends. | As `not_member`. The member keeps its local copy (RFC 0003). |
| `unknown_space` | The relay holds no log for this space id. | Create the space, or check the id. |
| `space_exists` | A space-creating change names a space the relay already holds. | Choose a new space id. |
| `not_allowed` | The member may not create spaces on this relay (decision R6). | The SDK reports it. |
| `malformed` | The frame is not well-formed or not canonical CBOR (RFC 0002). | Report it as a bug; never retry the same bytes. |
| `too_large` | The frame exceeds the largest frame the relay accepts. | The SDK reports it; the change stays local. |
| `unsupported_suite` | The relay does not know the change's suite and so cannot verify its signature. | Keep the change local until the relay is upgraded. |
| `bad_signature` | The signature does not verify against the author's signing key. | Record the relay's refusal and check the change again ([RFC 0002, "A change the relay refuses after the member folded it"](0002-change-and-log-format.md#a-change-the-relay-refuses-after-the-member-folded-it)). Push it to this relay again only once its own fold newly accepts it after a control change arrived ([RFC 0008, M19](0008-membership.md#m19-pushing-again-a-change-the-relay-refused)). |
| `author_not_member` | The author holds no grant that writes under the change's generation, or the change's author sequence lies above the cutoff of a revocation (RFC 0008). | Record the relay's refusal and check the change again ([RFC 0002, "A change the relay refuses after the member folded it"](0002-change-and-log-format.md#a-change-the-relay-refuses-after-the-member-folded-it)). Push it to this relay again only once its own fold newly accepts it after a control change arrived ([RFC 0008, M19](0008-membership.md#m19-pushing-again-a-change-the-relay-refused)). |
| `membership_rule` | A control change breaks the membership rules (RFC 0008). | Record the relay's refusal and check the change again ([RFC 0002, "A change the relay refuses after the member folded it"](0002-change-and-log-format.md#a-change-the-relay-refuses-after-the-member-folded-it)). Push it to this relay again only once its own fold newly accepts it after a control change arrived ([RFC 0008, M19](0008-membership.md#m19-pushing-again-a-change-the-relay-refused)). |
| `epoch_changed` | The cursor belongs to an earlier relay epoch, for example after a restore. | Treat every stored position of this relay as unconfirmed, pull from 0, push every change the relay lacks (see [Restores and the relay epoch](#restores-and-the-relay-epoch)). |
| `bad_record` | A person-key record is unsigned or badly signed, has no person-key entry with its generation and record hash, names a person-key id a shred entry destroyed, or would replace a record whose generation it does not descend from. | Report it as a bug; never retry the same record. |
| `compacted` | The cursor points before a range moved into the archive (the compaction RFC). | Fetch the archive, then pull from its end. |
| `bad_cursor` | The cursor is negative or above the head. | Pull from 0 or from the last confirmed position. |
| `rate_limited` | The member or the space exceeded its quota. | Retry after the given time. |
| `unavailable` | The backend failed or timed out. | Retry with backoff. Nothing was appended. |

A change the relay refuses is not dropped by the member. If the member already folded it, it stays folded. The member records the relay's refusal next to the change, does not push it to that relay again, and checks the change again against its own control changes ([RFC 0002, "A change the relay refuses after the member folded it"](0002-change-and-log-format.md#a-change-the-relay-refuses-after-the-member-folded-it)). The SDK reports the refusal (principle 3). A `rejected` result for one change says nothing about the others in the same push.

## What the relay checks

The relay runs every check below in its core, the same on every backend, before the backend appends anything.

| Check | With which data | Why it needs no content key |
| --- | --- | --- |
| The connection holds the member key it claims. | The two public keys and the signature in `auth`, against the relay's nonce. | Signature verification uses only the public key. |
| The uploader may write to the space. | The membership table: the connection's member id is a current member. | Membership comes from membership entries, which are control changes: signed, not encrypted. |
| The frame is well-formed and canonical. | The frame bytes: decode, re-encode, compare (RFC 0002). | Structure, not content. The payload is a byte string the relay never opens. |
| The frame is not too large. | Its length. | |
| The relay knows the suite. | Header key 8, the suite id. | It is a number in the signed header. |
| The signature is valid. | The author's public signing key, from the membership table, and the signed header bytes (RFC 0003, Ed25519, strict verification). | Encrypt-then-sign: the signature covers the ciphertext, so it verifies without decrypting (RFC 0003). |
| A sealed change's author may write it. | The header's author, author sequence and generation, against the membership table: the author's grants with their from generations, the revocations with their cutoffs, and the generation graph ([RFC 0008](0008-membership.md#sealed-changes)). | Numbers in signed headers and control bodies; nothing is decrypted. |
| A control change obeys the membership rules. | `resolve` of [RFC 0008](0008-membership.md#how-the-relay-applies-the-rules) over the relay's log plus the change, the same code the members run; the control body, which RFC 0002 makes readable to the relay. | Control bodies carry ids, public keys, wrapped keys and shred ids, never content. A key envelope is HPKE ciphertext to one member; the relay stores it and cannot open it. |
| A person-key record is acceptable (RFC 0003, decision 14). | The writer's signature over the record's clear fields and wrapped bytes; the person-key entries and shred entries in the log; the generation of the stored record. | Signature verification and ids only. The relay never unwraps the record; the commitment is checked by members. |
| A shred entry is signed by a current member and signed off by a second account. | The member's signature, and the account key signature, against the member certificates in the membership table (RFC 0003). | Signature verification uses only public keys. |
| The change is not already in the log. | The change id, which the relay computes from the frame (RFC 0002). | A hash of the bytes. |
| The reader may read the space. | The membership table, on every pull, subscribe, keys and person-keys request. | As above. |

What the relay **does not** check, because it cannot without reading content, and what follows from that:

- **Whether a sealed payload decrypts**, or what it says. A member that writes garbage under a valid signature gets it appended. Every other member detects it on decryption, keeps it marked as rejected, and the signature names the author (RFC 0003).
- **Equivocations.** Two changes with the same author and sequence are an equivocation (RFC 0002). The relay reads the author sequence only to compare it with the cutoffs of revocations, so it appends both changes of an equivocation: both are validly signed, and refusing one would make the relay pick a winner. Members detect and report it.
- **The order of dependencies.** They are encrypted (RFC 0002). The relay appends in arrival order, and members fold in any order.

Because every check works on public keys, signatures, hashes and control bodies, the relay, its operators and its backups never need, and never get, a key that decrypts content (principle 1). Nothing in this protocol carries a space key, a person key or plaintext to the relay: key envelopes and person-key records arrive wrapped, and the relay has no key to unwrap them.

**Revoked members.** A revocation, which removes a member or lowers its role, names a cutoff: the subject's highest author sequence that the revoking admin held when it wrote the revocation ([RFC 0008](0008-membership.md#the-cutoff)). The relay accepts the subject's changes at or below the cutoff, whenever they arrive and whoever forwards them, and refuses those above it that need the revoked right. Both numbers sit in signed, readable places, the cutoff in a control body and the author sequence in every header, so the relay and every member reach the same verdict without the encrypted clock (decision R7). Two limits follow. A change the subject wrote before the revocation, but that the revoking admin had not yet received, lies above the cutoff: the relay refuses it, and members that receive it directly reject it too. And a revoked member can write a new change under a sequence number at or below its cutoff that it already used. That is an equivocation; it is accepted, and members detect and report it.

## What the relay sees

The relay holds no keys. Here is everything that can be learned anyway, stated plainly (principle 4). The relay itself learns it from the log and from the protocol, the same on every backend. The server the relay runs on, on AWS the provider and the account's operators, sees all of that and more, and what exactly depends on how the account is configured.

**What the relay learns from the log** (as RFC 0002, RFC 0003 and RFC 0008 define it):

- Which member ids are in which space, their public keys and member certificates, so which members belong to one account across spaces, and the sub-agent tree.
- Each member's role and when it changes, the cutoff of every revocation, which control changes each admin had seen when it wrote, and the generation graph. The relay reads them because it applies the membership rules ([RFC 0008](0008-membership.md#how-the-relay-applies-the-rules), decision M11).
- For each key envelope, the member id it is for and the generation it carries, so when the space key rotates and who received each generation (RFC 0003).
- For each person-key record, its person-key id, its generation and the member id that wrote it, so how many people have personal data in the space, and when a record is added, re-wrapped or shredded. Never who the person is (RFC 0003).
- For each change: the space, the author's member id and sequence number, the suite, the key generation, the size of the frame and when the relay received it.
- For each shred entry, which person-key id was destroyed, which accounts signed it off, and which members acknowledged it (RFC 0003).

**What the relay learns from the protocol**:

- **Who is online, and when.** Each connection is authenticated as one member id, so the relay knows when each member connects, for how long, and from which network address.
- **Which software each member runs**, from the optional client string in `hello`, such as `ncdb-rs/0.3`, and from the protocol versions it offers. Together with the network address, this can fingerprint a device.
- **What each member follows.** Which spaces each connection subscribes to, and from which cursor. A cursor shows how far behind a member is, and so roughly how long it was offline.
- **Who exchanged changes directly.** When a member uploads a change another member authored, the relay learns that the two met in island mode or forwarded through each other.
- **Upload and download sizes** of large changes, and when they happen.
- **The rhythm of work**, per member and per space: bursts of small changes look like typing; a large push after days of silence looks like a member coming back from offline work.

**What the server sees: AWS, and the account's operators.** Everything the relay learns, because the relay's code and tables run in the account, and in addition:

| What | Where | What it depends on |
| --- | --- | --- |
| Every connection, its network address, and the size and time of every message. | API Gateway | Seen while the connection lasts. It is written down only if API Gateway access logging is on; the log format decides whether the source address is in it, and decision R11 how long it stays. |
| The log of frames, the membership table and the person-key store. | DynamoDB and S3 | Held for as long as the relay keeps them. |
| Which member id was connected when, and to which spaces it subscribed. | DynamoDB, the `connections` table | Decision R9: removed when the connection closes by default; kept for a period the operator chooses (default 30 days) if the operator turns that on. |
| What the relay logs: member ids, space ids, change ids, sizes, error codes and times. Never a frame, a payload, a key envelope, a person-key record or other key material (GENESIS.md, "Never write content, keys or personal fields into logs"). | CloudWatch Logs | Decision R11: kept 14 days by default, or any longer period the operator sets. |
| Earlier states of the tables, including person-key records a shred deleted. | DynamoDB point-in-time recovery | Decision R10: on, for 35 days. The records are wrapped; RFC 0003 states what follows from that. |
| Earlier versions of frames and uploads. | S3 versioning | Off (decision R10). An operator who turns it on keeps deleted objects until their versions expire. |
| Which role or function called which AWS API. | CloudTrail | Management events are recorded by default. Data events, one per DynamoDB or S3 access, only if the operator turns them on. They name the relay's functions, not members. |

**What neither can see**: any content, the ordering metadata (clock, dependencies), whether a sealed change is content or a work order, the model, and which objects or fields a change touches (RFC 0002). A network address is personal data under the GDPR. The relay holds it while a connection is open, and the server keeps it only as decisions R9 and R11 set.

## The backend interface

All checks live in one relay core, a Rust crate that every deployment links: a WebSocket server for SQLite and Postgres, Lambda functions on AWS. A backend only stores. It never checks a signature, a membership rule or a frame, so a backend cannot be the one that gets a check wrong, and the backends stay interchangeable.

```rust
/// What every relay backend implements: SQLite, Postgres, DynamoDB with S3.
/// The relay core calls it only after every check has passed.
#[async_trait]
pub trait RelayStore {
    /// Append `entries` at positions head + 1, head + 2, ... if and only if the
    /// space's head is still `expected_head`, and apply `effects` (membership
    /// table updates, person-key record deletions for shred entries) in the
    /// same atomic step. Returns `HeadMoved(actual)` when another append got
    /// there first; the core then re-checks and retries. A change id that is
    /// already in the space's log is never appended twice: `Duplicate(position)`.
    async fn append(
        &self,
        space: SpaceId,
        expected_head: Position,
        entries: Vec<NewEntry>,   // change id, accepted at, author, kind, frame bytes
        effects: Vec<Effect>,
    ) -> Result<AppendOutcome, StoreError>;

    /// The entries after `after`, in position order, at most `limit`.
    async fn read(&self, space: SpaceId, after: Position, limit: u32)
        -> Result<Vec<Entry>, StoreError>;

    /// The head of the space's log, or None if the relay holds no such space.
    async fn head(&self, space: SpaceId) -> Result<Option<Position>, StoreError>;

    /// The position of a change id in the space's log, if it is there.
    async fn find(&self, space: SpaceId, change: ChangeId)
        -> Result<Option<Position>, StoreError>;

    /// The membership table: `resolve` of RFC 0008 over the space's control
    /// changes. Per member id that was ever in the space: its public keys and
    /// certificate, its grants with their from generations, the revocations
    /// with their cutoffs; per space: the generation graph. Written only
    /// through `append` effects.
    async fn member(&self, space: SpaceId, member: MemberId)
        -> Result<Option<MemberRecord>, StoreError>;
    async fn members_of(&self, member: MemberId) -> Result<Vec<SpaceId>, StoreError>;

    /// The person-key store (RFC 0003): opaque wrapped records.
    async fn person_keys(&self, space: SpaceId) -> Result<Vec<PersonKeyRecord>, StoreError>;
    /// Stores records the core has matched to their person-key entries,
    /// each replacing the record of its person-key id, if and only if every
    /// put still replaces the generation it names, and nothing otherwise:
    /// one atomic step, the way an append is conditional on the head. The
    /// puts apply in order, so a later put of one id replaces the generation
    /// of the earlier. Returns `Moved` when another put got there first; the
    /// core then re-checks and retries. There is no delete request: a record
    /// goes only through `delete_person_keys` once its shred entry is stable
    /// (RFC 0008, M20).
    async fn put_person_keys(
        &self,
        space: SpaceId,
        puts: Vec<PersonKeyPut>,
    ) -> Result<PutOutcome, StoreError>;

    /// The relay epoch, and setting a new one, which a restore must do
    /// before the relay serves any request.
    async fn epoch(&self) -> Result<Epoch, StoreError>;
    async fn new_epoch(&self) -> Result<Epoch, StoreError>;

    /// Delete the person-key records of shred entries that became stable.
    async fn delete_person_keys(&self, space: SpaceId, ids: Vec<PersonKeyId>)
        -> Result<(), StoreError>;

    /// Large frames: an upload URL, and reading what was uploaded.
    async fn begin_upload(&self, space: SpaceId, size: u64) -> Result<Upload, StoreError>;
    async fn take_upload(&self, id: UploadId) -> Result<Vec<u8>, StoreError>;
}

/// A person-key record to store, and the generation of the stored record it
/// replaces, which the core checked it against: None when no record of its
/// person-key id is stored.
pub struct PersonKeyPut {
    pub record: PersonKeyRecord,
    pub replaces: Option<u64>,
}

/// What a put of person-key records did.
pub enum PutOutcome {
    /// Every record is stored.
    Stored,
    /// The stored record of `id` is not the one a put replaces: its
    /// generation is `stored`. Nothing was stored.
    Moved { id: PersonKeyId, stored: Option<u64> },
}

/// Delivers appended entries to subscribed connections. In-process for a
/// WebSocket server; through API Gateway's connection API on AWS.
#[async_trait]
pub trait Notifier {
    async fn subscribe(&self, conn: ConnectionId, space: SpaceId) -> Result<(), StoreError>;
    async fn unsubscribe(&self, conn: ConnectionId, space: SpaceId) -> Result<(), StoreError>;
    async fn publish(&self, space: SpaceId, entries: &[Entry], head: Position)
        -> Result<(), StoreError>;
}
```

The rules every backend must keep:

- **Dense positions, without a gap and without a position used twice.** The append is conditional on the head, so two relay processes appending to one space at the same time cannot both win. The loser gets `HeadMoved` and the core retries.
- **One change id, one position.** The check for a duplicate and the append are one atomic step.
- **An append and its effects are atomic.** A control change and the membership table change together or not at all. The deletion of a person-key record is not such an effect: it waits until its shred entry is stable, and the core requests it then (RFC 0008, M20).
- **A person-key put is conditional on the stored generation.** Each put names the generation of the stored record it replaces, none when no record of its person-key id is stored. The backend stores the request's records only if every one still replaces that generation, in one atomic step, and otherwise stores none and answers `Moved`. The core then reads the log and the records again, checks again and retries, up to the append's 64 tries before it answers `unavailable`. The condition is on the generation alone, because whether a record is acceptable depends on nothing else of the stored record: a re-wrap under the same generation stays acceptable whichever record of that generation is stored.
- **Exact bytes.** A backend returns the frame byte for byte as it was appended.
- **Read your writes.** A read after a successful append sees the appended entries.
- **A new epoch after any loss.** A backend restored from a backup, or one that may have lost appends for any other reason, draws a new epoch before it serves a request.

One conformance test suite, run against every backend, holds these rules, so a new backend is interchangeable when it passes it. How each backend keeps them:

| Rule | SQLite | Postgres | DynamoDB with S3 |
| --- | --- | --- | --- |
| Conditional append | One write transaction; the head is a row in `spaces`. | `UPDATE spaces SET head = head + n WHERE id = $1 AND head = $2`, then the inserts, in one transaction. | `TransactWriteItems`: a condition on the head item, plus the entry and change-id items (see [The AWS backend](#the-aws-backend)). |
| One change id, one position | A unique index on (space, change id). | A unique index on (space, change id). | A change-id item per change, put with `attribute_not_exists`, in the same transaction. |
| Conditional person-key put | One write transaction that reads each stored generation before it writes. | In one transaction, `UPDATE person_keys SET ... WHERE space = $1 AND id = $2 AND generation = $3`, or an insert where none is stored; a row count of zero rolls back and answers `Moved`. | `TransactWriteItems` with a condition expression on each item's generation in `person_keys`, or `attribute_not_exists` where none is stored. |
| Large frames | Kept inline. | Kept inline. | Frames above 8 KiB in S3, the entry points to them. |
| Notifier | In process. | In process, plus `LISTEN`/`NOTIFY` across relay processes. | API Gateway's connection API. |

## The AWS backend

The first backend runs serverless in the AWS account NoCentralDb-Prod (decided by Carsten, see `factory/DECISIONS.md`), in one region (decision R8).

```text
member ──WebSocket──▶ API Gateway (WebSocket API)
                          │  $connect, $disconnect, $default routes
                          ▼
                      Lambda "relay" (Rust, arm64, the relay core)
                          │                         │
                          ▼                         ▼
                      DynamoDB                  S3 bucket "frames"
                      (log, changes, spaces,    (frames above 8 KiB,
                       members, person keys,     uploads)
                       connections)
                          │
                          └──▶ API Gateway connection API: live entries to subscribers
```

### Services

| Service | What it does | Why this one |
| --- | --- | --- |
| API Gateway, WebSocket API | Terminates TLS and WebSockets, holds the connections, invokes Lambda for each message, and lets Lambda send to any connection. | No server to run, and nothing to pay while nobody is connected. |
| Lambda | Runs the relay core, one invocation per incoming message. Rust on arm64, 256 MB. | The same core as every other deployment; pays per message. |
| DynamoDB, on-demand | Holds the log, the membership table, the person-key store and the connection table. | Conditional transactions give dense positions and atomic effects; no capacity to plan; point-in-time recovery. |
| S3 | Holds frames above 8 KiB and uploads. | Large items are cheaper there, and DynamoDB items stop at 400 KB. |
| CloudWatch | Logs (ids, sizes, codes, times; no content and no keys; retention per decision R11), metrics, alarms, and a budget alarm. | Built in. |

No VPC, no NAT gateway and no long-running server: all of these cost money while idle.

### Tables and keys

Every id is stored as bytes; positions are numbers. "PK" is the partition key, "SK" the sort key.

| Table | PK | SK | Attributes | Notes |
| --- | --- | --- | --- | --- |
| `log` | space id | position | change id, accepted at, author member id, kind, frame (inline) or S3 key and size | One item per change. A pull is one `Query` on the space from `position > after`, in order. |
| `changes` | space id | change id | position | Makes "one change id, one position" a condition inside the append transaction. |
| `spaces` | space id | — | head, suite, created at, creator, quotas | The head is the condition of every append. One reserved item holds the relay epoch. |
| `members` | space id | member id | signing key, agreement key, certificate, grants with their from generations, revocations with their cutoffs | Written only by an append's effects. A global secondary index on member id answers "which spaces is this member in". The space's generation graph is one item per generation under the same space id. |
| `person_keys` | space id | person-key id | generation, writer member id, wrapped record, signature | Written only after the core checked it against its person-key entry (RFC 0003, decision 14; RFC 0008), in one `TransactWriteItems` with a condition expression on the generation it replaces; deleted only when its shred entry is stable (RFC 0008, M20). |
| `connections` | connection id | space id | member id, connected at, expires at (TTL) | One item per subscription, plus one for the connection itself. A global secondary index on space id finds a space's subscribers. TTL removes what a missed `$disconnect` leaves behind. An operator who sets decision R9 to B keeps the records after the connection ends, for the period it chose. |

An append of n changes is one `TransactWriteItems` with 2n + 1 items, plus effects: the conditional update of the head in `spaces`, and for each change a put in `log` and a put in `changes`, each with `attribute_not_exists`. DynamoDB allows 100 items per transaction, so a push of 32 changes fits with room for effects. A transaction that fails on the head condition is retried by the core after re-reading the head; a transaction that fails on a change id tells the core which change is a duplicate.

### How a space's log is partitioned

A space's log is one **item collection** in `log`: all items share the space id as partition key and are sorted by position. DynamoDB splits a large item collection across physical partitions by sort-key range on its own, and with no local secondary index there is no size limit on it. Pulls are range queries on one key, cheap and in order.

Different spaces spread across partitions by their random ids, so spaces never compete with each other. Within one space, every append updates the same head item, so a space's appends are serialised. Contended transactions on one item run at tens to hundreds per second; the implementation measures the exact figure. That is far above what a team's space writes, and members batch their changes into pushes. A space that needs more is a space that should be split into smaller spaces, which is how NoCentralDB scales rights anyway (RFC 0001).

### How large changes go to S3

- A frame above **8 KiB** is stored in S3 under `frames/<space id>/<change id>`, and its `log` item holds the key and the size. Below 8 KiB, the frame sits inline in the item. At current prices, a transactional DynamoDB write costs about as much per 4 KiB as one S3 PUT. Reads tip the balance further towards inline, because every member that pulls a frame from S3 pays for a GET, so the line sits at 8 KiB. Nearly every change stays inline (padding keeps most changes under 1 KiB), while large pastes go to S3. The threshold is a setting of the AWS backend, not part of the protocol.
- The relay writes the S3 object **before** the DynamoDB transaction. If the transaction fails, the object is an orphan, which a daily sweep deletes; a `log` item never points to a missing object.
- Uploads go to `uploads/<upload id>` through a presigned PUT URL that expires after 15 minutes, with the size fixed in the signature. The relay checks the frame, copies it to its `frames/` key and deletes the upload. A lifecycle rule deletes uploads older than one day.
- Downloads are presigned GET URLs, valid for 5 minutes, so large frames never pass through Lambda on the way out.
- The bucket blocks all public access. S3's default server-side encryption is on, which protects nothing the frame's own encryption does not, but costs nothing.

### Delivery to subscribers

After a successful append, the same Lambda invocation looks up the space's subscribers in `connections` and posts the new entries to each, through API Gateway's connection API. Delivery is best-effort: a post that fails is not retried. A member that misses an entry sees the gap at the next entry it receives, because entries carry dense positions and the head, and pulls the range. If no further entry comes, it catches up when it reconnects, which happens at the latest after 2 hours, when API Gateway closes every connection. Decision R3 compares this with fan-out through DynamoDB Streams.

### Restores and the relay epoch

A restore from a backup, such as DynamoDB point-in-time recovery, brings back the relay as it was at the restore point. Every append after that point is gone from the relay, but not from the members. Without care, this breaks RFC 0002's dense positions:

- New appends would reuse the lost positions for different changes. A member whose cursor already passed those positions would skip the new changes and never notice.
- Members would not re-push their own lost changes, because those already carry a relay position.
- Person-key records that a later shred deleted would come back.

So a restore always ends with a **new relay epoch**, before the relay serves any request:

1. The operator restores the tables, and the restore procedure draws a new epoch. The relay refuses every request until it has done so.
2. The relay re-applies the effects of every shred entry in the restored log that is stable, so no record survives that a stable shred in the restored log deleted.
3. Each member sees the new epoch in `welcome`, or gets `epoch_changed` on a `pull` or `subscribe`. It then treats every position it stored for this relay as unconfirmed, pulls each space from 0 and folds nothing new twice, because it recognises changes by their ids. Finally it pushes every change the relay lacks, in its local order: its own, and the ones it received from others. The relay gives each change a position in the new epoch.
4. Shred entries appended after the restore point come back in step 3, when the first member that holds one re-pushes it. Their effects delete the restored person-key records again.

**What it does not do.** Until a member that holds a lost shred entry reconnects, the relay serves the person-key record it deleted. The record is wrapped under a space key, so only members of the space can open it. A member could equally have kept the key, as RFC 0003 says for relay backups. A restore also costs every member a full pull of every space it belongs to: about the size of the log, once. The same procedure applies to any loss of appends, not only to a restore. Decision R10 weighs this against having no backups at all.

### What it does not do

- **No delivery guarantee faster than the next entry or the next reconnect.** A missed live entry is recovered then, not sooner.
- **One region.** If the region fails, the relay is down, and members work in island mode until it is back (see [Island mode](#island-mode)). A second region would double the bill.
- **No relay-side compaction yet.** Without compaction, storage grows with every change. The compaction RFC will move old ranges to an archive.

## What it costs

These are rough monthly costs, from AWS's us-east-1 list prices as we know them in November 2026. eu-central-1 (Frankfurt) is about 10 to 20 percent more expensive. Before the owner decides, the numbers must be checked in the AWS pricing calculator for the chosen region. Free tiers are left out.

Prices used: API Gateway WebSockets 1.00 USD per million messages (each up to 32 KB) and 0.25 USD per million connection minutes; Lambda on arm64 0.20 USD per million requests and 0.0000133 USD per GB-second; DynamoDB on demand 0.625 USD per million write request units and 0.125 USD per million read request units (a transactional write costs two units per KB), storage 0.25 USD per GB-month; S3 0.005 USD per thousand PUTs, 0.0004 USD per thousand GETs, 0.023 USD per GB-month; data transfer out 0.09 USD per GB after the first 100 GB; CloudWatch Logs 0.50 USD per GB.

One change of about 1 KiB, appended alone, costs 8 write units: 2 for the `log` item (frame and attributes are just over 1 KB), 1 for the `changes` item and 1 for the head update, doubled because it is a transaction. Batching several changes into one push shares the head update.

### A small team

Ten people with two devices each and ten agents, 30 members in 20 spaces of about 8 members each. Together they write 20,000 changes a day, 600,000 a month; each change reaches about 7 other members live.

| Item | Volume per month | Cost (USD) |
| --- | --- | --- |
| API Gateway messages | 0.6 M pushes, 0.6 M answers, 4.2 M live entries, 0.5 M pulls, subscribes and pings | 5.90 |
| API Gateway connection minutes | 30 members, 16 hours a day | 0.22 |
| Lambda | 0.9 M invocations, about 50 ms each | 0.30 |
| DynamoDB writes | 0.6 M changes × 8 units | 3.00 |
| DynamoDB reads | membership and subscriber lookups, pulls: about 1.5 M units | 0.19 |
| DynamoDB storage | 0.8 GB more each month | 0.20 per month of history |
| S3 | 1 % of changes above 8 KiB | 0.10 |
| CloudWatch Logs | about 1 GB | 0.50 |
| **Total** | | **about 10, rising to about 15 after a year of history** |

While nobody writes or connects, the relay costs only its storage: cents.

### 1,000 agents, each reading 1,000 times a minute

The reads are local, so the relay sees none of them: 1,000 × 1,000 reads a minute cost nothing on the relay (use case 4). The cost comes from what the agents write. We assume each agent writes **one change a minute** on average, around the clock, in spaces of about 10 members, all connected, so each change reaches 9 other members live. That is 43.2 million changes a month.

| Item | Volume per month | Cost (USD) |
| --- | --- | --- |
| API Gateway messages | 43.2 M pushes, 43.2 M answers, 389 M live entries, 1 M reconnects | 476 |
| API Gateway connection minutes | 1,000 agents, always connected: 43.2 M | 11 |
| Lambda | 44 M invocations, about 100 ms each with the fan-out | 23 |
| DynamoDB writes | 43.2 M changes × 8 units | 216 |
| DynamoDB reads | about 86 M units | 11 |
| DynamoDB storage | 56 GB more each month | 14 per month of history |
| S3 | 1 % of changes above 8 KiB | 4 |
| Data transfer out | about 390 GB of live entries | 27 |
| CloudWatch Logs | about 20 GB | 10 |
| **Total** | | **about 800, rising by about 14 for each month of history kept** |

The cost grows linearly with writes times the members each write reaches. At ten writes per agent per minute it is about ten times as much, around 8,000 USD a month. Live delivery through API Gateway is more than half of it. For comparison, a central database answering the same 1,000 × 1,000 reads a minute through API Gateway alone would pay about 43,000 USD a month for the 43 billion requests, before the database.

At this scale, a long-running WebSocket server on ECS Fargate behind a load balancer, with the same relay core and the same DynamoDB tables, pays no per-message price: roughly 60 USD for two tasks, 25 USD for the load balancer, plus the same DynamoDB, storage and transfer, about 370 USD in total. It costs about 45 USD a month even when idle, so it loses for a small team. Because the relay core is the same, switching is a deployment change, not a redesign (decision R3).

## Island mode

**When the relay is unreachable**, nothing changes for reads and writes:

- Reads never touched the relay; they come from the local view.
- A write enters the member's local log at once, folded, with no relay position (RFC 0002). The SDK reports how many of the member's changes the relay has not confirmed yet.
- The member retries the connection with exponential backoff and jitter, up to a few minutes between attempts.
- Members that can reach each other directly, on one network, exchange changes with the same messages as with the relay: `push`, and `pull` or `subscribe` with the peer's own local positions as the cursor. A peer runs the same checks as the relay, plus decryption. How peers find each other is a later RFC.
- Membership changes and key rotations made on an island are control changes like any other. RFC 0003 says what happens when two islands rotate at the same time.

**On reconnect**, a member:

1. Authenticates and, for every space, **pulls** from its last confirmed cursor up to the head (from 0, if the relay epoch changed), folding as it goes. It recognises changes it already holds, for example from a peer, by their ids.
2. **Pushes** every change in its log that has no relay position yet, in the order of its local log, in pushes of at most 32: its own changes and the ones it received from peers. Changes the relay already holds come back as `duplicate`, with their position.
3. Records the relay position of every change, and **subscribes** from its new cursor.

Pulling first lets the member see a membership entry that removed it before it pushes, and spares the relay the uploads it already holds. Order on the relay does not matter for the result: every member folds the same set of changes to the same state, in any order (principle 2).

**What island mode does not do.**

- Changes made on an island reach the members on other islands only when a path exists again, through the relay or a peer.
- The relay refuses a revoked author's changes above the revocation's cutoff, also when they were written on an island before the revocation (see [What the relay checks](#what-the-relay-checks)). Members reject them too, so the verdict is the same everywhere; the change stays in the logs of the members that hold it, and the SDK reports it (decision R7, RFC 0008's M3).
- Nothing enforces one global truth at write time, on an island or off it (RFC 0001).
- A relay that comes back after losing data, restored from a backup or empty, draws a new relay epoch, and its members repopulate it by their pushes (see [Restores and the relay epoch](#restores-and-the-relay-epoch)). Members match changes by id (RFC 0002). How a space moves between relays on purpose is a later RFC.

## What this RFC needs from RFC 0002, RFC 0003 and RFC 0008

| Need | From | Status |
| --- | --- | --- |
| The relay can compute a change id, the author, the key generation, the kind and the suite from the frame alone. | RFC 0002 | Met by its header and change ids. |
| Control bodies (membership entries, key envelopes, key chain entries, shred entries) are readable by the relay and carry the public keys and certificate of an added member. | RFC 0002, RFC 0003 | Met: RFC 0003, "Sharing a space key with a new member", step 1 and 3. |
| A key envelope names the member id it is for and its generation, readable by the relay, so that `keys` can return a member's own envelopes. | RFC 0003 | Met: RFC 0003 lists both as clear metadata. |
| The first change of a space is a control change that names the creator as its first member, with its public keys, so the relay can accept a new space. | [RFC 0008](0008-membership.md#the-first-change-of-a-space) | Met: the genesis. |
| The membership rules the relay applies, the person-key entry, the cutoff and when an irreversible effect may happen. | [RFC 0008](0008-membership.md#how-the-relay-applies-the-rules) | Met; this RFC follows its Conflicts points 11 and 12. |
| The member's signing key signs the relay challenge, with its own context string. | RFC 0003 | Met: RFC 0003, decision 13. |
| Person-key entries with a commitment, signed person-key records, and shred entries with a second account's sign-off that the relay can verify. | RFC 0003 | Met: RFC 0003, decision 14. |
| Person-key records carry their person-key id, generation and writer member id outside the wrapping. | RFC 0003 | Met: RFC 0003, decision 14. |

## Rejected options

One line each; the decisions below give the options that remain.

| Option | Why not |
| --- | --- |
| HTTP requests plus server-sent events | Two channels to authenticate and keep in step, and server-sent events carry only text, so frames would need base64. |
| gRPC | Browsers cannot open a bidirectional gRPC stream, so the web SDK would need a proxy and a second protocol. |
| WebTransport | Not available in every browser we target, and not on API Gateway. |
| Bearer tokens from an identity service (Cognito, OAuth) | Adds a party that knows every member's identity and can mint access; the member key already proves who connects. |
| Checks inside each backend | Three implementations of every security check, each able to get one wrong; the backends would drift apart. |
| Positions from timestamps or a database sequence | Timestamps tie and run backwards; sequences leave gaps after a failed transaction, and a gap looks like a lost change (RFC 0002). |
| Positions from DynamoDB Streams' order | Stream order is per shard and not dense; it cannot serve as a cursor. |
| One DynamoDB table for everything | Saves nothing at our size and makes access rules and capacity per purpose harder to reason about. |
| All frames in S3 | An S3 PUT costs more than the DynamoDB write for a 1 KiB change, and every pull would need one GET per change. |
| All frames in DynamoDB | Items stop at 400 KB, and a large frame costs two write units per KB in a transaction. |
| AppSync (GraphQL subscriptions) | A GraphQL schema around opaque bytes adds a layer with nothing to describe, and pricing per operation is no better. |
| A long-running server on EC2 | Costs money while idle, and needs patching; Fargate is the long-running option if one is needed (decision R3). |
| The relay folds membership into its own format, independent of the log | The membership table must be rebuildable from the log; a second source of truth could disagree with the members' fold. |
| The relay refuses equivocating changes | Refusing one of two signed changes makes the relay choose a winner; members detect equivocation themselves (RFC 0002). |
| Unsigned person-key records that any member may overwrite or delete | One member could erase a person's data without the second account's sign-off, or swap a key unnoticed (RFC 0003, decision 14). |
| Restoring a backup under the same epoch | Positions would be reused for different changes, and members past them would skip changes silently. |
| The relay judges a removed member's change by when it arrived, or by its generation | The relay's order is not the order members see on an island, and the relay cannot see when a change was written; the cutoff decides instead (decision R7, RFC 0008's M3). |

## Conflicts with GENESIS.md and RFC 0001

This RFC agrees with GENESIS.md and RFC 0001 except in the points below. The owner ruled on each (Carsten, 2026-11-08). NOC-47 made the changes they need in other documents, and RFC 0002 states its part in [What the relay can see, and what it cannot](0002-change-and-log-format.md#what-the-relay-can-see-and-what-it-cannot) and [On the relay](0002-change-and-log-format.md#on-the-relay).

1. **The relay sees more than RFC 0002's list.** RFC 0001, GENESIS.md, use case 2 and the FAQ link to [RFC 0002's list](0002-change-and-log-format.md#what-the-relay-can-see-and-what-it-cannot). [What the relay sees](#what-the-relay-sees) adds what the protocol shows (when each member is online, its client, its cursors, upload sizes), the person-key records, what the membership rules show (RFC 0008, M11), and what the server sees, with the AWS setting each item depends on. Ruled: this RFC separates what the relay learns from what the server sees, as above. Resolved (NOC-47): RFC 0002's list links to this RFC's, and RFC 0001, GENESIS.md, use case 2 and the FAQ link both.
2. **Use case 4, "at no cost".** The reads are free; the writes are not ([What it costs](#what-it-costs)). Accepted. Resolved (NOC-47): use case 4 says "reads at no cost" and what the writes cost, with a link to [What it costs](#what-it-costs).
3. **Use case 5, members on the same network.** This RFC defines the messages peers exchange, but not how they find each other. Accepted. Resolved (NOC-47): use case 5 says how peers find each other is missing until the peer RFC exists.
4. **RFC 0002, who uploads.** This RFC requires the uploader to be an authenticated current member of the space; the author may still be any member. This narrows RFC 0002 without contradicting it. Accepted. Resolved (NOC-47): RFC 0002 states the uploader rule in its section ["On the relay"](0002-change-and-log-format.md#on-the-relay).

## Summary of the choices

| Question | This RFC's answer |
| --- | --- |
| How does a member talk to the relay? | One WebSocket connection, CBOR messages, authenticated by a challenge signed with its member key. |
| What can it do? | Push changes, pull from a cursor, subscribe to a space, fetch its key envelopes, read and write the person-key store. |
| What does the relay check? | Canonical frames, signatures, membership of author and uploader, membership rules, duplicates. All with public data. |
| What does it see? | Membership, who writes when and how much, who is online from where, who follows what, how many persons a space holds keys for. The server on AWS sees more, as its settings allow. No content. |
| How do backends stay interchangeable? | All checks in one core; backends implement one storage interface and pass one conformance suite. |
| How does it run on AWS? | API Gateway WebSockets, Lambda, DynamoDB on demand, S3 for frames above 8 KiB, in eu-central-1. |
| What does it cost? | About 10 USD a month for a small team; about 800 USD for 1,000 agents writing once a minute; nothing for local reads. |
| What if the relay is down? | Members keep working; on reconnect they pull, then push what the relay has not confirmed. |
| What if the relay is restored from a backup? | It draws a new epoch; members reset their cursors, pull and push what it lacks, and lost shreds are re-applied. |
| Who may change the person-key store? | A current member, with a signed record for a person key the log names (RFC 0003, decision 14); only a signed-off shred entry deletes one. |

## Open questions

None. The owner has decided every question this RFC asked.

## Decisions

GENESIS.md reserves the wire format, anything that writes to AWS and anything that raises the bill for the owner. Each decision this RFC asked of the owner, in short form: what was chosen, by whom and when, why, and the options not taken. The design above describes the chosen options.

### R1. The transport

Decided (Carsten, 2026-11-08): one WebSocket connection, CBOR messages.

Why: one channel, one authentication and binary frames, in every browser and on API Gateway.

Not taken:

- HTTP requests for push and pull, server-sent events for subscriptions.
- gRPC, with a proxy for browsers.

### R2. How a connection authenticates

Decided (Carsten, 2026-11-08): a challenge signed with the member key, per connection.

Why: no new party learns who the members are, no token can be stolen and replayed, and the SDK already holds the key. A developer's own adapter may still put its own authentication step, such as Cognito, in front of the connection.

Not taken:

- Bearer tokens from an identity service such as Cognito.

### R3. Compute on AWS

Decided (Carsten, 2026-11-08): API Gateway WebSockets and Lambda, with live delivery from the appending Lambda.

Why: about 10 USD a month for a small team, nothing while idle, about 800 USD a month for 1,000 agents writing once a minute ([What it costs](#what-it-costs)).

Not taken:

- As chosen, with live delivery through DynamoDB Streams and a second Lambda.
- A WebSocket server on ECS Fargate behind a load balancer, with the same core and tables: at least 45 USD a month, and about 370 USD in the 1,000-agent case.

### R4. The protocol limits

Decided (Carsten, 2026-11-08): 128 KiB per message and 32 changes per push, on every backend.

Why: a member never behaves differently by backend.

Not taken:

- Limits per backend.

### R5. The largest change

Decided (Carsten, 2026-11-08): 16 MiB.

Why: large enough for rich-text pastes with images inlined, small enough that one change cannot block a member's pull. Files are a later RFC.

Not taken:

- 4 MiB.
- No limit, with multipart uploads.

### R6. Who may create a space on a relay

Decided (Carsten, 2026-11-08): members whose member certificate comes from an account on the relay operator's allowlist.

Why: a relay must not become free storage for strangers and a bill for its operator.

Not taken:

- Anyone who can reach the relay.
- Anyone with an invitation code from the operator.

### R7. Changes of a revoked member that arrive late

Decided (Carsten, 2026-11-08), through RFC 0008's decision M3: accept the revoked author's changes at or below the revocation's cutoff, refuse those above it.

Why: it needs only headers, so the relay and every member reach one verdict. The limits are in [What the relay checks](#what-the-relay-checks).

Not taken:

- Refuse every change of a removed author from the removal on, by the relay's order: members on an island do not have that order.
- Accept changes of a removed author under key generations before the removal: a removed member could write under an old generation forever.

### R8. The AWS region

Decided (Carsten, 2026-11-08): eu-central-1, Frankfurt.

Why: closer to the first users, and the metadata the relay keeps is personal data under the GDPR. It costs about 10 to 20 percent more than us-east-1.

Not taken:

- us-east-1, N. Virginia.

### R9. What the relay keeps about connections

Decided (Carsten, 2026-11-08): configurable. By default, a connection record exists only while the connection is open, removed by `$disconnect` or by TTL; an operator may keep records after the connection ends and choose the retention period, 30 days by default.

Why: an operator may need the records for abuse investigation, and [What the relay sees](#what-the-relay-sees) names the setting either way.

Not taken:

- Connection records always kept after the connection ends.

### R10. Backups of the relay

Decided (Carsten, 2026-11-08): DynamoDB point-in-time recovery for 35 days, S3 versioning off; every restore draws a new relay epoch.

Why: a restore brings back the membership table and the person-key store before any member reconnects. Its price: a shredded person-key record stays in the backups for up to 35 days, wrapped, as RFC 0003 states. It costs 0.20 USD per GB-month, about 0.20 USD a month for a small team.

Not taken:

- No backups; members repopulate a lost relay.

### R11. How long CloudWatch keeps the relay's logs

Decided (Carsten, 2026-11-08): 14 days for the relay's logs and API Gateway's access logs as the default, and the operator may set any longer period.

Why: in every setting, the logs hold ids, sizes, error codes, times and, in access logs, network addresses, never content or key material. It costs under 1 USD a month for a small team.

Not taken:

- 90 days.
- 1 day, and no network addresses in any log.

### R12. The keys of each message

Decided (Carsten, 2026-11-10), on NOC-98: the error map's example as the rule. The type is text at key 1, the request id at key 2 or absent, then each field at keys 3, 4, … in this RFC's order; a nested map numbers its fields from 1 in this RFC's order; types and error codes stay text. [The keys of each message](#the-keys-of-each-message) applies it to every message. The relay server (NOC-89) needed the keys, which the first version of this RFC left open.

Why: one rule a reader derives from the RFC, compact CBOR, and the same shape as the error map.

Not taken:

- Text keys for every field, such as `"space"` and `"frames"`: easier to read in a capture, but larger in every message and unlike the error map.
- Numbers picked by hand for each message, with gaps for future fields: CBOR maps already grow by adding keys at the end, and hand-picked numbers leave more to review.

The protection of the person-key store and the key that authenticates a connection are decided in RFC 0003, in its [decision 14](0003-keys-and-cryptography.md#14-integrity-of-the-person-key-store) and [decision 13](0003-keys-and-cryptography.md#13-how-a-member-authenticates-to-the-relay), because they are key handling.

## History

- **2026-11-08** · NOC-13 · The first version, with the decisions R1 to R11, which the owner ruled on, and the conflicts with GENESIS.md and RFC 0001 (#37).
- **2026-11-09** · NOC-47 · The conflicts with GENESIS.md and RFC 0001 say where each was resolved (#64).
- **2026-11-09** · NOC-55 · How to give feedback on this RFC (#72).
- **2026-11-09** · NOC-35 · The first line drops the draft number of a decided RFC (#76).
- **2026-11-10** · NOC-106 · [The keys of each message](#the-keys-of-each-message), with decision R12, which the owner decided as recommended on NOC-98 (#113).
- **2026-11-10** · NOC-101 · Rewritten as one complete text: the decisions in short form, an Open questions section, and a status that says what is built (#117).
- **2026-11-10** · NOC-142 · [The backend interface](#the-backend-interface) draws the person-key put conditional on the stored generation, and its rules and the AWS backend say how each backend keeps it, as the relay is built since #130 (NOC-118).
