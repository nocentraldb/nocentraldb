# RFC 0010: Accounts

REQUEST FOR COMMENTS · NOVEMBER 2026 · Ticket NOC-107

This RFC defines accounts: the account key and where it lives, the recovery key that is the account's identity, the recovery code, how a device or an agent joins an account, how the accounts that each database draws today are linked into one, what happens when a device is lost, and what a member certificate signed by an account means for membership. It turns the sketch in [RFC 0003, "The account key"](0003-keys-and-cryptography.md#the-account-key) into a design, so that a person's devices can be linked before the first release, as the owner asked when deciding that each database draws its own account key for now (NOC-95, Carsten, 2026-11-10).

It builds on [RFC 0001](0001-nocentraldb.md), above all [the design in one page](0001-nocentraldb.md#the-design-in-one-page); on [RFC 0003](0003-keys-and-cryptography.md), which defines the account key, the [member certificate](0003-keys-and-cryptography.md#member-certificate) and the [account sign-off of a shred](0003-keys-and-cryptography.md#account-sign-off-of-a-shred); on [RFC 0008](0008-membership.md), which places certificates in the genesis and in membership entries; on [RFC 0007](0007-relay.md), whose relay sees certificates; on [RFC 0006](0006-local-view.md), which keeps one database per member; and on [RFC 0009](0009-sdk-api.md), which names the entry points this RFC designs ([Accounts and devices](0009-sdk-api.md#accounts-and-devices), decision S15).

**How to give feedback.** Reply in this RFC's discussion, [RFC 0010: Accounts](https://github.com/nocentraldb/nocentraldb/discussions/154), one topic per reply, or open a pull request against this file to change its text. [Giving feedback](README.md#giving-feedback) says how, and what happens to your point.

**Status.** Decided. The owner decided this RFC as it stands (Carsten, 2026-11-10).

**Out of scope.** How a work order is claimed and signed off, beyond naming the account key as the key that signs (a later RFC on work orders). Passkeys and how the browser unlocks its keys (RFC 0003 leaves them out). Accounts shared by several people, such as an organisation with many administrators, beyond what one account with several devices gives. Deleting a space. A directory that finds an account by name or address: there is none, and people compare fingerprints.

## Summary

| Question | Answer |
| --- | --- |
| What is an account? | A key pair that answers for members: it signs the certificates of its devices and agents, and the sign-off of an erasure. It is not a login, and no server holds it. |
| Where does the account key live? | On every device of the account, in the platform keystore next to the member key, as RFC 0003 decided. Never on an agent, never at the relay. |
| What is the account's identity? | A recovery key that only the recovery code holds. It signs a statement naming the current account key, so a lost device is answered with a new account key. |
| What is the recovery code? | Twelve words that hold 128 random bits; the recovery key derives from them. |
| What does it restore? | The account, not the spaces: a new device under the account, which other members then add to their spaces. |
| How does a device join? | Two codes: the new device shows its invitation, a device of the account reads it, the person confirms, and the answer carries the certificate and the sealed account key back. |
| How are per-database accounts linked? | A database with its own account pairs like a new device; both account keys sign a link; the old key is deleted; memberships already written keep their certificate. |
| What does a certificate mean for RFC 0008? | Who answers for a member. It is checked once, when the member enters a space, and gives no right of its own. |

## Terms

| Term | Meaning in this RFC |
| --- | --- |
| Member, member key, member id | As in RFC 0003 and RFC 0008: a device, an agent or a sub-agent, its two key pairs, and the id that covers both public keys. |
| Account | Whatever answers for a set of members: a recovery key, and the account key it names. |
| Account key | The Ed25519 signing key that signs member certificates and erasure sign-offs (RFC 0003). |
| Recovery key | The account's identity: an Ed25519 key derived from the recovery code, held by no device, that names the current account key. |
| Account key statement | The recovery key's signature on "the account key is K, number n". |
| Recovery code | What the person keeps on paper or in a password manager to recover the account when every device is lost. |
| Device | A member of kind 1 in its certificate: an app's database on a phone, a laptop or in a browser, run by a person. |
| Agent | A member of kind 2: software that runs for an account. It never holds the account key. |
| Pairing | Adding a device or an agent to an account: it receives a certificate, and a device also receives the account key. |
| Account space | An ordinary space whose members are the account's devices, holding the account's own records. |
| Per-database account | The account key each database draws for itself on first open today ([RFC 0003, decision 17](0003-keys-and-cryptography.md#17-the-account-key-before-the-accounts-rfc)). |
| Linking | Turning a per-database account into a part of another account. |
| Fingerprint | A short, readable digest of the account's identity, which people compare once. |

## Where things stand today

- [RFC 0003](0003-keys-and-cryptography.md#the-account-key) defines the account key and decides where it lives: on every device of the account, plus a recovery copy the person keeps ([decision 8](0003-keys-and-cryptography.md#8-where-the-account-key-lives), Carsten, 2026-11-08). It defines the member certificate's bytes (decision 15) and leaves revoking a certificate to this RFC ([Not defined here](0003-keys-and-cryptography.md#not-defined-here)).
- [RFC 0008](0008-membership.md) carries the creator's certificate in the [genesis](0008-membership.md#genesis) and each new member's in its [membership entry](0008-membership.md#membership-entry). One rule reads the account: an erasure is signed off by another account that holds an admin member in the space ([Roles and rights](0008-membership.md#roles-and-rights), decision M9).
- [RFC 0007](0007-relay.md) lets members of allowlisted accounts create spaces (decision R6), and states that the relay sees which members share an account across spaces ([What the relay sees](0007-relay.md#what-the-relay-sees)).
- [RFC 0009](0009-sdk-api.md#accounts-and-devices) names three entry points, `db.account.pair`, `nc.open({ pairing })` and `db.account.removeMember`, and leaves what happens behind them to this RFC.
- Until this RFC is built, each database draws its own account key once and keeps it next to its member key, in the same keystore or, for a headless agent, the same key file; an `agent()` option marks the member as an agent ([RFC 0003, decisions 16 and 17](0003-keys-and-cryptography.md#17-the-account-key-before-the-accounts-rfc); [RFC 0009, "Accounts and devices"](0009-sdk-api.md#accounts-and-devices)). So a person's laptop and phone are two accounts, and an agent is an account of its own. RFC 0003 asks this RFC to accept the members certified this way, for example by letting a later pairing vouch for them. [Linking accounts drawn per database](#linking-accounts-drawn-per-database) does that.

## The account key and where it lives

The account key is an Ed25519 key pair, as RFC 0003 defines it. It is drawn on the account's first device when the account is created; today, that is every database's first open.

**Where it lives** was decided in RFC 0003: on every device of the account ([decision 8](0003-keys-and-cryptography.md#8-where-the-account-key-lives)). The SDK keeps it where it keeps the member key, in the platform keystore, and never hands its bytes to the app ([RFC 0003, "How the SDK hides keys"](0003-keys-and-cryptography.md#how-the-sdk-hides-keys)). In the browser, a non-extractable WebCrypto key wraps it in IndexedDB, and a browser that evicts the origin's data takes the account key with it ([RFC 0006, "Durability of a browser copy"](0006-local-view.md#durability-of-a-browser-copy)). The recovery code is then the way back, as for a lost phone. RFC 0003 did not take the other two places it weighed: an account key derived from a passkey, and an account key only on designated admin devices, which may come back later as an option for organisations.

**Who never holds it.** An agent or a sub-agent never holds the account key, following RFC 0003, whose holders are "the account's devices" and which keeps the account key from a parent agent ([Sub-agents get an ad-hoc space](0003-keys-and-cryptography.md#sub-agents-get-an-ad-hoc-space)). An agent holds only the certificate its account signed for it on a device. The relay never holds it either: an account at the relay, with a password that unlocks an account key held there, would let the relay or a backup of it hold a key wrapped by a secret people choose, which can be guessed offline, and that breaks the first principle ([GENESIS.md, "Principles, in priority order"](../../factory/GENESIS.md#principles-in-priority-order)). An organisation that runs agents does so under an account whose key lives on the devices of the people who answer for those agents.

**One database is one member** ([RFC 0006, "Where the view lives"](0006-local-view.md#where-the-view-lives)). Two apps on one laptop are two databases, so two members, and each keeps its own copy of the account key and pairs on its own. Phones and browsers keep apps apart, so letting apps share a keystore entry where a desktop platform allows it would make accounts behave differently by platform, which principle 7 rules out. The cost is that a person pairs each app once.

**What it signs.** Member certificates of the account's devices and agents ([RFC 0003, "Member certificate"](0003-keys-and-cryptography.md#member-certificate)); the sign-off of an erasure; and the link statements of [Linking accounts drawn per database](#linking-accounts-drawn-per-database).

### The account's identity

The account's identity is a **recovery key**, not the account key. The account key lives on every device, so a device that is lost or stolen carries it away. If the account key were the identity, nothing could take it back: a lost phone would sign certificates and erasure sign-offs in the person's name for as long as the account exists, and the only answer would be a new account. The recovery key derives from the recovery code, and no device keeps it. It signs an **account key statement** that names the current account key and a number. The account's fingerprint is the recovery key's, so it stays the same when the account key changes. A lost device is answered by a new account key, with the next number, signed with the recovery key. The cost is that the person needs the recovery code to answer a lost device, and that RFC 0003's certificate and RFC 0008's check change, which costs least before the first release, while no certificate outside development exists.

```text
account key statement (CBOR map, canonical)
├── 1 recovery key     32 bytes, the account's identity, an Ed25519 public key
├── 2 account key      32 bytes, the current account key
├── 3 number           uint: 1 for the first account key, one more with each new one
└── 4 signature        64 bytes: the recovery key signs
                       H("nocentraldb/account-key-statement" || canonical map of keys 1 to 3)
```

The member certificate carries the statement as a new key, so a member checks a chain of two: the statement against the recovery key, then the certificate against the account key it names. The statement is the only new thing that is signed; it follows RFC 0003's rule for every signed input, a context string of its own and a canonical map ([Formats of what keys sign and derive](0003-keys-and-cryptography.md#formats-of-what-keys-sign-and-derive)). RFC 0003's certificate format and RFC 0008's check change with it ([Changes to the decided RFCs](#changes-to-the-decided-rfcs)).

What the higher number does, and where it stops, is in [Losing a device](#losing-a-device).

### The fingerprint

People compare an account's fingerprint once, the way messaging apps compare safety numbers, when an account is new to their device or when they pair a device. It is SHA-256 over the context string `"nocentraldb/account-fingerprint"` and the recovery key, shown as 60 digits in twelve blocks of five: the 32-byte hash is read as one big-endian number and taken modulo 10^60, and the result is written as 60 decimal digits with leading zeros, in twelve blocks of five separated by one space. Both SDKs show the same digits, so a person comparing them on two devices sees no mismatch. That compares about 199 bits, and the bias of the modulo is negligible, about 10^-17. Signal's form of safety numbers, five bytes per block taken modulo 100000, would need 60 bytes and so a second hash under its own context string: more to specify, for no practical gain. Digits read aloud the same in every language, and messaging apps have taught people to compare them. The first 16 bytes of the hash in hexadecimal would be harder to read aloud, and words from the recovery code's list would invite people to confuse the fingerprint with the code.

## What a member certificate means for RFC 0008

RFC 0003 defines the certificate's bytes and RFC 0008 where it stands. This is what it means:

1. **It says who answers for a member. It gives no right.** RFC 0008 gives rights only through membership entries. A valid certificate admits nobody: an admin's add does, after its SDK checked the certificate ([RFC 0003, "Sharing a space key with a new member"](0003-keys-and-cryptography.md#sharing-a-space-key-with-a-new-member)). Pairing a device to an account therefore puts it in no space; [Adding a device or an agent to an account](#adding-a-device-or-an-agent-to-an-account) says which spaces it joins and who adds it.
2. **It is checked once, when the member enters a space,** in the genesis that creates the space or in the add that admits the member, and it stays fixed in that entry. The fold never reads it again. Linking two accounts, or a new account key, does not rewrite an entry already written.
3. **One rule of RFC 0008 reads the account:** the sign-off of an erasure, by an account that holds an admin member in the space and is not the performer's account (decision M9). "Account" there means the account's identity, its recovery key, so a new account key after a lost device does not make a second account ([Changes to the decided RFCs](#changes-to-the-decided-rfcs)). Two accounts of one person still count as two. That holds without per-database accounts too, because anybody can draw a second account at any time: what protects the rule is that an admin made that account's member an admin, not that accounts are people.
4. **A sub-agent answers through its parent.** Its certificate is signed by its parent agent and names the parent's account ([RFC 0008, decision M25](0008-membership.md#m25-who-signs-a-sub-agents-certificate)), so the chain leads from the sub-agent through the parent to the account. Nothing in this RFC changes that.
5. **Its kind is shown, not enforced,** except where RFC 0008 already uses it: an agent added without a role joins as a writer (decision M14), and `members.list()` shows every member's kind ([RFC 0009, "Invitations and members"](0009-sdk-api.md#invitations-and-members)).
6. **There is still no revocation of a certificate.** Removing a member from every space it belongs to stays the revocation, as RFC 0003 and RFC 0008 decided. What this RFC adds is a way to stop new certificates in the account's name after a device is lost: a new account key.
7. **The relay reads it** to apply decision R6, whose allowlist names accounts, and so learns which members share an account (see [What the relay learns](#what-the-relay-learns)).

## The recovery code

The recovery code is made when the account is created, on its first device. The SDK hands it to the app once, for the person to write down or store in a password manager, as an object whose string, JSON and debug forms are redacted so that it never reaches a log ([GENESIS.md, "What it must never do"](../../factory/GENESIS.md#what-it-must-never-do)). The app shows the words and calls `confirm()`. Apps own their screens, so the SDK does not ship its own screen, which would be a user interface on every platform; and the SDK does not store the code in the platform's password manager itself, because that would put it on the devices it is meant to outlive. It is the one way a private key leaves a device that RFC 0003 allows: "the account recovery flow, which a person runs, not an app".

**Its form** is twelve words from the BIP-39 English word list. They encode 128 random bits and a 4-bit checksum. The words are only an encoding: BIP-39's step that stretches the words with PBKDF2 into a wallet seed is not used. The recovery key derives from the 128 bits with HKDF-SHA256 under its own context string, `"nocentraldb/recovery-key"`, as RFC 0003 derives its other subkeys. 128 bits match the strength of Ed25519 itself, twelve words are copied by hand with fewer mistakes than twenty-four words or 26 characters in Crockford's base32, and people recognise the word list. Twenty-four words could encode the 32-byte Ed25519 seed itself, but would only make the code longer to copy for no strength the key has.

**What the device keeps.** The device forgets the recovery code once the app confirms that the person stored it, and keeps only the recovery key's public half. A stolen device therefore holds no recovery code.

**Recovering.** On a new device, the person enters the code. The SDK derives the recovery key, draws a new account key, signs its statement with the next number, certifies the new device's member key and forgets the recovery key again. The account comes back with its fingerprint unchanged.

**What it restores** is the account, not the spaces. The new device is a member of nothing yet. A space comes back when another member of it, a person's other account or an agent, adds the new device. A space whose every member is lost stays unreadable, because the relay holds only ciphertext and nobody holds a key to it. The SDK says so where it matters: when an account's only device is the only member of a space, the space's status says that losing this device loses the space. An app that must not lose a space adds a second member to it, another person or an agent. Recovery reaches no further on purpose: a recovery member derived from the code, added as a reader to every space the account administers, would put a member nobody runs into every member list, and a backup of the space keys at the relay, sealed under a key derived from the code, would need a new store at the relay. With either, a leaked code would read everything instead of only signing. Splitting the code among other people, social recovery, is more for people to manage and to explain; it can come later on top of a recovery key.

**Losing the code.** While a device of the account remains, the person can still use the account, but nobody can make a new account key, so the account can no longer be answered after a lost device; the person makes a new account and links the old one into it. A new recovery code for the same account is not possible without the old one, because the old one is the identity.

## Adding a device or an agent to an account

Pairing uses two codes and no server. It reuses RFC 0009's invitation as the first code:

```ts
// 1. On the new device, a database opened for pairing shows its invitation:
const db = await nc.open({ model, lock, pairing: "request" });
const invitation = await db.invitation();      // public: member id, public keys, kind; no certificate yet
// The app shows it as a QR code (a device) or as text (an agent in a terminal).

// 2. On a device of the account, after the person scanned it and confirmed:
const answer = await db.account.pair({ kind: "device", invitation });
// The app shows the answer as a QR code or text.

// 3. Back on the new device:
await db.account.completePairing(answer);       // certificate, and for a device the account key
```

RFC 0009 named `db.account.pair` and `nc.open({ pairing })` and left the rest to this RFC, which names the others. Linking uses the same calls, because linking is pairing a database that already has data, so one flow and one set of names covers both, without a separate `db.account.link`. `nc.open({ model, lock, recover: words })` recovers an account on a new device.

What happens underneath:

1. **The new device** draws its member key, as on every first open, and shows its invitation without a certificate, as a sub-agent's invitation already is ([RFC 0009, "Sub-agents and ad-hoc spaces"](0009-sdk-api.md#sub-agents-and-ad-hoc-spaces)). Nothing in it is secret.
2. **The device of the account** reads it, and the person confirms on that device that this is their new device or their agent. A device is confirmed only by a scanned code, which proves the two devices are side by side, or by a full fingerprint compared on both screens, because the answer carries the account key: a swapped invitation would hand the account to an attacker. An agent may be paired through any channel the person trusts, such as text copied from the agent's terminal, because it receives no account key, only a certificate, which a person can remove again. A short code compared on both screens is not enough for either: an attacker who draws keys until a short digest matches defeats it, and making it safe needs a commitment protocol we would design ourselves, which GENESIS.md forbids.
3. **The answer** carries the certificate the account key signed, and the account key statement. For a device, it also carries the account key, sealed with HPKE to the new device's agreement key, the same construction as RFC 0003's [key envelope](0003-keys-and-cryptography.md#key-envelope) under its own `info` label, `"nocentraldb/account-key"`. A camera that films the answer learns nothing it can open.
4. **The new device** checks the certificate against its own member id, opens the account key, and is a device of the account. Through the [account space](#the-account-space), it learns the account's other devices and spaces.

On pairing, the account key passes neither through the relay nor through the account space: it travels once, sealed, from one device to the other. Pairing works in island mode, because the two codes need no network, and the relay learns nothing of it. The cost is two scans, or two copy-pastes for an agent. One code and a mailbox at the relay for the rest would need a new request in RFC 0007 and tell the relay when pairings happen; a short code typed on the new device, with a password-authenticated key exchange (PAKE) through such a mailbox, would also need a PAKE crate whose audit we would first have to establish.

**Which spaces a new device joins.** The SDK adds a new device to every space where a device of the account is an admin, with the role the account's devices hold there, and returns one notice per space, because it changes who can read ([GENESIS.md, "What it must never do"](../../factory/GENESIS.md#what-it-must-never-do)). A person expects their new phone to show their data, so the app does not have to add it space by space. Where no device of the account is an admin, the SDK lists the spaces, and the app asks those spaces' admins with the new device's invitation, which their SDK accepts without a fingerprint check, because the account is known to it. Other admins' SDKs do not admit a certified device of a known account on their own: that would let a certificate admit a member without any admin deciding, which RFC 0008 never allows. An agent joins no space automatically: it gets the spaces its task needs and no more.

### The account space

The account's devices need records they share: which devices and agents the account certified, which spaces each device belongs to, which accounts are linked into it, and the account key statements. They live in an **account space**: an ordinary space under RFC 0008, created by the account's first device, whose members are the account's devices, each an admin. Its model is one the SDK ships. It needs no new format: its genesis, its adds and its key envelopes are RFC 0008's, and its records are sealed content the relay cannot read. With no shared record, each device would know only what it did itself: `removeMember` could not find the spaces of a lost device it is not in, and a new device could not learn which spaces to ask for. The cost is that the relay sees one more space per account and when devices join it.

Its content never holds the account key or the recovery code in a form its members can read. Content is sealed under the space key, so every device that joins later would walk the key chain back ([RFC 0003, decision 5](0003-keys-and-cryptography.md#5-what-a-new-member-reads)) and read every account key the account ever had, and an append-only log never forgets one. When a new account key must reach the remaining devices ([Losing a device](#losing-a-device)), the account space carries it wrapped with HPKE to each device's agreement key, as a key envelope wraps a space key, so that only that device opens it.

## Linking accounts drawn per database

Under RFC 0003's decision 17, each database has an account of its own, and each such account has exactly one member. Linking makes a per-database account part of another account, the one whose recovery code the person keeps.

1. **The database with the old account pairs** with a device of the account it joins, as a new device does, with the same two codes. Its member key and member id stay what they are: the member id covers the public keys, not the certificate ([RFC 0002, "Author and author sequence"](0002-change-and-log-format.md#author-and-author-sequence)).
2. **Both account keys sign a link statement**, "account X is part of account Y", which the account space keeps. The old key's signature shows that the holder of X agreed; nobody else can link X into their account.
3. **The old account key is deleted** from the database's keystore or key file. Nobody can make a new certificate in the old account's name. An agent that drew its own account key keeps none: after linking, it holds an agent certificate of the account it joined, as any agent does.
4. **New spaces and new adds use the new certificate.**

**Memberships already written** keep the old certificate, because RFC 0008's entries are fixed (see [What a member certificate means for RFC 0008](#what-a-member-certificate-means-for-rfc-0008)). They stay as written: the person's own SDK shows them as one account through the link statement, and an invitation carries the link statements of its account, so that other members' SDKs can show "linked to" next to the old fingerprint. This is local display only: the fold and the relay keep counting the old account as an account of its own, for the sign-off of an erasure and for the allowlist of decision R6. That limit, two accounts in the fold for one person, is no weaker than what anybody can do by drawing a second account. Leaving them as written costs no rotation and changes no format. To re-add each such member with the new certificate, where the account administers the space, would cost one rotation per space, and a new membership action that replaces a member's certificate would change RFC 0008's control bodies.

Linking needs the old database. A per-database account whose database is gone cannot be linked, and its memberships stay as they are.

## Losing a device

`db.account.removeMember(memberId)`, run on another device of the account ([RFC 0009, "Accounts and devices"](0009-sdk-api.md#accounts-and-devices)):

1. Removes the lost member from every space where a device of the account is an admin, which rotates each space key ([RFC 0003, "Removing a member and rotating the space key"](0003-keys-and-cryptography.md#removing-a-member-and-rotating-the-space-key)), and from the account space. The account space says which spaces those are, including ones the running device is not in, which it lists for the app.
2. Lists the spaces where the lost member is a member and no device of the account is an admin, so the app can ask their admins to remove it.
3. Asks for the recovery code, draws a new account key with the next number, and hands it to each remaining device through the account space, wrapped to that device's agreement key as a pairing answer is (see [The account space](#the-account-space)). Each remaining device receives a new certificate.

**What the higher number does.** An honest member's SDK that holds statement n refuses to write an add for a certificate under an older statement, and refuses an erasure sign-off by an older account key. That is a check the SDK makes before it writes, not a rule of the fold: the fold of RFC 0008 may read only a change's causal past, and no control entry carries a statement into a space yet. A member that has not yet learnt statement n still accepts the old key, and a change folded before stays folded. Writing statements into spaces, so that the fold itself refuses the old key, needs a control entry RFC 0008 does not have yet ([Changes to the decided RFCs](#changes-to-the-decided-rfcs)).

**What the lost device keeps:** its copy of every space it was in, as RFC 0003 says plainly, and the account key, valid to every member that has not learnt the next statement.

## What the relay learns

The relay already sees each member's certificate, so which members share an account across spaces, and the sub-agent tree ([RFC 0007, "What the relay sees"](0007-relay.md#what-the-relay-sees)). This RFC adds:

- **The account space**: one more space per account, whose members are exactly the account's devices, and when a device joins it or leaves it, so roughly when the person pairs a device or loses one.
- **Account key statements**, once certificates carry them: how often an account changed its account key, which most often means a lost device.
- **Links**, indirectly: the link statements are sealed in the account space, but a member that still carries an old account's certificate joining another account's space links the two accounts for the relay.

It learns no key. The account key travels sealed between two devices, mostly not through the relay at all, and the recovery code never leaves the person. Pairing with two codes involves no relay, so the relay learns nothing of a pairing until the account space changes.

## What this does not do

- **Recovery does not bring back a space whose every member was lost.** The relay holds only ciphertext and nobody holds its key.
- **A lost device keeps its copy**, of the spaces and of the account key. Removal stops it from reading what is written next; nothing takes back what it holds.
- **The new account key does not reach a member that has not learnt it**, and the fold does not refuse the old key until a statement can be written into a space.
- **Linking does not rewrite memberships.** Other members, the fold and the relay may see two accounts where the person sees one.
- **An account is not a person.** Anybody can draw any number of accounts. The rule that a second account signs off an erasure rests on the admin role, not on accounts being people.
- **Losing every device and the recovery code loses the account.** Nobody can restore it, by design: no server holds anything that could.
- **No directory.** Nothing finds an account by name, address or phone number; people compare fingerprints once, the way messaging apps compare safety numbers.
- **Not post-quantum**, as RFC 0003 says for v1 (decision 9).

## Agreement with GENESIS.md and the decided RFCs

| GENESIS.md says | This RFC |
| --- | --- |
| "The relay never reads content." | No key reaches the relay; the account space is sealed content; pairing passes beside it. |
| "Never invent cryptography." | Ed25519, HKDF-SHA256 and HPKE as RFC 0003 uses them; BIP-39 only as a word list. No short-code protocol. |
| "Never give an agent more than its spaces." | An agent never holds the account key and joins no space automatically. |
| "A developer doesn't need to care about keys." | The app handles invitations, answers and a recovery code it shows once; never key bytes. |
| "Never claim more than the design delivers." | [What this does not do](#what-this-does-not-do) lists what recovery, removal and linking do not reach. |
| "Never change how permissions or spaces behave without telling them plainly." | Every automatic add returns a notice; the status says when losing a device loses a space. |

**Conflicts.** None with GENESIS.md.

### Changes to the decided RFCs

Some choices change other texts. Each change is written into that text in its own pull request, as [How a decided RFC changes](README.md#how-a-decided-rfc-changes) says:

- **RFC 0003**, from the account's identity, the recovery code and the fingerprint: the member certificate carries the account key statement; the key table names the recovery key, held by no device; the sentence that the person keeps "a recovery copy" of the account key becomes the recovery code that derives the recovery key; and the recovery key's derivation, the account key statement, the `info` label `"nocentraldb/account-key"` of a pairing answer and the fingerprint's context string join "Formats of what keys sign and derive".
- **RFC 0008**, from the account's identity: a member checks the chain from the recovery key through the statement to the certificate; accounts are compared by the recovery key, also for the sign-off of an erasure (decision M9); and a control entry that carries a newer statement into a space, so that the fold refuses certificates and sign-offs by an older account key.
- **RFC 0009**, from the names of pairing, linking and recovery and the recovery code object: `nc.open({ pairing: "request" })`, `db.account.pair({ kind, invitation })`, `db.account.completePairing(answer)`, `nc.open({ recover })` and the recovery code object with `confirm()` join "Accounts and devices" and "Public names".
- **The FAQ**: its answer that the recovery code "restores your account key" becomes that it makes a new account key under the same account.

## History

- **2026-11-10** · First published (RFC 0001 to RFC 0010 rewritten as fresh texts, NOC-165).

## How this RFC is tested

`scripts/rfc-0010.test.mjs` checks the shape of this RFC: that the RFC index lists it, that its status says the owner decided it and when, that it links GENESIS.md and the RFCs it builds on and every link resolves, that it has a section for every part NOC-107 names, that it reads as one design with no list of decisions, rejected options or open questions and no reference to a question's number, that every option not taken still stands in the text, and that the decision log records it. Whether the design is right is the owner's review.
