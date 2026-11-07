# RFC 0007: The relay protocol and its AWS backend

REQUEST FOR COMMENTS · DRAFT 0.1 · NOVEMBER 2026 · ticket NOC-13

This RFC defines the relay: the protocol members use to push and pull a space's changes and to follow them live, the checks the relay runs before it appends a change, the interface every relay backend implements, and the first backend, serverless on AWS with DynamoDB and S3.

It builds on [RFC 0001](0001-nocentraldb.md), above all [the design in one page](0001-nocentraldb.md#the-design-in-one-page), on the principles in [GENESIS.md](../../factory/GENESIS.md), and on [use cases](../use-cases.md) 2, 4, 5 and 7. It uses the change, the log and the cursor of [RFC 0002](0002-change-and-log-format.md) (NOC-6, decided) and the keys, membership entries, key envelopes and person-key store of RFC 0003 (NOC-7), without redefining them. RFC 0003 is an open draft at the time of writing. Where this RFC needs something from either, it says so in [What this RFC needs from RFC 0002 and RFC 0003](#what-this-rfc-needs-from-rfc-0002-and-rfc-0003).

**Status.** A draft for the owner's decision. GENESIS.md reserves the wire format for the owner, and anything that writes to AWS or raises the bill. Nothing here is built or deployed. The open decisions are in [Decisions for the owner](#decisions-for-the-owner), each with options, a recommendation and its cost. Until the owner decides, the design below describes the recommended option of each decision.

## What this RFC does not cover

- **The bytes of a change and of the log.** RFC 0002. This RFC moves frames as opaque bytes, except for the header fields the relay reads.
- **Cryptography and keys.** RFC 0003. This RFC uses the member key to authenticate a connection, with its own context string, and invents no primitive.
- **Membership rules and roles.** Who may add or remove whom is a later RFC. The relay applies those rules; this RFC only says where.
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
- **Backends are interchangeable.** All checks live in one relay core. A backend only stores: it appends at the next position if nobody else did, reads a range, and keeps the membership table and the person-key store. SQLite, Postgres and DynamoDB with S3 all implement the same interface and pass the same test suite.
- **On AWS, it costs nothing while nobody writes.** API Gateway WebSockets, Lambda, DynamoDB and S3. A small team pays about 10 dollars a month. 1,000 agents that each read 1,000 times a minute pay nothing for the reads, because reads are local; their writes cost about 800 dollars a month at one write per agent per minute.
- **Without the relay, members keep working.** Writes go into the local log at once. On reconnect, a member pulls what it missed and pushes what the relay has not confirmed. The relay recognises changes it already holds by their ids. A relay restored from a backup announces a new epoch, so no member trusts a position the restore undid.

## The protocol

### Transport and encoding

A member talks to the relay over one **WebSocket** connection (RFC 6455) with TLS. Every message is one binary WebSocket message holding one CBOR map, encoded with RFC 0002's deterministic profile. A message's key 1 names its type, and key 2 holds a request id that the member chooses, so that answers can arrive out of order. Messages the relay sends on its own, such as new entries for a subscription, carry no request id.

WebSockets work in every browser we target, natively on every platform, and through API Gateway on AWS, so the same protocol runs everywhere (principle 7). See decision R1 for the alternatives.

**Limits that hold on every backend.** A message is at most 128 KiB, because that is API Gateway's limit, and a protocol that behaves differently by backend would make backends not interchangeable. A change whose frame is larger travels through an upload (see [Large changes](#large-changes)). A push carries at most 32 changes. These numbers are protocol constants, announced in the `welcome` message, so that a future relay can raise them.

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
- `auth` carries the member's two public keys and an Ed25519 signature, with its member signing key (decision R13), over `H("nocentraldb/relay-auth" || relay id || nonce)`. The context string keeps this signature from ever being mistaken for the signature of a change. Binding the relay id keeps one relay from replaying a member's answer against another.
- The relay computes the member id from the two public keys, as RFC 0003 defines it, and verifies the signature. From then on the connection acts as that member id, and only as it.
- `welcome` carries the relay epoch. A member that stored positions under another epoch of this relay treats them all as unconfirmed (see [Restores and the relay epoch](#restores-and-the-relay-epoch)).

A connection proves possession of a member key, nothing more. Whether that member may read or write a space is checked per request, against the space's membership, every time.

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

record = {person-key id, generation, wrapped, writer member id, entry: change id}
```

A wrapped person key is opaque to the relay: a person key wrapped under a space key generation (RFC 0003). The relay cannot check what is inside, so the store is protected by tying every record to the log (decision R12):

- **Every record is announced in the log.** A member that writes or re-wraps a record first appends a **person-key entry**, a control change that names the person-key id, the generation and the SHA-256 hash of the wrapped bytes. The entry is signed like every change, so its author is known.
- **The relay accepts a record only with its entry.** `put-person-keys` must name an entry already in the space's log, whose author is the uploader, whose person-key id and generation match, and whose hash matches the wrapped bytes. Otherwise the relay answers `bad_record`. A record replaces an older one only with a higher generation.
- **Members accept a record only with its entry.** A member checks the hash against the entry in its own copy of the log before it unwraps a record, so a relay that swaps records is caught, and the entry names who wrote each record.
- **Nothing deletes a record except a shred entry.** There is no delete request. The relay deletes a record only when it appends a valid shred entry for its person-key id, in the same backend transaction as the append, so a shred and the deletion cannot come apart. A shred entry carries the second account's sign-off, signed with that account key (RFC 0003). The relay verifies it against the account certificates in the membership table. A member therefore cannot erase a person's data alone.

**What this does not prevent.** A member of the space can still announce and upload a wrongly wrapped key under its own name. Every member then fails to unwrap it and reports it, and the entry names the member that wrote it. A member that holds the space key could equally have leaked the person key itself (RFC 0003).

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
| `bad_signature` | The signature does not verify against the author's signing key. | Keep the change marked as rejected (RFC 0003); never retry. |
| `author_not_member` | The author is not a member of the space at the change's key generation, or was removed. | Keep the change marked as rejected. |
| `membership_rule` | A control change breaks the membership rules (the later membership RFC). | Keep the change marked as rejected. |
| `epoch_changed` | The cursor belongs to an earlier relay epoch, for example after a restore. | Treat every stored position of this relay as unconfirmed, pull from 0, push every change the relay lacks (see [Restores and the relay epoch](#restores-and-the-relay-epoch)). |
| `bad_record` | A person-key record has no matching person-key entry in the log, or would replace a newer generation. | Append the entry first; never retry the same record without one. |
| `compacted` | The cursor points before a range moved into the archive (the compaction RFC). | Fetch the archive, then pull from its end. |
| `bad_cursor` | The cursor is negative or above the head. | Pull from 0 or from the last confirmed position. |
| `rate_limited` | The member or the space exceeded its quota. | Retry after the given time. |
| `unavailable` | The backend failed or timed out. | Retry with backoff. Nothing was appended. |

A change rejected by the relay is not dropped by the member. It stays in the member's log, marked as rejected, and the SDK reports it (principle 3). A `rejected` result for one change says nothing about the others in the same push.

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
| The author is a member at the change's key generation and has not been removed. | The membership table: when each member was added and removed, and the space's key generations. | As above: control changes, readable by design. |
| A control change obeys the membership rules. | The control body, which RFC 0002 makes readable to the relay. | Control bodies carry ids, public keys, wrapped keys and shred ids, never content. A key envelope is HPKE ciphertext to one member; the relay stores it and cannot open it. |
| A person-key record matches its person-key entry. | The SHA-256 hash of the wrapped bytes against the hash in a signed entry in the log. | A hash of opaque bytes. The relay never unwraps the record. |
| A shred entry carries the second account's sign-off. | The account signature in the shred entry, against the account certificates in the membership table. | Signature verification uses only public keys. |
| The change is not already in the log. | The change id, which the relay computes from the frame (RFC 0002). | A hash of the bytes. |
| The reader may read the space. | The membership table, on every pull, subscribe, keys and person-keys request. | As above. |

What the relay **does not** check, because it cannot without reading content, and what follows from that:

- **Whether a sealed payload decrypts**, or what it says. A member that writes garbage under a valid signature gets it appended. Every other member detects it on decryption, keeps it marked as rejected, and the signature names the author (RFC 0003).
- **The author's sequence numbers.** Two changes with the same author and sequence are an equivocation (RFC 0002). The relay appends both, because both are validly signed and refusing one would make the relay pick a winner. Members detect it.
- **The order of dependencies.** They are encrypted (RFC 0002). The relay appends in arrival order, and members fold in any order.

Because every check works on public keys, signatures, hashes and control bodies, the relay, its operators and its backups never need, and never get, a key that decrypts content (principle 1). Nothing in this protocol carries a space key, a person key or plaintext to the relay: key envelopes and person-key records arrive wrapped, and the relay has no key to unwrap them.

**Removed members.** From the position of a membership entry that removes a member, the relay refuses every change that member authored, also changes it wrote earlier under an older generation and that another member forwards later. The relay cannot tell when such a change was written, because the clock is encrypted, so it cannot accept "old" changes without also accepting new ones a removed member backdates. The cost is plain: a change a member wrote shortly before its removal, which never reached the relay, reaches other members only by direct exchange (decision R7).

## What the relay sees

The relay holds no keys. Here is everything it learns anyway, stated plainly (principle 4). RFC 0002 lists what the relay sees in the log; this list adds what it sees through the protocol, and what AWS sees on top.

**From the log** (as RFC 0002 lists it):

- Which member ids are in which space, their public keys and member certificates, and when members are added and removed.
- That a key envelope exists for each member id and generation, and when the space key rotates.
- For each change: the space, the author's member id and sequence number, the suite, the key generation, the size of the frame and when the relay received it.
- That a shred entry destroyed a person key, the person-key id, and which members acknowledged it.

**From the protocol**:

- **Who is online, and when.** Each connection is authenticated as one member id, so the relay knows when each member connects, for how long, and from which network address.
- **Which software each member runs**, from the optional client string in `hello`, such as `ncdb-rs/0.3`, and from the protocol versions it offers. Together with the network address, this can fingerprint a device.
- **What each member follows.** Which spaces each connection subscribes to, and from which cursor. A cursor shows how far behind a member is, and so roughly how long it was offline.
- **Who exchanged changes directly.** When a member uploads a change another member authored, the relay learns that the two met in island mode or forwarded through each other.
- **How many people have personal data in a space**, from the number of records in the person-key store, and when a record is added, re-wrapped or deleted.
- **Upload and download sizes** of large changes, and when they happen.
- **The rhythm of work**, per member and per space: bursts of small changes look like typing; a large push after days of silence looks like a member coming back from offline work.

**On AWS, the provider and the account's operators see the same**, through these services:

- API Gateway sees every connection, its network address and the size and time of every message.
- DynamoDB and S3 hold everything the relay stores: the log of frames, the membership table, the person-key store, the connection table.
- CloudWatch Logs hold what the relay logs: member ids, space ids, change ids, sizes, error codes and times, and in access logs network addresses. The relay never logs a frame, a payload, a key envelope, a person-key record or any other key material (GENESIS.md, "Never write content, keys or personal fields into logs"). Retention is decision R11.
- Backups (DynamoDB point-in-time recovery, S3 versioning if enabled) keep deleted person-key records until they expire. They are wrapped, and RFC 0003 states what follows from that.

**What the relay cannot see**: any content, the ordering metadata (clock, dependencies), whether a sealed change is content or a work order, the model, and which objects or fields a change touches (RFC 0002). A network address is personal data under the GDPR. The relay keeps it only for as long as a connection is open (decision R9) and in access logs of limited retention (decision R11).

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

    /// The membership table: one record per member id that was ever in the
    /// space, with its public keys, roles, and the positions of its addition
    /// and removal. Written only through `append` effects.
    async fn member(&self, space: SpaceId, member: MemberId)
        -> Result<Option<MemberRecord>, StoreError>;
    async fn members_of(&self, member: MemberId) -> Result<Vec<SpaceId>, StoreError>;

    /// The person-key store (RFC 0003): opaque wrapped records.
    async fn person_keys(&self, space: SpaceId) -> Result<Vec<PersonKeyRecord>, StoreError>;
    /// Stores records the core has matched to their person-key entries.
    /// There is no delete: records go only through a shred entry's effect.
    async fn put_person_keys(&self, space: SpaceId, records: Vec<PersonKeyRecord>)
        -> Result<(), StoreError>;

    /// The relay epoch, and setting a new one, which a restore must do
    /// before the relay serves any request.
    async fn epoch(&self) -> Result<Epoch, StoreError>;
    async fn new_epoch(&self) -> Result<Epoch, StoreError>;

    /// Large frames: an upload URL, and reading what was uploaded.
    async fn begin_upload(&self, space: SpaceId, size: u64) -> Result<Upload, StoreError>;
    async fn take_upload(&self, id: UploadId) -> Result<Vec<u8>, StoreError>;
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
- **An append and its effects are atomic.** A membership entry and the membership table, a shred entry and the deletion of its person-key record, change together or not at all.
- **Exact bytes.** A backend returns the frame byte for byte as it was appended.
- **Read your writes.** A read after a successful append sees the appended entries.
- **A new epoch after any loss.** A backend restored from a backup, or one that may have lost appends for any other reason, draws a new epoch before it serves a request.

One conformance test suite, run against every backend, holds these rules, so a new backend is interchangeable when it passes it. How each backend keeps them:

| Rule | SQLite | Postgres | DynamoDB with S3 |
| --- | --- | --- | --- |
| Conditional append | One write transaction; the head is a row in `spaces`. | `UPDATE spaces SET head = head + n WHERE id = $1 AND head = $2`, then the inserts, in one transaction. | `TransactWriteItems`: a condition on the head item, plus the entry and change-id items (see [The AWS backend](#the-aws-backend)). |
| One change id, one position | A unique index on (space, change id). | A unique index on (space, change id). | A change-id item per change, put with `attribute_not_exists`, in the same transaction. |
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
| `members` | space id | member id | signing key, agreement key, certificate, roles, added at (position), removed at (position) | Written only by an append's effects. A global secondary index on member id answers "which spaces is this member in". |
| `person_keys` | space id | person-key id | generation, wrapped record, writer, entry change id | Written only after the core matched it to its person-key entry; deleted only in the same transaction as a shred entry's append. |
| `connections` | connection id | space id | member id, connected at, expires at (TTL) | One item per subscription, plus one for the connection itself. A global secondary index on space id finds a space's subscribers. TTL removes what a missed `$disconnect` leaves behind. |

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
2. The relay re-applies the effects of every shred entry in the restored log, so no record survives that a shred in the restored log deleted.
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
- The relay refuses changes from an author who was removed while the change was on its way (see [What the relay checks](#what-the-relay-checks)). Members that received such a change directly keep it; members that did not never see it. Every member still folds the changes it holds to the same state as any other member holding the same set, but the sets differ, and the SDK reports a change the relay rejected (decision R7).
- Nothing enforces one global truth at write time, on an island or off it (RFC 0001).
- A relay that comes back after losing data, restored from a backup or empty, draws a new relay epoch, and its members repopulate it by their pushes (see [Restores and the relay epoch](#restores-and-the-relay-epoch)). Members match changes by id (RFC 0002). How a space moves between relays on purpose is a later RFC.

## What this RFC needs from RFC 0002 and RFC 0003

| Need | From | Status |
| --- | --- | --- |
| The relay can compute a change id, the author, the key generation, the kind and the suite from the frame alone. | RFC 0002 | Met by its header and change ids. |
| Control bodies (membership entries, key envelopes, key chain entries, shred entries) are readable by the relay and carry the public keys and certificate of an added member. | RFC 0002, RFC 0003 | Met: RFC 0003, "Sharing a space key with a new member", step 1 and 3. |
| A key envelope names the member id it is for, readable by the relay, so that `keys` can return a member's own envelopes. | RFC 0003 | To confirm. |
| The first change of a space is a control change that names the creator as its first member, with its public keys. | RFC 0003, or the membership RFC | To define. This RFC assumes it. |
| The member key may sign a relay challenge, with its own context string, or each member has a separate authentication key (decision R13). | RFC 0003 | To confirm. It is a second use of the signing key, separated by context. |
| A person-key entry, a control change that announces each person-key record by id, generation and hash. Shred entries carry the second account's signature, which the relay can verify (decision R12). | RFC 0003 | To define. |
| Key envelopes carry the recipient member id, and person-key records their person-key id and generation, outside the wrapping. | RFC 0003 | To confirm. |

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
| The relay checks the author's sequence numbers | Refusing an equivocating change makes the relay choose a winner between two signed changes; members detect equivocation themselves (RFC 0002). |
| Unsigned person-key records that any member may overwrite or delete | One member could erase a person's data without the second account's sign-off, or swap a key unnoticed (decision R12). |
| Restoring a backup under the same epoch | Positions would be reused for different changes, and members past them would skip changes silently. |
| The relay accepts changes of removed members written under an old generation | The relay cannot see when a change was written, so a removed member could keep writing under the old generation (decision R7). |

## Decisions for the owner

GENESIS.md reserves the wire format, anything that writes to AWS and anything that raises the bill for the owner. Each decision names the options, a recommendation and, where it differs, the cost.

**R1. The transport.**

- A. One WebSocket connection, CBOR messages. *(recommended)*
- B. HTTP requests for push and pull, server-sent events for subscriptions.
- C. gRPC, with a proxy for browsers.

Recommendation: A. One channel, one authentication, binary frames, every browser and API Gateway. Cost: as in [What it costs](#what-it-costs).

**R2. How a connection authenticates.**

- A. A challenge signed with the member key, per connection. *(recommended)*
- B. Bearer tokens from an identity service such as Cognito.

Recommendation: A. No new party learns who the members are, no token can be stolen and replayed, and the SDK already holds the key. B adds an identity service, roughly 0.015 USD per monthly active user above the free tier, and an operator who can mint access.

**R3. Compute on AWS.**

- A. API Gateway WebSockets and Lambda, live delivery from the appending Lambda. *(recommended for now)*
- B. As A, with live delivery through DynamoDB Streams and a second Lambda: retried delivery, about 100 to 300 ms more latency, slightly more cost.
- C. A WebSocket server on ECS Fargate behind a load balancer, the same core and tables.

Recommendation: A. About 10 USD a month for a small team and nothing while idle; about 800 USD a month for 1,000 agents writing once a minute. C costs about 45 USD a month at minimum and about 370 USD in the 1,000-agent case, so we switch to C when API Gateway messages pass about 300 USD a month. A budget alarm at 50 USD a month warns before that.

**R4. The protocol limits.**

- A. 128 KiB per message and 32 changes per push, on every backend. *(recommended)*
- B. Limits per backend: larger on SQLite and Postgres, API Gateway's on AWS.

Recommendation: A. A member never behaves differently depending on which backend its relay runs on, so backends stay interchangeable.

**R5. The largest change.**

- A. 16 MiB. *(recommended)*
- B. 4 MiB.
- C. No limit, with multipart uploads.

Recommendation: A. Large enough for rich-text pastes with images inlined, small enough that one change cannot block a member's pull. A larger file is not a change; files are a later RFC. Each 16 MiB change costs about 0.0004 USD to store a month and 0.0014 USD in transfer per member that downloads it.

**R6. Who may create a space on a relay.**

- A. Members whose member certificate comes from an account on the relay operator's allowlist. *(recommended)*
- B. Anyone who can reach the relay.
- C. Anyone with an invitation code from the operator.

Recommendation: A. A relay that accepts spaces from anyone is a free storage service for strangers and a bill for the owner. A is a list of account fingerprints in the relay's configuration.

**R7. Changes of a removed member that arrive late.**

- A. The relay refuses every change of a removed author from the removal on. *(recommended, as RFC 0003 says)*
- B. The relay accepts changes of a removed author under key generations before the removal.

Recommendation: A. B would let a removed member keep writing under the old generation forever, because the relay cannot see when a change was written. The cost of A, stated in [Island mode](#island-mode): a change written just before a removal may never reach the members that did not receive it directly.

**R8. The AWS region.**

- A. eu-central-1, Frankfurt. *(recommended)*
- B. us-east-1, N. Virginia.

Recommendation: A. The first users and the owner are in Europe, so latency is lower, and the metadata the relay keeps (network addresses, member ids) is personal data under the GDPR. It costs about 10 to 20 percent more than B.

**R9. What the relay keeps about connections.**

- A. Connection records only while the connection is open, removed by `$disconnect` or by TTL. *(recommended)*
- B. Connection records kept for 30 days after the connection ends, for abuse investigation.

Recommendation: A. The relay needs a connection only while it is open. How long logs keep network addresses is R11. Cost: none either way.

**R10. Backups of the relay.**

- A. DynamoDB point-in-time recovery (35 days); every restore draws a new relay epoch, re-applies shred effects, and members re-sync. *(recommended)*
- B. No backups. A lost relay starts empty under a new epoch, and members repopulate it.

Recommendation: A. In both options members re-sync after a loss, through the same procedure (see [Restores and the relay epoch](#restores-and-the-relay-epoch)). A restore needs fewer re-pushes, and it brings back the membership table and the person-key store before any member reconnects. With B, the person keys for a new member exist only on members until one of them re-uploads them. The price of A is that a shredded person-key record stays in the backups for up to 35 days, wrapped, as RFC 0003 states. Restores also return such a record until a member re-pushes the shred entry. Cost: 0.20 USD per GB-month, about 0.20 USD a month for a small team, about 11 USD a month per 56 GB in the 1,000-agent case.

**R11. How long CloudWatch keeps the relay's logs.**

- A. 14 days for the relay's logs and API Gateway's access logs. *(recommended)*
- B. 90 days.
- C. 1 day, and no network addresses in any log.

What the logs hold in every option: member ids, space ids, change ids, sizes, error codes, times, and, in access logs, network addresses. They never hold a frame, a payload, a key envelope, a person-key record or any key material.

Recommendation: A. 14 days is enough to investigate abuse or an outage, and short enough to state plainly what the relay keeps. Network addresses and member ids are personal data under the GDPR. B keeps them longer than the relay needs. C leaves abuse uninvestigable. Cost: storage of 0.03 USD per GB-month on top of ingestion, so under 1 USD a month for a small team. In the 1,000-agent case, about 0.30 USD a month for A and about 2 USD for B.

**R12. Protecting the person-key store.**

- A. Every record is announced by a signed person-key entry in the log; the relay and members accept a record only with a matching entry; records are deleted only by a shred entry carrying the second account's sign-off, which the relay verifies. *(recommended)*
- B. Records signed by their writer, without an entry in the log, and deletion by a signed request from any member.
- C. As first drafted: any member may put or delete records, and the relay trusts it.

Recommendation: A. C lets one member erase a person's data without the second account's sign-off, or swap a person key, and nobody can tell who did it. B names the writer, but the relay alone decides which record is current, and a single member can still delete. A ties every record and every deletion to the log, which every member holds and checks. Its cost is one more control change per person-key record written: a few hundred bytes each. It needs RFC 0003 to define the person-key entry and the account signature on shred entries; pm has asked NOC-7 to cover both.

**R13. Which key authenticates a connection.**

- A. The member's Ed25519 signing key, over a challenge with its own context string (`nocentraldb/relay-auth`). *(recommended)*
- B. A separate authentication key per member, certified by the account key and listed in the membership entry.

Recommendation: A. The context string and the relay id in the signed bytes keep an authentication signature from ever being valid as a change signature, or at another relay. The SDK manages no extra key. B keeps the signing key out of a second protocol entirely. It costs a third key per member, one more certificate field, and a membership entry whenever the key rotates. This is key handling, so RFC 0003 must confirm it; pm has asked NOC-7 to.

## Conflicts with GENESIS.md and RFC 0001

This RFC agrees with GENESIS.md and RFC 0001 except in the points below. Each needs the owner's ruling or a change to the other document.

1. **The relay sees more than RFC 0001 lists.** RFC 0001, GENESIS.md and use case 2 say the relay sees "which key is in which space, and when and how much each key writes". [What the relay sees](#what-the-relay-sees) adds who is online and from where, which spaces each member follows and how far behind it is, which members exchanged changes directly, and how many people have personal data in a space. RFC 0002 raised the same point for the log. We propose that RFC 0001, use case 2 and the FAQ link to both lists.
2. **Use case 4, "at no cost".** The reads are free; the writes are not. [What it costs](#what-it-costs) puts 1,000 agents writing once a minute at about 800 USD a month. The use case should say "reads at no cost", so a reader does not infer more than the design delivers (principle 4).
3. **Use case 5, members on the same network.** The use case says members on one network keep exchanging changes during an outage. This RFC defines the messages they exchange, but not how they find each other; until the peer RFC exists, the SDK cannot do it. The use case should say so, or wait for that RFC.
4. **RFC 0002, who uploads.** RFC 0002 says the relay need not know who uploads a change. This RFC requires the uploader to be an authenticated current member of the space, to keep strangers from filling a space's log; the author may still be any member. This narrows RFC 0002 without contradicting it.

## Summary of the choices

| Question | This RFC's answer |
| --- | --- |
| How does a member talk to the relay? | One WebSocket connection, CBOR messages, authenticated by a challenge signed with its member key. |
| What can it do? | Push changes, pull from a cursor, subscribe to a space, fetch its key envelopes, read and write the person-key store. |
| What does the relay check? | Canonical frames, signatures, membership of author and uploader, membership rules, duplicates. All with public data. |
| What does it see? | Membership, who writes when and how much, who is online from where, who follows what, how many persons a space holds keys for. No content. |
| How do backends stay interchangeable? | All checks in one core; backends implement one storage interface and pass one conformance suite. |
| How does it run on AWS? | API Gateway WebSockets, Lambda, DynamoDB on demand, S3 for frames above 8 KiB, in eu-central-1. |
| What does it cost? | About 10 USD a month for a small team; about 800 USD for 1,000 agents writing once a minute; nothing for local reads. |
| What if the relay is down? | Members keep working; on reconnect they pull, then push what the relay has not confirmed. |
| What if the relay is restored from a backup? | It draws a new epoch; members reset their cursors, pull and push what it lacks, and lost shreds are re-applied. |
| Who may change the person-key store? | Only a member whose signed entry in the log announces the record; only a signed-off shred entry deletes one. |
