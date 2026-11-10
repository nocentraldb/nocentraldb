# RFC 0003: Keys and cryptography

REQUEST FOR COMMENTS · NOVEMBER 2026 · nocentraldb.io

**Changelog.**

- **2026-11-08** · NOC-15, NOC-30 · [Amendment 1: the bytes of a sealed change](#amendment-1-the-bytes-of-a-sealed-change) fixes the content-key derivation, the nonce, the associated data, the padding and the crate versions. The owner decided all five (Carsten, 2026-11-08). The sentence it makes precise elsewhere is marked *(Amendment 1)*.
- **2026-11-08** · NOC-44 · [Amendment 2: following RFC 0008](#amendment-2-following-rfc-0008) brings this RFC in line with the owner's decisions in [RFC 0008](0008-membership.md): generation ids, exposure and the merge rotation, the narrower sign-off, the person-key entry, the model-id subkey, and the wait before a shredded key is destroyed. It decides nothing new. The sentences it changes elsewhere are marked *(Amendment 2)*.
- **2026-11-09** · NOC-75 · [Amendment 3: following RFC 0009](#amendment-3-following-rfc-0009) brings the sketch in "How the SDK hides keys" in line with the names the owner decided in [RFC 0009](0009-sdk-api.md), and says where the source of a copy in an ad-hoc space is kept. It decides nothing. The sentences it changes are marked *(Amendment 3)*.
- **2026-11-10** · NOC-106 · Where the member key lives, with a key file for a headless agent (decision 16), the account key each database draws until the accounts RFC (decision 17), and the 72 bytes of an encrypted previous key (decision 18), as the owner decided on NOC-95 and NOC-97, written into the text where each belongs (#111).

Which keys NoCentralDB uses, who holds them, and which audited primitives use them.

**Status.** Decided. GENESIS.md reserves crypto and key handling for the owner. The owner accepted the recommendation of every decision in [Decisions for the owner](#decisions-for-the-owner): 1 to 15 when this RFC was merged with the owner's approval in #27 (Carsten, 2026-11-08), and 16 to 18, on where the SDK keeps keys before the accounts RFC and on the bytes of an encrypted previous key (Carsten, 2026-11-10). The design below describes the decided options. Signing and sealing a change are built in `ncdb-core` (NOC-15); the rest is not built yet. [RFC 0001](0001-nocentraldb.md) describes the whole design; this RFC fills in the part about keys. The byte layout of a change and of the log belongs to [RFC 0002](0002-change-and-log-format.md), and this RFC uses its terms without defining the bytes: a change's **header** is the part the relay can read, its **payload** the part only members can read, and a **control change** (kind 1) is a signed change whose body the relay reads because it needs it to check membership, serve key envelopes or delete a person-key record. Every choice below names the options we rejected, and [Decisions for the owner](#decisions-for-the-owner) records the owner's decision on each.

**How to give feedback.** This RFC has no discussion of its own yet: start one in the category [RFC feedback](https://github.com/nocentraldb/nocentraldb/discussions/categories/rfc-feedback), named after the RFC, or open a pull request against this file to change its text. [Giving feedback](README.md#giving-feedback) says how, and what happens to your point.

We do not design a new protocol or a new primitive. Every construction here is published, widely deployed and implemented by a maintained Rust crate. Our work is to combine them correctly and to say plainly where the combination stops protecting you.

## Summary

- Every **member**, a device or an agent, has its own **member key**: a signing key and a key-agreement key. It is created on the member and never leaves it.
- Every person who uses NoCentralDB has an **account key**. It certifies which member keys belong to the account, and it signs decisions that need a person, such as the sign-off of an erasure.
- Every space has a **space key**, a random symmetric key with numbered generations. Each member receives the current generation wrapped to its own member key. When a member leaves, the space key rotates to a new generation that the removed member never receives.
- Every person whose data is marked personal has a **person key** in each space that holds such data. It encrypts that person's personal fields. Destroying it is the erasure.
- A change is encrypted with the space key and then signed with the author's member key. The relay checks the signature and the membership; it never holds a key that decrypts anything.
- The primitives are Ed25519 for signatures, HPKE (RFC 9180) with X25519 to wrap keys for a member, XChaCha20-Poly1305 to encrypt content, and HKDF-SHA256 to derive subkeys.
- The SDK creates, stores, wraps, rotates and destroys all of these keys. A developer never sees key bytes.

## What this protects, and against whom

The design protects content against:

- **The relay, its operators and its backups.** They store and forward ciphertext and key envelopes they cannot open.
- **Anyone who copies the relay's data**, through a breach or a stolen backup.
- **Members of other spaces.** A member holds keys only for its own spaces.
- **A member removed from a space**, for everything written after the space key rotated.

It does not protect against:

- **A member of the space.** Every member can read everything in its spaces. That is the point of a full copy.
- **The member's own device and operating system.** A process that runs as you can read what you can read (RFC 0001, "What it is not").
- **Metadata.** The relay sees which member key is in which space, who writes when and how much, and how many members a space has. From the key material it stores, it also sees:
  - for each key envelope, the member id it is for and the generation it carries, so it knows when the space key rotates and who received each generation;
  - for each person-key record, its person-key id, its generation and the member id that wrote it, so it knows how many people have personal data in the space, and when a record is added, re-wrapped or shredded (it never learns who the person is);
  - for each shred entry, which person-key id was destroyed, which accounts signed it off, and which members acknowledged it.

  These ids sit outside the encryption on purpose: the relay needs them to serve each member its own envelopes and to keep the person-key store.

## The keys

| Key | Kind | Who creates it | Where it lives | What it can do |
| --- | --- | --- | --- | --- |
| Member key | Ed25519 signing key and X25519 key-agreement key | The member (device or agent), on itself | The member's platform keystore, or a key file that a headless agent names; never leaves the member | Sign the member's changes; receive wrapped space keys |
| Account key | Ed25519 signing key | The person's first device, when the account is created | The account's devices, plus a recovery copy the person keeps | Certify and revoke the account's member keys; sign off irreversible work orders |
| Space key | 256-bit random symmetric key, one per generation | The member that creates the space, then the member that rotates it | Wrapped to each member key in the space's key envelopes; unwrapped only inside each member | Encrypt and decrypt the space's changes |
| Person key | 256-bit random symmetric key | The member that first writes a personal field for that person in that space | The space's person-key store, wrapped under the current space key; unwrapped only inside each member | Encrypt and decrypt that person's personal fields; destroying it erases them |

### The member key

A member is one device or one agent: a laptop, a phone, a browser profile, an agent process, a cloud function. Each member creates its own member key the first time it opens a database. The member key is two keys:

- an **Ed25519 signing key**, which signs every change and every membership entry the member writes;
- an **X25519 key-agreement key**, to which other members wrap space keys.

Both public halves form the member's public identity. Its **member id** is the SHA-256 hash of the two public keys, with a fixed context string in front (`"nocentraldb/member-id"`), and that id is what the relay, the member list and the SDK show. The private halves never leave the member. A lost or stolen device is handled by removing its member key from every space (see [Removing a member and rotating the space key](#removing-a-member-and-rotating-the-space-key)).

The SDK keeps the private halves in the platform keystore (see [How the SDK hides keys](#how-the-sdk-hides-keys)). A headless agent, in a container or on a server, often has no keystore. Such an agent names a key file instead, with the Rust SDK's `key_file` option ([RFC 0009, "The Rust SDK"](0009-sdk-api.md#the-rust-sdk)), and the SDK keeps the member key there, readable only by the user the agent runs as. Only its file permissions protect it: whoever reads the file, or a backup of it, holds the member key. Where no keystore is reachable and no key file is named, the SDK does not open the database ([decision 16](#decisions-for-the-owner)).

The signing key also **authenticates the member to the relay**. The relay sends a fresh random challenge, and the member signs `H("nocentraldb/relay-auth" || relay id || nonce)`, so that one relay cannot replay the answer against another. Every signature a member key makes signs either a canonical CBOR header (RFC 0002), whose first byte marks a CBOR map, or a message that starts with a `"nocentraldb/..."` context string, whose first byte is a letter; one can never be read as the other, so a signed challenge can never pass as a signed change. A separate key just for the relay is the alternative (decision 13).

We use two keys instead of converting the Ed25519 key into an X25519 key. Converting is possible, but it uses one secret in two protocols, and the analyses that show it safe are narrower than the ones for separate keys.

An agent is a member like any device. It has its own member key, it appears in the member list, and it is marked as an agent in the certificate its account issues (see the next section). A cloud agent or a cloud function that must read content joins the same way, as GENESIS.md requires: there is no hidden reader.

### The account key

A person has one account and usually several members: a laptop, a phone, the agents that run for them. The **account key** is an Ed25519 signing key that ties them together. It signs a short **member certificate** for each of the account's member keys: "member id M belongs to account A, as a device" or "as an agent". A sub-agent's certificate is signed by its parent agent instead, which never holds the account key (see [Member certificate](#member-certificate)). Other members check these certificates before they accept a new member in a space, so that "Alice's agent" is provably Alice's.

The account key also signs what must come from a person rather than a device, above all the second account's sign-off of an irreversible work order such as an erasure (GENESIS.md, "Work orders for clean-ups").

The account key is created on the person's first device. A second device receives it, wrapped to the new device's member key, after the person confirms the pairing on a device that already holds it, for example by scanning a code. The person keeps a recovery copy, such as a printed recovery code, for the case that every device is lost. Decision 8 chose where the account key lives.

Pairing and the recovery copy belong to the accounts RFC, which is not written yet. Until it is, each database draws its own account key once, on its first open, and keeps it next to the member key, in the same keystore or key file. It certifies its own member as a device, or as an agent when the app opens the database with the Rust SDK's `agent()` option ([RFC 0009, "The Rust SDK"](0009-sdk-api.md#the-rust-sdk)). A person's devices are then unrelated accounts: an admin who adds a second device of the same person compares its fingerprint, as for any account new to it. A genesis carries its creator's certificate for good, so the accounts RFC must accept the members certified this way, for example by letting a later pairing vouch for them ([decision 17](#decisions-for-the-owner)).

An organisation that runs agents does so under an account, so every agent traces back to an account that answers for it.

### The space key

A space is the unit of sharing, membership and keys (RFC 0001). Each space has a **space key**: 32 random bytes from the operating system's random number generator. It has **generations**. Generation 1 is created with the space, and every rotation creates a new one. A generation is named by an id no member can choose: a 16-bit ordinal, one more than the highest of its previous generations, and a 48-bit tag derived from the space, the rotation's author and its previous generations, so two concurrent rotations never name the same generation ([RFC 0008, "Generation ids"](0008-membership.md#generation-ids)). *(Amendment 2.)*

Members never use the space key directly. They derive one subkey per purpose with HKDF-SHA256, with a label for the purpose and the space id, generation and suite in the context. The first purpose is the content key that encrypts changes. Separate subkeys keep one purpose's use from weakening another's.

Each member receives the current generation in a **key envelope**: the generation's key, encrypted with HPKE to that member's X25519 key, and signed by the member who wrote it. Each generation also stores the keys of its previous generations, encrypted under the new generation (a **key chain**), so a member that holds the current generation can read the space's whole history without an envelope for every old generation. After concurrent rotations, a generation can have more than one previous generation. *(Amendment 2.)*

### The person key

The schema marks personal fields and ties each one to a person (RFC 0001, "Personal fields"). The person need not be a member; it is usually a customer, a patient or an employee the data is about. A person is identified across spaces by its **person id**, as RFC 0005 decision D14 defines it: 128 random bits, kept on the person's objects inside the encrypted payload. Members map a person id to that person's person-key id in each space. The relay never sees the person id. Its person-key store is keyed by the person-key id, which is random per space, so the relay cannot link one person across spaces. Where RFC 0005 says the person-key store works by person id, this RFC reads it as the members' mapping, not the relay's key.

For each person and each space that holds that person's data, the SDK creates a **person key**: 32 random bytes. It encrypts the person's personal fields inside the change, before the change as a whole is encrypted with the space key. Unlike everything else in a space, the person key does not live in the append-only log, because a log entry cannot be destroyed. It lives in the space's **person-key store**: one record per person, wrapped under the current space key generation, held by the relay as an opaque record that can be deleted, and by each member in its local keystore. Destroying the key is the erasure; see [Shredding a person's key](#shredding-a-persons-key).

The relay cannot check what a record contains, so the store must not let any one member swap a key or erase one on its own. Three rules hold it together (decision 14):

- **The log names every person key.** When a member creates a person key, it writes a **person-key entry**, a control change, with the person-key id (16 random bytes, never derived from who the person is), a commitment to the key: `H("nocentraldb/person-key-commit" || space id || person-key id || key)`, and the generation and SHA-256 hash of the record it stores, so that the relay accepts a record only with its entry. Every re-wrap of the record writes a further person-key entry that repeats the commitment. The key is 32 random bytes, so the commitment reveals nothing about it, before or after a shred. RFC 0008 defines the entry's body ([RFC 0008, "Person-key entry"](0008-membership.md#person-key-entry)). *(Amendment 2.)*
- **Every record is signed and checked against the log.** A record carries, outside the wrapping, the person-key id, the generation it is wrapped under and the member id of its writer, and the writer signs it with its member key (context `"nocentraldb/person-key-record"`). The relay accepts a record only from a current member, only for a person-key id that a person-key entry names and no shred entry has destroyed, and only for the same generation as the one it holds or a generation that descends from it in RFC 0008's generation graph. *(Amendment 2.)* A member accepts a record only if the signature verifies and the unwrapped key matches the commitment in the log. A record that fails is ignored, the SDK reports its writer, and any member that still holds the key writes a correct record again.
- **Only a shred entry deletes.** The relay deletes a record only for a valid shred entry: written by a member with the right to perform an erasure, and signed off by the account key of a second account, one that holds an admin member in the space and is not the performing member's account. It deletes the record once the shred is stable, not when it appends the entry ([RFC 0008, "How the relay applies the rules"](0008-membership.md#how-the-relay-applies-the-rules)). No other request deletes a record. *(Amendment 2.)*

So a member cannot swap a person key without every other member noticing, and cannot erase one without a second account. A member that overwrites a record with garbage can make the key unavailable to new members until another member repairs it, and the signature on the record names it. It cannot make the other members delete their copies; only a shred entry does that.

## Primitives

Each primitive is a published construction, used in widely deployed systems, with a maintained, pure-Rust crate that builds for `wasm32-unknown-unknown`. This RFC names the crates; it pins no version. The implementation ticket pins them, and the pin check in `pnpm gates` keeps them pinned.

| Purpose | Construction | Crate | Audit | `wasm32-unknown-unknown` |
| --- | --- | --- | --- | --- |
| Signatures | Ed25519 (RFC 8032), strict verification | `ed25519-dalek` 2.x on `curve25519-dalek` 4.x | Quarkslab, 2019: in-depth on `curve25519-dalek`, a lighter look at `ed25519-dalek` and `x25519-dalek`; only minor findings | Yes, pure Rust; the backend is chosen automatically |
| Key agreement and key wrapping | HPKE (RFC 9180), base mode, suite DHKEM(X25519, HKDF-SHA256), HKDF-SHA256, ChaCha20-Poly1305 | `hpke`, on `x25519-dalek` | No paid audit known to its authors; Cloudflare reviewed version 0.8 | Yes, pure Rust, `no_std` |
| Content encryption | XChaCha20-Poly1305: ChaCha20-Poly1305 (RFC 8439) with a 192-bit nonce through HChaCha20 | `chacha20poly1305` (RustCrypto) | NCC Group, 2020: no significant findings | Yes, pure Rust |
| Subkey derivation | HKDF (RFC 5869) with SHA-256 | `hkdf`, `sha2` (RustCrypto) | No dedicated audit we know of; HKDF-SHA256 is also inside the HPKE suite above | Yes, pure Rust |
| Hash H, for member ids and change ids | SHA-256 (FIPS 180-4), 32-byte output, always with a context string in front of the input | `sha2` (RustCrypto) | No dedicated audit we know of | Yes, pure Rust |
| Randomness | The operating system's generator; `crypto.getRandomValues` in the browser | `getrandom`, with its `wasm_js` backend in the browser | Not applicable: it calls the platform | Yes, through the `wasm_js` backend |
| Secret hygiene | Wipe key bytes from memory when dropped | `zeroize` | Not applicable | Yes |

Notes on each choice:

- **Ed25519** is deterministic, so a signature never depends on the quality of a random number at signing time. We verify with `verify_strict`, which rejects the malleable and small-order encodings that differ between implementations; every member must accept exactly the same signatures, or the copies diverge. Two advisories touched these crates since the audit: RUSTSEC-2022-0093 (a signing oracle when a caller passes a mismatched public key, fixed in `ed25519-dalek` 2.0 by an API that no longer allows it) and RUSTSEC-2024-0344 (timing variability in scalar subtraction, fixed in `curve25519-dalek` 4.1.3). The pinned versions must include both fixes.
- **HPKE** is the IETF standard for encrypting to a public key, and it is the building block MLS uses. We use base mode and authenticate the sender with an Ed25519 signature on the whole envelope, instead of HPKE's auth mode, so that one signature scheme authenticates everything a member writes. The `info` input binds the envelope to the space id, the generation id and the recipient's member id (see [Formats of what keys sign and derive](#formats-of-what-keys-sign-and-derive)). The `hpke` crate has no paid audit; we run the RFC 9180 test vectors in CI, and decision 3 asks whether to fund an audit.
- **XChaCha20-Poly1305** takes a 192-bit nonce, so every member can pick a random nonce for every change without coordinating, and collisions are negligible however many members write under one key. It runs fast in software, which matters in WebAssembly, where there is no hardware AES. XChaCha is specified in an IRTF draft, not an RFC; it is HChaCha20 in front of the RFC 8439 construction and has been deployed in libsodium for years.
- **SHA-256** is the one hash function H of suite 1, and it stays the hash for change ids and member ids in every later suite. A client that does not know a newer suite must still identify, deduplicate and attribute its changes, so RFC 0002's frame and change ids, and the member ids in the header, always use SHA-256; changing that hash would take a new frame version, not a new suite. Only the other primitives (signatures, key agreement, content encryption, subkey derivation) can change per suite. HKDF-SHA256 derives subkeys, and plain SHA-256 computes member ids here and change ids in RFC 0002. Each use puts its own context string in front of the input, so a hash computed for one purpose can never be mistaken for one computed for another. HKDF-SHA256 already sits inside the HPKE suite, so we add no second hash function.

Every ciphertext names exactly one key: the space, the generation, or the person key by id. A member never tries several keys on one ciphertext. ChaCha20-Poly1305 does not commit to its key, so a malicious member could build a ciphertext that decrypts under two different keys; because the key is named and covered by the signature, every member uses the same key and reads the same plaintext.

Each space records a **suite id** that names this set of primitives. RFC 0002 carries it in every change's header. A future suite, for example a post-quantum one, is then a new suite id for new spaces or new generations, not a format break.

**Rejected options:**

- **AES-256-GCM** for content: a 96-bit nonce allows only about 2³² random nonces per key before collisions become a real risk, which many writers per space can reach; and in WebAssembly it runs in slower, bitsliced software. It stays a candidate if the owner prefers a NIST construction (decision 2).
- **ChaCha20-Poly1305 with a 96-bit nonce**: same nonce problem as AES-GCM.
- **ECDSA on P-256** for signatures: the only curve that Apple's Secure Enclave, Android's hardware keystore and WebCrypto's oldest APIs all hold in hardware; but it needs a good random number for every signature, and its implementations have a long history of nonce bugs. It stays a candidate if hardware-bound member keys matter more (decision 4).
- **Converting Ed25519 keys to X25519** instead of two keys: one secret in two protocols, see [The member key](#the-member-key).
- **libsodium's sealed boxes** instead of HPKE: libsodium is audited, but it is C; building it for `wasm32-unknown-unknown` alongside the Rust core is awkward, and sealed boxes are not an IETF standard.
- **`hpke-rs`** (Cryspen) instead of `hpke`: also no clean audit record; a 2026 paper reported bugs in it and its `libcrux` backend, which Cryspen says it fixed. Comparable choice, larger dependency tree.
- **The platform's own crypto** (WebCrypto, CryptoKit, Android Keystore) for the core's operations: the behaviour would then fork by platform, which principle 7 forbids. The platform keystores still protect our keys at rest (see [How the SDK hides keys](#how-the-sdk-hides-keys)).
- **BLAKE3 or SHA-3** for derivation: no gain over HKDF-SHA256, which HPKE already needs.

## How a member signs and encrypts a change

RFC 0002 defines the bytes: its header carries the suite id, the space key generation, the author's member id and the author sequence, and each personal field carries its person-key id. This RFC defines the order and what each step covers:

1. **Personal fields first.** For each personal field, the SDK encrypts the value with the person's key (XChaCha20-Poly1305, random nonce). The associated data binds the space id, the object id, the field id and the person-key id, so a ciphertext cannot be moved to another field or another person.
2. **The change next.** The SDK encrypts the whole change payload, personal-field ciphertexts included, with the content key of the current space key generation. The associated data is every header field except the payload itself: the space id, the author's member id, the author sequence, the key generation, the kind, the header format and the suite id. A payload therefore cannot be moved to another space, author, sequence number, generation or kind, even by a member that re-signs it. Binding the whole header costs nothing, because every one of these fields is known before the payload is encrypted. The exact bytes of the content key, the nonce, the associated data and the padding are in [Amendment 1](#amendment-1-the-bytes-of-a-sealed-change). *(Amendment 1.)*
3. **Then the signature.** The author signs the header, which contains the ciphertext, with its Ed25519 key. This is encrypt-then-sign: the relay can verify the signature without decrypting anything, and the signature stays valid after a person key is destroyed, because it covers ciphertext, not plaintext. That is what keeps the audit trail intact after an erasure (RFC 0001, "Shred and audit").

A member receiving a change checks the signature, checks that the author was a member when the change was written, then decrypts. A change that fails any check is kept, not dropped silently (principle 3): it is rejected, as [RFC 0002 Amendment 1 (D11)](0002-change-and-log-format.md#rejected-changes) defines it, with a reason, and checked again when a control change arrives that could make it valid.

## Sharing a space key with a new member

A member with the right to add members (the roles belong to another RFC) adds a new member like this:

1. It obtains the new member's public identity: its member id, its two public keys and its member certificate. The SDK takes it from an invitation the new member accepted, never from a developer typing key bytes.
2. It checks the member certificate against the account key it expects. If the account is new to it, the SDK shows the account's fingerprint, so a person can compare it once, the way messaging apps compare safety numbers.
3. It writes a **membership entry**, a control change, that adds the member id to the space.
4. It writes a **key envelope** for the new member, also a control change: the current space key generation, wrapped with HPKE to the new member's X25519 key, signed by the adding member.
5. The new member downloads the space's log, opens its envelope, walks the key chain back to generation 1 and decrypts the whole history. It also opens the person-key store.

The relay sees the membership entry and that one envelope exists for the new member id. It cannot open the envelope. Adding a member costs one envelope.

By default a new member reads the whole history, because every member holds the full copy (RFC 0001). Whether a space can instead start a new member at the generation it joins is decision 5.

## Removing a member and rotating the space key

Removing a member works like this:

1. A member with the right to remove writes a **membership entry**, a control change, that removes the member id. The entry names a cutoff: the highest author sequence of the removed member that the remover held. The relay refuses the removed member's changes above the cutoff and stops serving it the space ([RFC 0008, "The cutoff"](0008-membership.md#the-cutoff)). *(Amendment 2.)*
2. The same member creates the next space key generation, writes a key envelope for every remaining member, and adds the **key chain entry** that encrypts the previous generation under the new one; the envelopes and the key chain entry are control changes too. Removing a member costs one envelope per remaining member.
3. It re-wraps every record in the person-key store under the new generation and deletes the old wraps, so a removed member that kept an old generation and later gets a copy of the relay's data still cannot open the person keys.
4. Every member that sees the new generation writes with it from then on.

This is **eager rotation**: the space key rotates as part of the removal. The alternative, rotating on the next write, is decision 6.

**What rotation does not do.** A removed member keeps the copy it already holds: every change, every space key generation and every person key it received before the removal. Rotation only keeps it from reading what is written afterwards. If you revoke access to a laptop or an agent that held a copy, assume that copy is still readable by whoever holds the laptop (principle 4). Two more limits follow from island mode:

- A member that has not seen the removal yet, because it is offline or on an island, keeps writing under the old generation. The removed member can read those changes, if it can get them, until that member catches up. Every change names its generation, so the members can see which changes were written under an outdated key.
- Two members can rotate at the same time on two islands. Both new generations are valid, and every remaining member receives envelopes for both when the islands rejoin. RFC 0008 decides which generation members then write with, and how the space leaves a generation that a removed member may hold. Every generation has an id that no member can choose, and every copy picks the same current generation. A generation is **exposed** when anyone who may hold it, through any envelope written for it, is no longer a member. When the current generation is exposed, the SDK of an admin device writes a **merge rotation** that lists every concurrent generation as previous and wraps the new key only to current members ([RFC 0008, "Two removals that rotate the key at once"](0008-membership.md#two-removals-that-rotate-the-key-at-once)). Until that merge rotation lands, members may write under a generation a removed member holds, and the SDK reports it (principle 4). *(Amendment 2.)*

A lost or stolen device is removed the same way, from every space its member key belongs to; the SDK offers this as one action on the account. In v1 that removal is the revocation: there is no separate signed statement that revokes a member certificate (see [Formats of what keys sign and derive](#formats-of-what-keys-sign-and-derive)).

## Sub-agents get an ad-hoc space

An agent that hands part of its task to a sub-agent does not share its own member key or its space keys (GENESIS.md, "Never give an agent more than its spaces"). Instead:

1. The parent agent creates an **ad-hoc space** with a fresh space key.
2. The sub-agent creates its own member key. The parent signs a member certificate for it with the parent's own signing key, marked as a sub-agent of the parent, so the chain of responsibility leads from the sub-agent through the parent to the account. The parent never holds the account key, so it cannot sign as the account.
3. The parent adds the sub-agent to the ad-hoc space and copies into it only the objects the task needs. The copies are new objects, encrypted under the ad-hoc space's key. The parent's SDK keeps, on the parent only, which object each copy came from, which is all the parent needs to write results back; the sub-agent never learns the source's id ([RFC 0009, "Sub-agents and ad-hoc spaces"](0009-sdk-api.md#sub-agents-and-ad-hoc-spaces)). *(Amendment 3.)*
4. The sub-agent works in the ad-hoc space. The parent, also a member, reads the results and writes what it accepts back into the original space under its own member key.
5. When the task ends, the parent deletes the ad-hoc space and its key, and the sub-agent's member key is discarded. For a local ad-hoc space this is a local deletion and needs no format. Deleting a space that reached the relay, for every member and at the relay, is not defined yet (see [Formats of what keys sign and derive](#formats-of-what-keys-sign-and-derive)).

When the parent and the sub-agent run on the same machine, the ad-hoc space can stay local and never reach the relay; when the sub-agent runs elsewhere, the space syncs through the relay like any other.

Copying a personal field into an ad-hoc space would put that person's data under a second space. Either the person key is shared into the ad-hoc space, so an erasure still reaches the copy, or personal fields are not copied at all by default. This is decision 11.

**What it does not do.** The ad-hoc space limits what the sub-agent can decrypt. It does not stop a sub-agent that runs as the same operating-system user from reading the parent's keystore on disk. For a real boundary, the sub-agent must run in a sandbox, container or separate user that cannot reach the parent's keystore. The SDK says so where it creates the space.

## Shredding a person's key

An erasure is a work order (RFC 0001, "Work orders"): one member performs it, and a second account, one that holds an admin member in the space, signs it off with its account key, because an erasure cannot be undone. *(Amendment 2.)* Then:

1. The performing member writes a **shred entry**, a control change, to the log: "person key P in this space is destroyed". It contains the key's id, never the key.
2. The relay checks the shred entry's signature and the second account's sign-off, and appends it. It deletes the person-key record from its store only once the shred is stable, because until then a concurrent revocation could still reject the shred, and a deleted record cannot come back (see [The person key](#the-person-key) and [RFC 0008, "How the relay applies the rules"](0008-membership.md#how-the-relay-applies-the-rules)). *(Amendment 2.)*
3. Every member that accepts the shred entry writes a "seen" acknowledgement at once, another control change. Once the shred is stable, it deletes its local copy of the key and writes a "deleted" acknowledgement. The "deleted" acknowledgements are the evidence: the audit shows which members confirmed the shred and which never came back (RFC 0001, "Where we want your advice", point 3). RFC 0008 defines both acknowledgements and when a shred is stable ([RFC 0008, "Acknowledgement"](0008-membership.md#acknowledgement)). *(Amendment 2.)*
4. From the shred entry on, the fold treats the person's personal fields as erased on every member, whether or not that member still holds the key. Computed fields and views therefore converge (principle 2): a member that has not deleted its copy yet does not compute a different state from the one that has.

The person's fields stay in the log, in the archive and in old snapshots as ciphertext under a key that no longer exists. The signatures on those changes stay valid, because they cover the ciphertext, so the audit trail survives the erasure.

**What shredding does not reach.** It cannot make anyone forget a key they already held. Concretely:

- **Devices that never come back online** keep the person key and the data. Their acknowledgement never arrives, and the audit shows that.
- **A modified or compromised client** can ignore the shred entry and keep the key. Every member of the space could read the person's fields while the key existed; the design cannot take that back.
- **Copies made outside the database**, such as exports and screenshots, stay readable.
- **Personal data in unmarked free text** was never encrypted with the person key, so shredding does not touch it.
- **An agent running as that user on the user's device** can read what the user could read while the key existed.
- **The key lives on until the shred is stable.** A shred is stable once every admin device of the space has written a control change after seeing it. Every view treats the person's fields as erased at once, but until then the key still exists on the members and the relay keeps the record. One admin device that stays offline delays it for as long as it is away; after 7 days the SDK of every admin raises an alarm that names the blocking devices, and an admin demotes or removes them ([RFC 0008, "Effects that wait until a change is stable"](0008-membership.md#effects-that-wait-until-a-change-is-stable), decisions M20 and M21). *(Amendment 2.)*
- **Backups of the relay** keep the wrapped person-key record until they expire. The relay and anyone who steals a backup cannot open it, because they hold no space key. A member of the space who also obtains such a backup could; that member could equally have kept the key while it was a member.

Whether destroying a key counts as erasure is for each data protection officer to decide. This is a design, not legal advice.

## Formats of what keys sign and derive

[RFC 0002](0002-change-and-log-format.md) defines the bytes of a change, and RFC 0008 (membership, NOC-23) defines the control bodies that carry these structures. This section defines what an account key or a member key signs inside them, and what a space key derives. Every format here is a proposal for the owner (decision 15).

Every hash input below starts with a context string of its own, and every field after it has a fixed length, or is canonical CBOR (RFC 0002), so no two inputs can be read as the same bytes. Ed25519 then signs the 32-byte hash. Ed25519 could sign the input directly; hashing it first keeps every signed message one length and matches RFC 0007's relay challenge.

### Member certificate

```text
member certificate (CBOR map, canonical)
├── 1 account key        32 bytes, the account's Ed25519 public key
├── 2 member id          32 bytes, the certified member
├── 3 kind               uint: 1 device, 2 agent, 3 sub-agent
├── 4 parent member id   32 bytes; sub-agents only
└── 5 signature          64 bytes
```

- **Device and agent** (kind 1 and 2): the account key signs `H("nocentraldb/member-cert" || canonical map of keys 1 to 4)`.
- **Sub-agent** (kind 3): the parent member's signing key signs `H("nocentraldb/sub-agent-cert" || canonical map of keys 1 to 4)`. Key 1 carries the parent's account key, so the sub-agent is attributed to that account. A member checks it against the parent's own certificate, which must be of kind 2 or 3 and name the same account key. RFC 0008 draft 0.2 has the account key sign every kind. That cannot work for a sub-agent: the parent agent does not hold the account key, and must not (see [Sub-agents get an ad-hoc space](#sub-agents-get-an-ad-hoc-space)). A separate context string keeps a parent's certificate signature from ever passing as an account's.

The member id covers both public keys, so the certificate binds them too.

### Account sign-off of a shred

```text
sign-off (CBOR map, canonical)
├── 1 account key        32 bytes
└── 2 signature          64 bytes, over
                         H("nocentraldb/shred-sign-off" || space id (16 bytes) ||
                           person-key id (16 bytes) || performer member id (32 bytes))
```

The sign-off is valid when its account holds an admin member in the space, and is not the account of the performer's certificate, as RFC 0008 requires. It names no work order, so the relay does not learn which sealed change was one. Replaying it gains nothing: it can only authorise the same performer to shred the same person key in the same space, which happens once.

### Key-chain subkey

The key chain entry encrypts each previous generation's key under a subkey of the new generation's key:

```text
key-chain subkey = HKDF-SHA256(ikm = new generation's key, salt = none,
                               info = "nocentraldb/key-chain" || space id (16 bytes) ||
                                      new generation id (8 bytes, big-endian))
```

Each previous key is encrypted with XChaCha20-Poly1305 under that subkey, with a random nonce. The associated data is the space id, the new generation id and that previous generation id, so an encrypted key cannot be moved to another place in the list or to another entry.

```text
encrypted previous key = nonce (24 bytes) || ciphertext (32 bytes) || tag (16 bytes)     72 bytes
  plaintext            = the previous generation's 32-byte space key
  AEAD                 = XChaCha20-Poly1305, random nonce
```

It is the layout of a sealed payload ([Amendment 1](#amendment-1-the-bytes-of-a-sealed-change), point 2) and of the [model id field](#model-id-subkey): one layout for everything a space key seals. Every key chain entry stores its previous keys this way, a merge rotation's included, which lists every head as previous ([RFC 0008, "Key chain entry"](0008-membership.md#key-chain-entry)) ([decision 18](#decisions-for-the-owner)).

### Model-id subkey

The genesis of a space carries the creator's `model.id` (RFC 0005, D13) sealed, so that members can read it and the relay cannot (RFC 0008, decision M26). It is sealed under a subkey of generation 1's key:

```text
model-id subkey = HKDF-SHA256(ikm = generation 1's space key, salt = none,
                              info = "nocentraldb/model-id" || space id (16 bytes))
model id field  = nonce (24 bytes) || ciphertext (16 bytes) || tag (16 bytes)     56 bytes
  plaintext       = model.id as its 16 raw bytes
  AEAD            = XChaCha20-Poly1305, random nonce
  associated data = "nocentraldb/model-id" || space id (16 bytes) || creator member id (32 bytes)
```

Every member can open it, because every member holds generation 1's key through the key chain; the relay holds no space key. The associated data binds the field to its space and its creator, so it cannot be copied into another genesis. RFC 0008 places the field in the genesis, as its key 13 ([RFC 0008, "Genesis"](0008-membership.md#genesis)).

**Why the label cannot collide.** The content key of generation 1 derives from the same key. Its `info` starts with `"nocentraldb/content-key"`, the key-chain subkey's with `"nocentraldb/key-chain"`, and the model-id subkey's with `"nocentraldb/model-id"`. No label is a prefix of another, and each is followed by fields of fixed length, so two different labels never give the same `info` bytes, and HKDF gives unrelated subkeys for different `info`. The label is new, used for nothing else, and the subkey seals only this one field per space. *(Amendment 2.)*

### Key envelope

The HPKE `info` input is `"nocentraldb/key-envelope" || space id (16 bytes) || generation id (8 bytes, big-endian) || recipient member id (32 bytes)`. The generation id is RFC 0008's 64-bit id: a 16-bit ordinal and a 48-bit tag that the members derive. The HPKE ciphertext is the 32-byte space key of that generation.

A note on the 48-bit tag: a member cannot choose a collision with a single try, but an admin that wants to can search about 2⁴⁸ member keys for one whose tag collides with a rotation it expects. RFC 0008 then rejects both entries. That attack only blocks a rotation, which a malicious admin can do more simply by not rotating; it exposes no key. Still, it is cheap enough to name.

### Not defined here

- **Revoking a member certificate.** There is no account-wide log in which a revocation would live and that every space's members read. In v1 a lost or compromised member is removed from every space it belongs to (RFC 0008), which is what keeps it from reading anything new. A signed revocation belongs to a later RFC about accounts, together with where the account key lives (decision 8).
- **Deleting a space** at the relay and on every member, such as an ad-hoc space that synced. It is a question of the relay's storage and of each member's copy, not of keys: the keys go with the space. It belongs to RFC 0008 or a later RFC. A local ad-hoc space is deleted locally and needs no format.

## How the SDK hides keys

A developer works with spaces, members and work orders. The SDK does everything with keys (principle 5):

```ts
import { nc } from "@nocentraldb/sdk";

const db = await nc.open({ model, lock });           // creates this member's key on first open
const project = await db.spaces.create();            // creates the space key

await project.members.add(invitation, { role: "writer" });       // checks the certificate, writes the envelope
const removal = await project.members.remove(memberId);          // rotates the space key
removal.notice; // "The removed member keeps its copy of everything it received before now."

await db.workOrders.erase({ person: customerId });   // shreds the person key after a second account signs off
```

The names are [RFC 0009](0009-sdk-api.md)'s, which the owner decided, except `db.workOrders.erase`, which stays a sketch until the work-order RFC. A space has no name in the SDK: the app keeps a name in its own model ([RFC 0009, "Spaces"](0009-sdk-api.md#spaces)). *(Amendment 3.)* What the SDK does underneath:

- **Creates** the member key on first open, the space key with the space, a person key on the first personal field for a person in a space, and the next generation on every removal.
- **Stores** private keys in the platform's keystore, or in the key file a headless agent names, and never hands their bytes to the app: the Keychain on macOS and iOS, the Android Keystore, the Windows credential store, the Secret Service on Linux, and in the browser a non-extractable WebCrypto key that wraps the core's keys in IndexedDB, optionally unlocked by a passkey. Only the file's permissions protect a key file. Details of passkeys and browser storage are out of scope here.
- **Offers no API that returns key bytes.** The developer handles member ids, invitations and fingerprints, which are public. The one way a private key leaves a device is the account recovery flow, which a person runs, not an app.
- **Says what a permission change means.** Removing a member, moving data between spaces, creating an ad-hoc space and running an erasure return a notice that states the limits above in plain words, so the developer can pass them on. A change of how permissions behave is never silent (GENESIS.md, "What it must never do").

[Use case 3](../use-cases.md#3-an-outside-agent-limited-to-one-space) shows `members.add(invitation)` and `members.remove(memberId)`. A public key is not secret, but passing an invitation or a member id keeps developers away from key material altogether; this is part of decision 12.

## Keys never reach logs, errors or telemetry

No key, no content and no personal field is ever written to a log, an error message, telemetry or a crash report (GENESIS.md, "Never write content, keys or personal fields into logs"). The rules for the implementation:

- **Secret types do not print.** Every type that holds key bytes or plaintext implements `Debug` as `[redacted]` and has no `Display`. Key bytes are wiped from memory when dropped (`zeroize`).
- **Errors carry ids and kinds, not bytes.** "Decryption failed for change C in space S, generation 4" is fine; the ciphertext, the plaintext and the key are not part of any error.
- **No payload at any log level**, including trace and debug builds.
- **The core sends no telemetry.** If an app adds its own, it sees only what the SDK's public types expose, and none of those holds a secret.
- **The relay logs only what it already sees:** ids, sizes and times. That is the metadata named above, and the relay must not keep more of it than it needs to operate.
- **A test proves it.** The implementation ticket adds a test that fills keys, content and personal fields with recognisable canary values, drives every error path and formats every public type, and fails if a canary appears in any output.

## Group key distribution: options compared

The ticket asked to compare existing protocols for distributing a group key and to recommend one, not to design a new one. The three candidates:

| | Per-member wrapped space key | Sender keys | MLS (RFC 9420) |
| --- | --- | --- | --- |
| Used by | Keybase teams, 1Password vaults, many end-to-end encrypted file stores | Signal and WhatsApp groups | Webex, Wire, XMTP; via OpenMLS in Rust |
| Cost to add a member | 1 envelope | 1 envelope per sender to the new member, n in total | O(log n) path update |
| Cost to remove a member | n-1 envelopes | Every remaining sender re-sends its key: O(n²) messages | O(log n) path update |
| Forward secrecy | No, within a generation | Yes, per message | Yes, per epoch |
| Recovery after a member's key leaks | At the next rotation | At the next re-keying | At the next commit |
| A new member reads history | Yes, through the key chain | No, by design | No, by design |
| Concurrent changes to membership | Both rotations stay valid; deterministic pick | Works, but messy | Needs one total order of commits per group; two islands fork the group |
| What the relay must do | Store envelopes and serve each member its own | Relay pairwise messages | Act as the delivery service and order every commit; it still holds no key |
| Rust crate | Built from the primitives above | Built from the primitives above | `openmls` (audited by SRLabs; one high-severity finding, fixed) |

**Recommendation: the per-member wrapped space key.** It is the only one of the three that gives a new member the full history, which NoCentralDB promises (every member keeps the complete log). It needs no ordering of membership changes, so island mode keeps working. And it costs the relay only storage of opaque envelopes. Its price is no forward secrecy within a generation and a removal cost that grows with the number of members, which is acceptable because spaces are meant to be small and removals rare.

**Rejected:**

- **MLS**: its forward secrecy deletes the keys a new member would need to read history, and its commits need one total order, which a relay could impose but island mode cannot. We would be fighting the protocol's design goals. A future space type that does not need history, such as a chat, could use MLS; that is not for v1.
- **Sender keys**: no history for new members either, and the cost of a removal grows with the square of the space's size.
- **A key server that holds space keys**, such as a cloud KMS shared by all members: the relay's operators or the cloud provider could read content. Principle 1 forbids it.
- **One key per object**: finer erasure, but the number of envelopes explodes, and person keys already give erasure where it is needed.

## Decisions for the owner

GENESIS.md reserves crypto, key handling and formats for the owner. Each decision below names the options and a recommendation. Nothing here becomes code until the owner has decided.

1. **Group key distribution.** (a) Per-member wrapped space key with generations and a key chain. (b) MLS. (c) Sender keys. **Recommendation: (a)**, see the comparison above.

   Decided (Carsten, 2026-11-08): the recommendation, (a).
2. **Content encryption.** (a) XChaCha20-Poly1305. (b) AES-256-GCM. **Recommendation: (a)**: random nonces are safe for many writers, and it is fast in WebAssembly.

   Decided (Carsten, 2026-11-08): the recommendation, (a).
3. **HPKE implementation and an audit.** (a) The `hpke` crate, with the RFC 9180 test vectors in CI, and a paid audit of our combination before 1.0. (b) The same without the audit. (c) `hpke-rs`. **Recommendation: (a)**. The audit costs money, which GENESIS.md also reserves for the owner.

   Decided (Carsten, 2026-11-08): the recommendation, (a).
4. **Signature algorithm.** (a) Ed25519 in software keystores. (b) ECDSA P-256, held in hardware where the platform offers it. **Recommendation: (a)**: deterministic signatures, and one behaviour on every platform; the platform keystore still protects the key at rest.

   Decided (Carsten, 2026-11-08): the recommendation, (a).
5. **What a new member reads.** (a) The whole history, through the key chain. (b) Only from the generation it joined, optional per space. **Recommendation: (a) as the default, (b) later as an option** for spaces that ask for it.

   Decided (Carsten, 2026-11-08): the recommendation, (a).
6. **When the space key rotates.** (a) Eagerly, as part of every removal. (b) Lazily, on the next write after a removal. **Recommendation: (a)**: a removed member must not read anything written after its removal, even if nobody writes for a while.

   Decided (Carsten, 2026-11-08): the recommendation, (a).
7. **Scope of the person key.** (a) One key per person id (RFC 0005, D14) per space. (b) One key per person across all spaces, distributed into each space. **Recommendation: (a)**: a space's members never need a key that also opens the person's data in other spaces; an erasure work order finds the person's keys by person id and shreds them in every space the performing account can reach.

   Decided (Carsten, 2026-11-08): the recommendation, (a).
8. **Where the account key lives.** (a) On every device of the account, plus a recovery code the person keeps. (b) Only on designated admin devices, plus a recovery code. (c) Derived from a passkey. **Recommendation: (a)** for v1, the simplest for people to use; (b) as an option for organisations.

   Decided (Carsten, 2026-11-08): the recommendation, (a).
9. **Post-quantum key agreement.** X25519 does not resist a future quantum computer, and the relay's operators could keep today's ciphertext until one exists. (a) Ship X25519 in v1 and reserve a suite id for a hybrid suite later. (b) Use a hybrid X25519 + ML-KEM key agreement from v1. **Recommendation: (a)**, because the Rust implementations of ML-KEM are young; we say plainly in the documentation that v1 is not post-quantum.

   Decided (Carsten, 2026-11-08): the recommendation, (a).
10. **Person keys at the relay.** (a) The relay holds each person key wrapped under the space key, in a store it can delete from, so a new member can open personal fields while no other member is online. (b) Person keys never reach the relay; members hand them to each other directly. **Recommendation: (a)**: it adds no reader the design does not already have (see the limits of shredding), and it keeps new members, including cloud agents, working when nobody else is online.

   Decided (Carsten, 2026-11-08): the recommendation, (a).
11. **Personal fields in an ad-hoc space.** (a) The person key is shared into the ad-hoc space, so an erasure reaches the copy. (b) Personal fields are not copied unless the parent asks explicitly, and then as in (a). **Recommendation: (b)**: a sub-agent gets the least it needs.

   Decided (Carsten, 2026-11-08): the recommendation, (b).
12. **Public API names** for the key-related calls (`members.add`, `members.remove`, `workOrders.erase`, invitations). **Recommendation:** invitations and member ids instead of public keys in `members.add`, and a returned notice on every call that changes who can read what.

   Decided (Carsten, 2026-11-08): the recommendation.
13. **How a member authenticates to the relay.** (a) The member's Ed25519 signing key signs the relay's challenge, separated from change signatures by a context string. (b) A third, separate key per member only for relay authentication, certified by the account key. **Recommendation: (a)**: the context string already keeps the two uses apart, the relay must know the signing key anyway to verify changes, and a third key adds a certificate and a rotation without protecting anything more.

   Decided (Carsten, 2026-11-08): the recommendation, (a).
14. **Integrity of the person-key store.** (a) A person-key entry with a commitment in the log, signed records the relay accepts only from current members, and deletion only through a shred entry with a second account's sign-off, as in [The person key](#the-person-key). (b) Unsigned records that any member may overwrite or delete, as an opaque store. (c) Person keys only inside control changes in the log, wrapped per member. **Recommendation: (a)**. (b) lets one member swap a key or erase it without a sign-off. (c) puts the key into the append-only log, where a shred can no longer destroy it.

   Decided (Carsten, 2026-11-08): the recommendation, (a).
15. **Formats of what keys sign and derive**, as [Formats of what keys sign and derive](#formats-of-what-keys-sign-and-derive) proposes them for RFC 0008: the member certificate, the account sign-off of a shred, the key-chain subkey and the key envelope's `info`. **Recommendation:** accept them, including the change from RFC 0008 draft 0.2 that a parent agent, not the account, signs a sub-agent's certificate.

   Decided (Carsten, 2026-11-08): the recommendation. RFC 0008's decision M26 B adds the [model-id subkey](#model-id-subkey) to these formats (Carsten, 2026-11-08). *(Amendment 2.)*
16. **Where the member key lives** (NOC-95, point 1). The keystore leaves out headless agents, which often have none. (a) The platform keystore, and a key file for an agent that names one; without either, the database does not open. (b) The platform's app data directory, found from an app id, with the same keystore. (c) In memory only, until a keystore ticket. **Recommendation: (a)**: it works the same for a desktop app, a server and an agent in a container, and only an app that asks for a key file gets one, with its limit stated. (b) suits apps but rarely servers and agents, and (c) keeps no native data at all.

   Decided (Carsten, 2026-11-10): the recommendation, (a). Asked on NOC-95.
17. **The account key before the accounts RFC** (NOC-95, point 2). (a) Each database draws its own account key once and keeps it next to the member key; an option marks the member as an agent. (b) The app passes the account key. (c) No space can be created until the accounts RFC exists. **Recommendation: (a)**: a person's devices are unrelated accounts until the accounts RFC links them, which that RFC must allow for. (b) puts a key in the app's hands, which principle 5 rules out, and (c) blocks every space.

   Decided (Carsten, 2026-11-10): the recommendation, (a). Asked on NOC-95. The accounts RFC follows (NOC-107).
18. **The bytes of an encrypted previous key** in a key chain entry (NOC-97). (a) `nonce (24) || ciphertext (32) || tag (16)`, 72 bytes, with a random nonce, as a sealed payload. (b) A CBOR map `{1: nonce, 2: ciphertext and tag}`, about 76 bytes. (c) A deterministic nonce, 48 bytes. **Recommendation: (a)**: one layout everywhere. (b) is a second layout to read for room it does not need, and (c) could repeat a nonce under one key when two rotations name the same generation (RFC 0008, M23).

   Decided (Carsten, 2026-11-10): the recommendation, (a). Asked on NOC-97.

## Agreement with GENESIS.md and RFC 0001

This RFC follows GENESIS.md and RFC 0001: the relay holds no keys and checks only signatures and membership; every reader, a cloud agent included, is a member with its own key; a sub-agent gets an ad-hoc space; work orders that cannot be undone need a second account; shredding leaves signatures intact; the SDK manages keys; and only audited, widely used primitives are combined.

It differs from them, or makes them more precise, in these points. pm flags each to the owner:

1. **"One key per person"** (GENESIS.md, "Shred and audit"). This RFC recommends one key per person per space (decision 7). An erasure still destroys every key of that person, but it is more than one key.

   Accepted (Carsten, 2026-11-08).
2. **The limits of erasure.** GENESIS.md and RFC 0001 name offline devices, exports, unmarked free text and an agent running as the user. This RFC adds two: a modified client that ignores the shred, and a member that obtains a relay backup taken before the shred. Both follow from the fact that a key once held cannot be taken back. RFC 0001 should list them.

   Accepted (Carsten, 2026-11-08).
3. **Use case 3** says a removed agent "receives no new changes". True for the relay. A member that has not yet seen the removal may still write under the old generation, which the removed agent could read if it got hold of those changes (see [Removing a member and rotating the space key](#removing-a-member-and-rotating-the-space-key)).

   Accepted (Carsten, 2026-11-08).
4. **Use case 3** passes `agentPublicKey` to `members.add`. This RFC recommends invitations or member ids, so a developer never handles key material (decision 12).

   Accepted (Carsten, 2026-11-08).
5. **Use case 1** says that without the key "every copy of those fields is unreadable, on every device". That holds for copies whose device has deleted the key; a device that never comes back keeps it, as the same use case says further down. The sentence should name that limit where it makes the claim.

   Accepted (Carsten, 2026-11-08).
6. **Post-quantum.** Neither GENESIS.md nor RFC 0001 mentions it. v1 would not resist an attacker who records ciphertext today and has a quantum computer later (decision 9). Principle 4 asks us to say so.

   Accepted (Carsten, 2026-11-08).

## Amendment 1: the bytes of a sealed change

Added on 2026-11-08 for NOC-15, after this RFC was merged. [How a member signs and encrypts a change](#how-a-member-signs-and-encrypts-a-change) fixes the primitives and their order, and [RFC 0002](0002-change-and-log-format.md) D8 fixes that payloads are padded to 256 bytes. Five details at byte level were still open. They are part of the wire format, so the owner decided them (NOC-30). A wrong choice could only be fixed with a new suite.

1. **The content key.** Content key = HKDF-SHA256(ikm = the space key of the header's generation, salt = none, info = `"nocentraldb/content-key"` ‖ space id (16 bytes) ‖ key generation (8 bytes, big-endian) ‖ suite id (8 bytes, big-endian)), 32 bytes. It has the same shape as the [key-chain subkey](#key-chain-subkey), and with fixed widths two different contexts never give the same bytes.
   - Rejected: the inputs as a canonical CBOR array in `info`, which needs an encoder for the derivation and a second shape of subkey.
   - Rejected: the space id as the HKDF salt, which splits the context over two inputs where every other subkey has none.
2. **The nonce.** Each seal draws a random 24-byte nonce. Header key 6 holds the nonce, then the XChaCha20-Poly1305 ciphertext with its 16-byte tag: nonce ‖ ciphertext ‖ tag, libsodium's layout. A random nonce needs no stored state, which is why XChaCha was chosen.
   - Rejected: the nonce in a header key of its own, which costs a key and a rule for the associated data and gains nothing.
   - Rejected: a nonce derived from author and sequence, which repeats when a device is restored from a backup or reuses a sequence number, and then reveals both plaintexts.
3. **The associated data.** The canonical CBOR encoding (RFC 0002) of the header map without key 6. Unknown header keys are included, so a header field added later is bound without anyone having to remember it.
   - Rejected: a fixed list of fields, which leaves a later field unbound.
   - Rejected: no associated data, relying on the signature alone, which loses the binding wherever the signature is checked apart from the decryption.
4. **The padding.** The canonical encoding of the whole payload map, padding key 5 included, is a multiple of 256 bytes. The sealer picks the length of the zero bytes in key 5 so that it lands on the multiple. A payload already on a multiple gets no key 5. Key 5 costs at least 2 bytes, and one length (26 bytes short) cannot be reached either, so a payload 1 or 26 bytes short of a multiple pads to the multiple after. Readers ignore key 5, so the rule can change without a new format (RFC 0002, D8).
   - Rejected: padding the payload without key 5 to the multiple, which lets the key's own 1 to 3 bytes show the true size again.
   - Rejected: padding outside the CBOR, at the encryption layer, which needs its own length field and goes against D8.
5. **The crates.** `chacha20poly1305 =0.11.0` and `hkdf =0.13.0`, with the same `sha2 =0.11.1` that computes change ids. The ChaCha20-Poly1305 cipher core the 2020 NCC Group review covered has not changed since, and security fixes land only in the current line. The pinned versions themselves have no audit of their own.
   - Rejected: the older line the review covered, `chacha20poly1305 0.10.1` and `hkdf 0.12`, which is no longer maintained and brings a second `sha2`.
   - Rejected: libsodium through bindings, which [Primitives](#primitives) already rejected for `wasm32-unknown-unknown`.

Decided: Carsten, 2026-11-08 (NOC-30), each as recommended. `ncdb-core` implements all five in `src/seal.rs`. `crates/ncdb-core/tests/vectors/generate.py` writes a test vector for a signed, sealed change from this description with independent libraries.

## Amendment 2: following RFC 0008

Added on 2026-11-08 for NOC-44. [RFC 0008](0008-membership.md) defines membership, roles and the control bodies that carry this RFC's formats, and the owner accepted every one of its decisions, M1 to M26 (Carsten, 2026-11-08). Its section [Conflicts with GENESIS.md and the other RFCs](0008-membership.md#conflicts-with-genesismd-and-the-other-rfcs) names where this RFC said something else. This amendment changes those sentences, marked *(Amendment 2)* above. The owner has decided every point, so it adds no decision.

- **Concurrent rotations** (conflict 8, decisions M6 and M15). Generations are named by derived ids, an ordinal and a tag, instead of consecutive numbers, so two islands that rotate at once create two distinct generations. A generation is exposed when anyone who may hold it is no longer a member, and the SDK of an admin device that sees an exposed current generation writes a merge rotation. Picking "the generation whose entry has the lower hash" is gone ([RFC 0008, "Generation ids"](0008-membership.md#generation-ids), and [Two removals that rotate the key at once](0008-membership.md#two-removals-that-rotate-the-key-at-once)).
- **The sign-off of an erasure** (conflict 9, decision M9). The second account must hold an admin member in the space, in the state of the shred's causal past, and must not be the performer's account. Any account with a certified member no longer suffices, because an erasure cannot be undone and a reader's account should not approve one ([RFC 0008, "Roles and rights"](0008-membership.md#roles-and-rights)).
- **The person-key entry** (conflict 11). It carries both this RFC's commitment and the generation and record hash the relay needs. This RFC points to RFC 0008 for its body, and a record replaces a stored one only for the same or a descendant generation, because generation ids carry a tag and are no longer ordered by size ([RFC 0008, "Person-key entry"](0008-membership.md#person-key-entry)).
- **The model-id subkey** (conflict 14, decision M26 B). [Formats of what keys sign and derive](#formats-of-what-keys-sign-and-derive) adds the subkey `"nocentraldb/model-id"` of generation 1's key, which seals the model id in the genesis. RFC 0002's list of what the relay cannot see stays true: the relay cannot read the model ([RFC 0008, "Genesis"](0008-membership.md#genesis)).
- **When a shredded key is destroyed** (decisions M20 and M21). Members delete their copy of a person key, and the relay its record, only once the shred is stable; every view treats the fields as erased at once. The list of what shredding does not reach names the wait and the alarm ([RFC 0008, "Effects that wait until a change is stable"](0008-membership.md#effects-that-wait-until-a-change-is-stable)).
- **What a removal refuses** (decision M3). The relay refuses a removed member's changes above the cutoff the removal names, not every change from the moment the removal arrives ([RFC 0008, "The cutoff"](0008-membership.md#the-cutoff)).

## Amendment 3: following RFC 0009

Added on 2026-11-09 for NOC-75. [RFC 0009](0009-sdk-api.md) defines the SDK's API, and the owner accepted every one of its decisions, S1 to S18 (Carsten, 2026-11-09). Its section [Conflicts with GENESIS.md and the other RFCs](0009-sdk-api.md#conflicts-with-genesismd-and-the-other-rfcs) names two points where this RFC says something else. This amendment changes those sentences, marked *(Amendment 3)* above, and decides nothing.

- **The sketch in "How the SDK hides keys"** (RFC 0009's [point 5](0009-sdk-api.md#conflicts-with-genesismd-and-the-other-rfcs), decisions S1, S11 and S13). The sketch named a space with `db.spaces.create({ name: "Project A" })`, where the name has no place: the genesis holds no name, because a name is content the relay must not read. It now creates the space without a name, adds the member with the role `"writer"`, one of RFC 0008's roles, instead of `"editor"`, and opens the database with its `model.lock.json`. The names are RFC 0009's, except `db.workOrders.erase`, which stays a sketch.
- **The source of a copy** (RFC 0009's [point 9](0009-sdk-api.md#conflicts-with-genesismd-and-the-other-rfcs)). Step 3 of "Sub-agents get an ad-hoc space" said the copies keep a reference to their source, and not where. The parent's SDK keeps it, on the parent only; the sub-agent never learns the source's id.

## Sources

- RFC 8032 (EdDSA), RFC 9180 (HPKE), RFC 8439 (ChaCha20-Poly1305), RFC 5869 (HKDF), FIPS 180-4 (SHA-256), RFC 9420 (MLS); the IRTF draft for XChaCha20.
- Quarkslab, security audit of the dalek libraries, 2019.
- NCC Group, RustCrypto AES-GCM and ChaCha20+Poly1305 implementation review, 2020.
- The `hpke` crate's README, on its audit status.
- SRLabs, audit of OpenMLS, sponsored by the Sovereign Tech Agency.
- RustSec advisories RUSTSEC-2022-0093 and RUSTSEC-2024-0344.
