# RFC 0019: Pairing a device through the relay: an account's pairing mailbox, and the fingerprint as words

REQUEST FOR COMMENTS · NOVEMBER 2026 · Ticket NOC-593

This RFC lets a new device ask to join an account without a scanned code and without text copied between devices. The new device leaves its pairing request in the account's **pairing mailbox** at the relay, sealed so that only the account's devices open it; a device of the account sees the request live, the person compares the full fingerprint on both screens, and the answer comes back the same way, sealed to the new device. It also shows that fingerprint as 32 words of the full digest beside RFC 0010's 60 digits, so that a person compares it more easily without comparing a shorter code ([The problem](#the-problem)).

It builds on the pairing of [RFC 0010](0010-accounts.md) and the relay protocol of [RFC 0007](0007-relay.md); [How this RFC relates to the others](#how-this-rfc-relates-to-the-others) names the rest.

**How to give feedback.** Start a discussion in the category [RFC feedback](https://github.com/nocentraldb/nocentraldb/discussions/categories/rfc-feedback), naming RFC 0019 and the section you mean, or open a pull request against this file to change its text. [Giving feedback](README.md#giving-feedback) says how, and what happens to your point.

**Status.** Draft. Each choice that GENESIS.md keeps for the owner is put to the owner in a ticket of its own; the owner decides the RFC in its pull request. The fingerprint's words can be built on their own, before the mailbox.

**At 0.1.** Not built. Release 0.1 pairs with two codes only, an invitation and an answer that the app carries between the devices, and shows fingerprints as 60 digits only.

**Out of scope.** A pairing invitation small enough for a dense QR code, which its own ticket measures (NOC-594). A statement signed by the account that tells an app's backend a device was admitted: NoCentralDB is not a sign-in factor for an app. A mailbox for invitations into spaces, which [RFC 0009, "Invitations and members"](0009-sdk-api.md#invitations-and-members) did not take and this RFC does not change. A directory that finds an account by name or address: the app tells the new device which account it asks to join. The account fingerprint that `members.add` asks for keeps its digits in TypeScript; this RFC adds the words where pairing shows a fingerprint. Pairing in island mode, which the two codes keep doing.

## Summary

| Question | Answer |
| --- | --- |
| What is the pairing mailbox? | A place at the relay, one per account, keyed by the account's identity, where a device that is not yet a member leaves a pairing request and receives its answer. |
| How does the new device know the account? | The app gives it the account's identity, `db.account.identity` of an account device, for example from its own backend after the person signed in. |
| How is the request sealed? | With HPKE to the account's mailbox key, an X25519 key every device of the account derives from the account key. The account key signs the mailbox key, so the new device checks it against the identity before it seals anything. |
| How is the answer sealed? | RFC 0010's pairing answer, sealed again with HPKE to the new device's agreement key. The account key inside stays sealed as RFC 0010 seals it. |
| Who reads the mailbox? | A device of the account that shows its certificate to the relay, while its app follows the mailbox. Agents do not. |
| Who may post? | Any connection the relay authenticates, one open request per member, at most eight open requests per mailbox. |
| How long does a request live? | 10 minutes by the relay's clock, then the relay deletes it. |
| What does the person compare? | The full member fingerprint of the new device, on both screens: 60 digits or 32 words. A request someone else posted looks the same until the two do not match. |
| What does the relay see? | That an account receives a pairing request, from which member and address, when, and whether it was answered, declined or lapsed. Not the request, not the answer, no key. |
| What are the words? | The 32 bytes of the fingerprint's SHA-256 digest, one word per byte from the PGP word list, shown in eight groups of four. |

## The problem

RFC 0010 pairs a device with two codes: the new device shows its invitation, a device of the account reads it and shows its answer, and the new device reads the answer. On two phones, that is two scans. Where one device has no camera, or the two are not side by side, the person copies two texts between them by hand, through a channel the app must provide. An app that uses NoCentralDB asked for a pairing in which no text is copied: the new device asks, a device the person already holds shows the request, and the person confirms there.

That needs a place both devices reach before the new device is a member of anything. The relay is the one place every device of the account already reaches. RFC 0010 did not take a mailbox at the relay, because it needs new requests in RFC 0007 and tells the relay when pairings happen. This RFC takes that cost, beside the two codes, which stay as they are, and says plainly what the relay learns ([What the relay sees](#what-the-relay-sees)).

Without a scan, the only check that the request on the account's device comes from the person's own new device is a fingerprint compared on both screens: anybody who knows the account's identity can post a request, and the relay itself could. RFC 0010 already pairs a device by a full fingerprint where no scan is possible, and rules out a short code, which an attacker who draws keys until a short digest matches defeats. Sixty digits are slow to compare. The second part of this RFC shows the same fingerprint as words, a different rendering of the full digest, not a shorter one.

## Terms

| Term | Meaning in this RFC |
| --- | --- |
| Account, account key, identity, recovery key, account key statement | As in RFC 0010. The identity is the recovery key, 32 bytes, which `db.account.identity` gives in hexadecimal. |
| Member key, member id, member certificate, invitation | As in RFC 0003. |
| Pairing, pairing answer, member fingerprint | As in RFC 0010. |
| Pairing mailbox | The relay's store of one account's open pairing requests and their answers, keyed by the account's identity. |
| Mailbox key | The X25519 key pair, derived from the account key, to which a new device seals its request. |
| Mailbox record | The mailbox key's public half, signed by the account key, with the account key statement that names that key. |
| Pairing request | A new device's invitation, sealed to the mailbox key, that the relay holds for 10 minutes. |
| Follow the mailbox | What a device of the account does while its app watches for pairing requests: the relay sends it each request as it arrives. |
| Fingerprint words | The 32 words that show a fingerprint's full digest. |

## R1. A pairing mailbox per account

### A pairing through the mailbox

```ts
// On the new device: the app knows the account's identity, for example from its backend.
const db = await nc.open({ model, lock, relay, pairing: "request" });
const request = await db.account.requestPairing({ identity });
show(request.fingerprint, request.fingerprintWords);   // the person compares these on a device of the account
const { notices } = await request.completed;            // the answer arrived and the pairing is complete

// On a device of the account, while the app follows the mailbox:
const stop = db.account.pairingRequests().subscribe((requests) => {
  for (const r of requests) show(r.kind, r.fingerprint, r.fingerprintWords);
});
// Once the person compared the fingerprint of the request they chose:
const { added, askAdmins } = await chosen.accept();
// or: await chosen.decline();
```

What happens underneath:

1. **The new device** opens for pairing and draws its member key, as RFC 0010's first step does. It connects to the relay and authenticates with its member key, as every member does: a connection proves which member key it holds, nothing more ([RFC 0007, "Opening a connection"](0007-relay.md#opening-a-connection)).
2. **It fetches the account's mailbox record** by the identity the app gave it, and checks it: the account key statement verifies under the identity, and the account key it names signed the mailbox key ([The mailbox key](#the-mailbox-key)). A record that does not hold is refused, and nothing is sealed.
3. **It seals its invitation** to the mailbox key and posts it ([The request and the answer](#the-request-and-the-answer)). The relay gives the request an id and a time at which it lapses, and the new device shows its member fingerprint, as it does under an invitation today.
4. **A device of the account that follows the mailbox** receives the request, opens it with the mailbox key, and checks that the invitation it holds belongs to the member that posted it. Its app shows the request with the kind and the member fingerprint the invitation gives ([RFC 0010, "Adding a device or an agent to an account"](0010-accounts.md#adding-a-device-or-an-agent-to-an-account), step 2).
5. **The person compares the full fingerprint** on both screens, as RFC 0010 requires for a device confirmed without a scan, and accepts or declines on the account's device. An agent is confirmed the same way: the mailbox is not a channel the person trusts, since anybody may post to it.
6. **On accept,** the device runs RFC 0010's pairing as `db.account.pair` does: it makes the answer, with the certificate, the account key statement and, for a device, the account key sealed to the new device, and adds the new device to the spaces the account administers. It seals the answer to the new device's agreement key and hands it to the relay, which closes the request for the account's other devices.
7. **The new device** receives the answer, opens it, checks that its recovery key is the identity it asked for, and completes the pairing as `db.account.completePairing` does, with the same checks.

The two codes stay. They work without a relay, in island mode, and between two devices side by side a scanned code proves more than a fingerprint the person may skim. The mailbox is a second way to carry the same invitation and the same answer, so RFC 0010's pairing, its answer format and its checks do not change.

### The mailbox key

The new device has no key in common with the account, and the account's devices have no key to receive on as an account: the account key is an Ed25519 signing key, and each device's agreement key is its own. So every device of the account derives one X25519 key pair from the account key, under a context of its own:

```text
mailbox seed   = HKDF-SHA256(ikm = the account key's 32-byte Ed25519 seed, salt = none,
                             info = "nocentraldb/pairing-mailbox-key")                    32 bytes
mailbox key    = DeriveKeyPair(mailbox seed) of the HPKE KEM DHKEM(X25519, HKDF-SHA256)
                 (RFC 9180, section 7.1.3)
```

Every device of the account derives the same key, so the account has one mailbox key and the relay one record. A new account key, after a lost device, gives a new mailbox key, and the lost device's copy of the old one opens no request sealed to the new one. The mailbox key is derived, not converted: RFC 0003 did not take using one secret as an Ed25519 key and an X25519 key ([RFC 0003, "The member key"](0003-keys-and-cryptography.md#the-member-key)), and an HKDF output under its own context string is an independent key, as RFC 0003 derives the recovery key from the recovery code.

The account key signs the mailbox key, so a new device that knows only the identity can check it:

```text
mailbox record (CBOR map, canonical)
├── 1 statement     map, the account key statement that names the account key and its number
├── 2 mailbox key   32 bytes, the mailbox key's X25519 public key
└── 3 signature     64 bytes: the account key that key 1 names signs
                    H("nocentraldb/pairing-mailbox" || canonical map of keys 1 and 2)
```

It follows RFC 0003's rule for every signed input, a context string of its own and a canonical map ([RFC 0003, "Formats of what keys sign and derive"](0003-keys-and-cryptography.md#formats-of-what-keys-sign-and-derive)). A device publishes the record when it starts to follow the mailbox; an Ed25519 signature is deterministic, so every device of the account publishes the same bytes for the same account key. The relay keeps the record with the highest statement number for each identity. A new device checks the statement against the identity, then the signature against the account key the statement names, and refuses the record otherwise.

### The request and the answer

**The request** is the new device's invitation, version 2 or, for a database that links its own account, version 1 ([RFC 0003, "Invitation"](0003-keys-and-cryptography.md#invitation)), sealed with HPKE in base mode, the suite RFC 0003 uses for every key envelope:

```text
plaintext  = invitation length (2 bytes, big-endian) || invitation bytes || zero bytes up to 269 bytes
info       = "nocentraldb/pairing-request" || recovery key (32 bytes) || statement number (8 bytes, big-endian)
aad        = empty
sealed     = HPKE encapsulated key (32 bytes) || ciphertext and tag (285 bytes)                   317 bytes
```

The padding makes every request the same size, 269 bytes being the length field and the longest invitation pairing takes, so the relay cannot tell from the size whether a device or an agent asks, or whether a database links its own account. The `info` binds the request to the account and to the mailbox key it was sealed to. A device of the account that opens a request reads the invitation, refuses one whose version pairing does not take, and checks that the member id it derives from the invitation's two public keys is the member id the relay says posted it. One that does not match is declined, and never shown: a request that names another member than the one waiting for its answer would send that answer to the wrong device.

**The answer** is RFC 0010's pairing answer, the bytes of `ncdb-pair-1:` ([RFC 0010, "The pairing answer"](0010-accounts.md#the-pairing-answer)), sealed again to the new device's agreement key, which the invitation carries:

```text
plaintext  = answer length (2 bytes, big-endian) || answer bytes || zero bytes up to 350 bytes
info       = "nocentraldb/pairing-answer" || recovery key (32 bytes) || request id (16 bytes)
aad        = empty
sealed     = HPKE encapsulated key (32 bytes) || ciphertext and tag (366 bytes)                   398 bytes
```

350 bytes hold the longest answer, a device's with a link, so every answer has the same size too. For a device, the account key inside stays sealed to the same agreement key under RFC 0003's `info` label `"nocentraldb/account-key"`; the outer seal hides the certificate, the account key statement and the kind from the relay. The new device opens the answer, refuses one whose recovery key is not the identity it asked for with `invalidCertificate`, and completes the pairing with RFC 0010's checks.

**A decline** carries no bytes. The relay tells the new device that its request was declined, and the account's other devices that it is closed.

### Who reads and who posts

**Who follows a mailbox.** A device follows its account's mailbox only while its app follows `pairingRequests()`, so a database that never asks for requests receives none. To follow it, the device shows the relay its member certificate with the account key statement it carries, and the mailbox record. The relay checks, from public data only: the certificate's member id is the connection's; its kind is a device; the statement verifies under the identity it names and the certificate under the account key the statement names; the statement's number is no lower than the highest the relay has seen for that identity, which it already keeps for admission ([RFC 0007, "What the relay checks"](0007-relay.md#what-the-relay-checks)); and the record's statement is the certificate's and its signature verifies. A lost device is therefore shut out once the relay has seen the next account key statement. An agent does not follow the mailbox: it holds no account key, so it could neither open a request nor answer one.

**Which accounts have a mailbox.** The relay keeps a mailbox record only for an account it already serves: one on its allowlist, one whose admission holds, or one with a member in a space the relay holds ([RFC 0015, "How the relay takes it"](0015-relay-admission.md#how-the-relay-takes-it)). A relay does not become a message store for accounts that have nothing on it.

**Who may post.** Any connection the relay has authenticated, since a new device is a member of nothing yet. The relay bounds what one poster and one mailbox can hold:

- **One open request per member.** A member that posts while it has a request open anywhere on the relay gets `rate_limited`.
- **Eight open requests per mailbox.** A ninth gets `rate_limited`, with the time until the oldest one lapses.
- **Fixed sizes.** A request is 317 sealed bytes and an answer 398; the relay refuses any other length with `malformed`.
- **The relay's own rate limits** on connections and messages apply, as for every request ([RFC 0007, "The errors"](0007-relay.md#the-errors)).

Member ids cost nothing to draw, so a poster that wants to can fill a mailbox's eight places for 10 minutes at a time, and the person sees requests they did not start. The limits bound the relay's storage and the number of prompts at once, not who prompts. Knowing the identity is no barrier either: every member of a space the account belongs to sees the account's certificates, and so its identity. What keeps a stranger out of the account is the fingerprint the person compares, and nothing else ([What this does not do](#what-this-does-not-do)).

### How long a request lives

A request lives **10 minutes** from when the relay accepted it, by the relay's clock. Until then the new device may withdraw it, and a device of the account may answer or decline it once. An answer is kept until the request's 10 minutes are over, so a new device whose connection dropped gets it when it asks again. The relay then deletes the request and its answer, and deletes them at once when the new device withdraws the request or a device of the account declines it. A new device whose request lapsed without an answer may post again.

Ten minutes leave the person time to take the device they already hold, open the app and compare, and keep the open requests at the relay few. The account's devices that followed the mailbox learn when a request closes, and why, so no device keeps showing a request another one answered.

### In the protocol

RFC 0007 gains these messages, numbered by its rule: key 1 the type, key 2 the request id where there is one, then the fields in the order listed ([RFC 0007, "The keys of each message"](0007-relay.md#the-keys-of-each-message)).

| Message | Direction | Keys after key 1 |
| --- | --- | --- |
| `mailbox` | member → relay | 2 req, 3 identity |
| `mailbox` | relay → member | 2 req, 3 identity, 4 mailbox record |
| `follow-mailbox` | member → relay | 2 req, 3 certificate, 4 mailbox record |
| `unfollow-mailbox` | member → relay | 2 req |
| `request-pairing` | member → relay | 2 req, 3 identity, 4 sealed |
| `pairing-posted` | relay → member | 2 req, 3 request id, 4 until |
| `pairing-request` | relay → member | 3 request id, 4 member id, 5 posted at, 6 until, 7 sealed |
| `answer-pairing` | member → relay | 2 req, 3 request id, 4 sealed (absent: declined) |
| `pairing-answer` | relay → member | 2 req (absent when sent live), 3 request id, 4 sealed (absent: declined) |
| `pairing-closed` | relay → member | 3 request id, 4 reason |
| `pairing-status` | member → relay | 2 req, 3 request id |
| `withdraw-pairing` | member → relay | 2 req, 3 request id |

```text
new device → relay   mailbox           {req, identity}
relay → new device   mailbox           {req, identity, mailbox record}
new device → relay   request-pairing   {req, identity, sealed}
relay → new device   pairing-posted    {req, request id, until}

device → relay       follow-mailbox    {req, certificate, mailbox record}
relay → device       ok                {req}
relay → device       pairing-request   {request id, member id, posted at, until, sealed}   each open request, then each new one
device → relay       answer-pairing    {req, request id, sealed}
relay → device       ok                {req}
relay → new device   pairing-answer    {request id, sealed}                                  to each open connection of the poster
relay → devices      pairing-closed    {request id, reason: "answered"}                      to every device that follows the mailbox
```

- **`follow-mailbox`** lasts as long as its connection, as a subscription does, or until `unfollow-mailbox`. The relay stores the record when it has none for the identity or the record's statement number is higher than the stored one's. One connection follows one mailbox.
- **`request-pairing`** is refused with `unknown_mailbox` when the relay holds no record for the identity, since no device of the account could open the request.
- **`answer-pairing`** comes from a connection that follows the request's mailbox, or is refused with `not_allowed`. The first answer or decline closes the request; a later one gets `unknown_request`.
- **`pairing-status`** lets a new device that reconnected ask what became of its request: the relay answers `pairing-posted` while it is open, `pairing-answer` once it was answered or declined, and `unknown_request` once it lapsed or was withdrawn, or when the connection's member did not post it.
- **`pairing-closed`** gives the reason as text: `"answered"`, `"declined"`, `"withdrawn"` or `"lapsed"`.
- **Delivery is best-effort,** as for live entries. A device that missed a request gets every open one again when it follows the mailbox again, and a new device asks with `pairing-status`.

The certificate in `follow-mailbox` is RFC 0003's canonical map, with the account key statement it carries. Sizes stay far below the 23 KiB of one message. RFC 0007's error table gains two codes:

| Code | When | What the member does |
| --- | --- | --- |
| `unknown_mailbox` | The relay holds no mailbox record for the identity: no device of the account follows a mailbox on this relay, or the relay does not serve the account. | The SDK reports it; the app pairs with the two codes. |
| `unknown_request` | The request id names no open request this connection may answer or ask about: it lapsed, was withdrawn or answered, or another member posted it. | The SDK reports a lapsed request; the new device may post again. |

`not_allowed`, `malformed` and `rate_limited` keep their meaning, for a certificate that does not pass the checks above, bytes of the wrong shape or length, and the limits above.

### In the backend

The checks run in the relay core, as every check does; a backend only stores ([RFC 0007, "The backend interface"](0007-relay.md#the-backend-interface)). The interface gains:

- **The mailbox record of an identity:** read it, and write it only when none is stored or its statement number is higher than the stored one's.
- **The open requests of an identity:** write a request only when the mailbox holds fewer than eight open requests and its poster none, in one conditional step, as the backend appends at the next position only if nobody else did; read the open ones; close one with its answer, its decline or its withdrawal, again only while it is open.
- **Which connections follow a mailbox,** beside the subscriptions a connection holds, and which connections a member holds, which the relay already finds for `added`.

**On AWS,** a table `pairing` holds the records and the requests, partitioned by identity, each request with its until as a time-to-live attribute; the `connections` table records which mailbox a connection follows. The relay deletes a request when it closes. DynamoDB deletes a lapsed item by its time to live within days, not at once, so the relay treats an item past its until as gone and deletes it when it next reads the mailbox. One pairing costs a few writes and a few Lambda invocations, well under a cent, and nothing while nobody pairs; it changes nothing in the ceiling on the project's own relay ([RFC 0015, "Abuse and cost"](0015-relay-admission.md#abuse-and-cost)). DynamoDB's point-in-time recovery keeps the table's earlier states for 35 days, so a backup holds sealed requests and sealed answers for that long ([What the relay sees](#what-the-relay-sees)).

### In the SDK

Both SDKs gain these calls. The rest of pairing keeps its names ([RFC 0009, "Accounts and devices"](0009-sdk-api.md#accounts-and-devices)).

- **`db.account.requestPairing({ identity })`**, on a database opened for pairing with a relay, fetches and checks the mailbox record, posts the request and returns a `PairingRequest` with the request's `fingerprint` and `fingerprintWords`, its `until`, `completed`, a promise of `{ notices }` that resolves once the answer arrived and the pairing completed, and `withdraw()`. `completed` rejects with `pairingDeclined` or `pairingExpired`; an identity that is not 64 hexadecimal digits fails with `invalidValue` before anything reaches the relay, and a relay that holds no mailbox for it with `unknownMailbox`. In Rust, `db.account().request_pairing(&identity)` returns the same, with `completed()` as an async method.
- **`db.account.pairingRequests()`**, on a device of the account, returns a report whose `current()` gives the open requests and whose `subscribe(listener)` follows them, as `db.sync()` does. The device follows the mailbox while the app subscribes, and stops when the last listener stops. Each request gives its `kind` (`"device"` or `"agent"`), its `fingerprint` and `fingerprintWords`, its `postedAt` and `until`, `accept()` and `decline()`. `accept()` returns what `db.account.pair` returns, `{ added, askAdmins }` with the answer already sent; `pair` itself stays for the two codes. In Rust, `db.account().pairing_requests()` with `PairingRequests::subscribe()`, and `IncomingPairing::accept()` and `decline()`.
- **No account key, no key bytes** reach the app on either side. The app handles an identity, which is public, and two fingerprints it shows.

The new device needs the identity before it can post. NoCentralDB has no directory ([RFC 0010, "What this does not do"](0010-accounts.md#what-this-does-not-do)), so the app provides it: an app whose backend stores `db.account.identity` when a person's first device signs up can hand it to a new device after the person signs in there. An app with no backend pairs with the two codes.

### What the person compares

The person compares the new device's member fingerprint, the one RFC 0010 shows under a waiting device's invitation (2026-11-13, NOC-288), as 60 digits or as [fingerprint words](#r2-the-fingerprint-as-words), the same form on both screens. Nothing else proves that the request came from the device in the person's hand: the relay forwards whatever was posted, and a request from anybody looks the same until its fingerprint differs. The SDK cannot see whether the person compared. An app that offers an accept without showing the full fingerprint, or that shows the request without saying it must match the new device's screen, hands whoever posted it the account key. For a database that links its own account, the invitation's fingerprint is its old account's, as with the two codes.

### Options not taken

- **A mailbox keyed by a hash of the identity.** It hides nothing: the relay sees every certificate, and with it every identity, and can compute the hash.
- **Requests in the clear.** An invitation holds only public data, and the relay sees the poster's public keys when it authenticates. Sealing still hides the kind, whether a database links another account, and the answer's certificate and statement, at the cost of one HPKE seal per message.
- **One mailbox key per device,** each device publishing a record of its own and the new device sealing to each. The relay would choose which records to hand out, every device would publish and renew its own, and a request would grow with the devices. One key derived from the account key needs one record per account.
- **An agreement key in the account key statement.** It changes the statement's format, and so every certificate, invitation and pairing answer that carries one ([RFC 0010, "The account's identity"](0010-accounts.md#the-accounts-identity)).
- **Converting the account key into an X25519 key.** One secret in two protocols, which RFC 0003 did not take for the member key.
- **A short code typed on the account's device, or a password-authenticated key exchange through the mailbox.** A short code is what RFC 0010 rules out and the request for this RFC asked not to have, and a PAKE crate whose audit we would first have to establish is what RFC 0010 did not take.
- **Posting only with a token an app's backend signs,** as an admission is signed ([RFC 0015, "The admission"](0015-relay-admission.md#the-admission)). It keeps strangers from posting to a relay that has admission keys, but not members who share a space with the account, and the fingerprint decides what a token would. It would also make the mailbox depend on an app's backend.
- **Every device following the mailbox whenever it is connected.** The relay would learn which devices of each account are online with no pairing under way, and every app would receive requests it never asked for.
- **Lifetimes of 2 minutes or 1 hour.** Two minutes is short for a person who has to find their other device; an hour keeps requests and answers at the relay for longer than a person waits.
- **A notification to the account's devices through a platform push service.** It adds a service that learns when pairings happen; a device that does not follow the mailbox sees no request, and the app may ask the person to open it.

## R2. The fingerprint as words

### The form

A fingerprint is a SHA-256 digest, 32 bytes ([RFC 0010, "The fingerprint"](0010-accounts.md#the-fingerprint)). Its words show all 32 bytes, one word per byte, from the **PGP word list**: two lists of 256 English words, one of two syllables for the bytes at even offsets and one of three syllables for the bytes at odd offsets. The list was published in 1995 for reading digests aloud over a voice channel, its words were chosen to sound different from each other, and PGP and ZRTP ([RFC 6189](https://www.rfc-editor.org/rfc/rfc6189)) use it to compare fingerprints and short authentication strings. Two lists that alternate catch the mistakes people make reading a long sequence: two words swapped, a word repeated, a word left out.

```text
fingerprint words: byte 0 from the two-syllable list, byte 1 from the three-syllable list, and so on,
                   32 words in the order of the digest's bytes, shown in eight groups of four
for example:       the bytes E5 82 94 F2 E9 A2 27 48 ... read
                   topmost Istanbul Pluto vagabond   treadmill Pacific brackish dictator   ...
```

The words are a second way to show the same digest, not a shorter one: they carry all 256 bits, where the 60 digits carry about 199. Every fingerprint, the account's and a member's, has both forms, the same on every platform and in both SDKs; the person compares one of them, the same form on both screens. The words keep RFC 0010's rule that a person compares the whole fingerprint. The list is committed beside the RFC's vectors with its SHA-256, so that both SDKs read the same 512 words.

**Not the recovery code's list.** RFC 0010 shows the recovery code as twelve words of the BIP-39 English list, and chose digits for the fingerprint so that people would not confuse the two. The PGP word list is another list. 59 of its 512 words are also BIP-39 words, 42 of the two-syllable ones and 17 of the three-syllable ones, so a fingerprint holds about four of them on average. A fingerprint is always 32 words in eight groups, written with the list's own capitals, and never asked for back; the recovery code is twelve words, shown once and asked back two at a time. An app shows the two in different places, and the texts of the SDK call one the recovery code and the other the fingerprint.

### Where it shows

- **Rust:** `Fingerprint::words()` returns `[&'static str; 32]`. `Display` keeps the digits.
- **TypeScript:** where the SDK returns a fingerprint for pairing, it returns its words beside it: the invitation that `db.invitation()` returns gains `fingerprintWords`, `db.account.pairingFingerprintWords(invitation)` gives the words `db.account.pairingFingerprint(invitation)` gives as digits (NOC-583), and the mailbox's request objects carry both.
- The fingerprint an app passes to `members.add` stays the digits, which is what it matches.

### Options not taken

- **The BIP-39 list.** RFC 0010 rules it out, because people would confuse the fingerprint with the recovery code.
- **Fewer words.** A shorter code is what an attacker defeats by drawing keys until it matches, as RFC 0010 says of a short digest; the request for this RFC asked for the full digest.
- **25 words, the digits' strength.** It would compare 200 bits, the 60 digits' strength, with seven words fewer, but it is not the full digest, and two forms of one fingerprint that cover different bits are harder to explain.
- **Emojis,** 43 symbols from the 64 of the Matrix protocol's list for short authentication strings, each holding 6 bits. They read the same in every language, but platforms draw the same emoji differently, so a person compares two pictures that differ when the fingerprints match, and 43 of them is a long row to compare.
- **Our own list,** the PGP word list with the 59 shared words replaced. Nobody else uses it, and nobody has measured how its new words sound against the others.
- **A single list, such as Bytewords,** one four-letter word per byte. It does not catch two words swapped.
- **Hexadecimal,** 64 characters. Harder to read aloud than words or digits, as RFC 0010 says of 16 bytes in hexadecimal.

## What the relay sees

The relay reads no request and no answer and holds no key that opens either. Stated plainly (principle 4), it learns:

- **That an account pairs through the mailbox:** its identity, its mailbox record, which is public, and which of its devices follow the mailbox, when and from which network address. The relay already knows the account's devices from their certificates; following shows when the person's app watches for a pairing.
- **For each request:** the identity it is for, the member id that posted it and that member's public keys, its network address, when it was posted, and whether it was answered, declined, withdrawn or lapsed, and when. Every request and every answer has the same size, so the relay does not learn the kind or whether a database links its own account.
- **Which device of the account answered or declined.**
- **Pairings that did not happen.** A declined or lapsed request shows an attempt, which pairing with two codes never shows the relay.
- **When the new device joins,** earlier than before: with the two codes, the relay learns of a pairing when the account space changes ([RFC 0010, "What the relay learns"](0010-accounts.md#what-the-relay-learns)); with the mailbox, at the request, before the new device is a member of anything, together with its network address.

The server the relay runs on sees all of that, as RFC 0007 says for everything else the relay learns ([RFC 0007, "What the relay sees"](0007-relay.md#what-the-relay-sees)). The relay's logs hold identities, request ids, member ids, sizes and times, never a sealed request, a sealed answer or a record's bytes. Backups hold sealed requests and answers for as long as they are kept: 35 days of point-in-time recovery on AWS.

**The account key passes through the relay, sealed.** With the two codes it never does. Through the mailbox, a device's answer carries the account key sealed to the new device's X25519 key, inside a second seal to the same key, so the relay and its backups hold it as ciphertext. v1 is not post-quantum ([RFC 0003, "Not post-quantum in v1"](0003-keys-and-cryptography.md#not-post-quantum-in-v1)): someone who records that ciphertext today could open it once a quantum computer can break X25519, and would then hold the account key as it was at that pairing. An account whose person wants to avoid that pairs with the two codes.

## What this does not do

- **It does not tell a stranger's request from the person's own.** Anybody who knows the account's identity can post one, including every member of a space the account shares, and the relay. The full fingerprint, compared on both screens, is the only check. A person who accepts without comparing gives the account key to whoever posted.
- **It does not stop a flood of prompts.** Eight open requests per mailbox and one per member bound what the relay holds, but member ids are free, so a poster can keep a mailbox full.
- **It does not work without the relay.** In island mode, or on a relay that does not serve the account, the two codes remain.
- **It does not stop the relay from withholding.** The relay can drop a request, an answer or a record, or tell a new device that its request was declined. It cannot forge an answer, which the account key signs, nor read one.
- **It does not shut out a lost device the relay has not yet heard about.** Until the relay sees the account's next account key statement, a lost device can follow the mailbox and open requests sealed to the mailbox key it derives. It already holds the account key, with which it could pair any device of its own.
- **A relay that serves an older mailbox record** can let a lost device that kept the older account key answer a new device. The new device then holds a retired account key and a certificate that members who learnt the newer statement refuse.
- **Words do not make comparing quicker than reading.** Thirty-two words are fewer things to compare than sixty digits, but a person still has to look at all of them.
- **Not post-quantum,** and the account key crosses the relay sealed, as [What the relay sees](#what-the-relay-sees) says.

## Agreement with GENESIS.md

- **Principle 1, the relay never reads content.** Requests and answers are sealed with HPKE to keys the relay does not hold, and the account key inside an answer stays sealed to the new device as RFC 0010 seals it.
- **Principle 4, say plainly what it does not do.** [What the relay sees](#what-the-relay-sees) lists the new metadata, and [What this does not do](#what-this-does-not-do) the limits, the account key crossing the relay included.
- **Principle 5, developers do not handle keys.** The app handles an identity, which is public, and fingerprints it shows.
- **Principle 7, one core.** The mailbox and the words are the core's, natively and in the browser.
- **Never invent cryptography.** HKDF-SHA256, HPKE's DeriveKeyPair and base mode, Ed25519 signatures with a context string and a canonical map, and SHA-256, as RFC 0003 uses them, with RFC 0003's crates. The mailbox key and its two seals are new uses of these, and belong in the paid audit of our combination that RFC 0003 plans before 1.0. The words are a published list, used as a rendering only.
- **Never add a hidden reader.** The mailbox reads nothing; the new device joins as a visible member, as with the two codes.
- **Conflicts.** None with GENESIS.md.

## Changes to the decided RFCs

Once this RFC is decided, each change is written into that text in its own pull request, as [How a decided RFC changes](README.md#how-a-decided-rfc-changes) says:

- **RFC 0003:** the mailbox seed and key, the mailbox record, and the `info` labels `"nocentraldb/pairing-request"` and `"nocentraldb/pairing-answer"` join "Formats of what keys sign and derive"; the fingerprint's words join "Account fingerprint".
- **RFC 0007:** the messages and the two error codes of [In the protocol](#in-the-protocol), the checks of [Who reads and who posts](#who-reads-and-who-posts) in "What the relay checks", the mailbox's metadata in "What the relay sees", the interface of [In the backend](#in-the-backend), and the table `pairing` on AWS.
- **RFC 0009:** `requestPairing`, `pairingRequests`, `pairingFingerprintWords`, `fingerprintWords`, `Fingerprint::words()` and the errors `pairingDeclined`, `pairingExpired` and `unknownMailbox` join "Accounts and devices", "Errors" and "Public names".
- **RFC 0010:** pairing gains the mailbox as a second way to carry the invitation and the answer, beside the two codes; the sentence that a mailbox at the relay was not taken becomes what this RFC decides; "What the relay learns" gains the mailbox; "The fingerprint" gains the words, and its sentence against words names the recovery code's list.

## Follow-up tickets

Each becomes its own build ticket once the RFC is decided, test first:

- **The fingerprint's words** in the core and both SDKs, with the word list and test vectors under `docs/rfc/vectors/`, which both SDKs check. It depends on nothing else here.
- **The mailbox key and the seals** in the core: the derivation, the record, the request and the answer, with vectors.
- **The relay:** the messages, the checks, the limits and the lifetime in the relay core, the SQLite backend and the AWS backend with its table, and the backend test suite.
- **The SDKs:** `requestPairing` and `pairingRequests` in both SDKs, and an end-to-end test that pairs a device through a relay.
- **The guides:** how an app hands the identity to a new device, and what its screens must show for the person to compare.

## How this RFC is tested

The RFC tests in `scripts/` check this file's form: its status and what 0.1 builds, its links and the headings they name, its terms against the glossary, and its record. The design is tested when it is built, by the tickets above.

## How this RFC relates to the others

It carries [RFC 0010](0010-accounts.md)'s invitation and pairing answer through the relay of [RFC 0007](0007-relay.md), adding messages, checks and a table to it; it derives and seals with [RFC 0003](0003-keys-and-cryptography.md)'s primitives and adds formats to it; it serves only the accounts a relay admits under [RFC 0015](0015-relay-admission.md); it adds calls to [RFC 0009](0009-sdk-api.md); and it shows the fingerprints of RFC 0010 in a second form.

## History

- 2026-11-28 · First published as a draft (NOC-593)
