# RFC 0010: Accounts

REQUEST FOR COMMENTS · DRAFT 0.1 · NOVEMBER 2026 · Ticket NOC-107

This RFC drafts accounts: the account key and where it lives, the recovery code, how a device or an agent joins an account, how the accounts that each database draws today are linked into one, and what a member certificate signed by an account means for membership. It turns the sketch in [RFC 0003, "The account key"](0003-keys-and-cryptography.md#the-account-key) into a design, so that a person's devices can be linked before the first release, as the owner asked when deciding that each database draws its own account key for now (NOC-95, choice 2 A, Carsten, 2026-11-10).

It builds on [RFC 0001](0001-nocentraldb.md), above all [the design in one page](0001-nocentraldb.md#the-design-in-one-page); on [RFC 0003](0003-keys-and-cryptography.md), which defines the account key, the [member certificate](0003-keys-and-cryptography.md#member-certificate) and the [account sign-off of a shred](0003-keys-and-cryptography.md#account-sign-off-of-a-shred); on [RFC 0008](0008-membership.md), which places certificates in the genesis and in membership entries; on [RFC 0007](0007-relay.md), whose relay sees certificates; on [RFC 0006](0006-local-view.md), which keeps one database per member; and on [RFC 0009](0009-sdk-api.md), which names the entry points this RFC designs ([Accounts and devices](0009-sdk-api.md#accounts-and-devices), decision S15).

**How to give feedback.** This RFC has no discussion of its own yet: start one in the category [RFC feedback](https://github.com/nocentraldb/nocentraldb/discussions/categories/rfc-feedback), named after the RFC, or open a pull request against this file to change its text. [Giving feedback](README.md#giving-feedback) says how, and what happens to your point.

**Status.** Draft 0.1. Nothing in it is decided. GENESIS.md reserves crypto, key handling, the wire and on-disk formats and public API names for the owner ([What has to come back through me](../../factory/GENESIS.md#what-has-to-come-back-through-me)), and every choice this RFC asks for falls under one of them. Each is an [open question](#open-questions), with options and a recommendation, for the owner to decide. This draft changes no decided RFC: where an answer would change one, the question names the RFC and the change, which follows the owner's decision. Nothing here is built.

**Out of scope.** How a work order is claimed and signed off, beyond naming the account key as the key that signs (a later RFC on work orders). Passkeys and how the browser unlocks its keys (RFC 0003 leaves them out). Accounts shared by several people, such as an organisation with many administrators, beyond what one account with several devices gives. Deleting a space. A directory that finds an account by name or address: there is none, and people compare fingerprints.

## Summary

| Question | Answer this draft recommends |
| --- | --- |
| What is an account? | A key pair that answers for members: it signs the certificates of its devices and agents, and the sign-off of an erasure. It is not a login, and no server holds it. |
| Where does the account key live? | On every device of the account, in the platform keystore next to the member key, as RFC 0003 decided. Never on an agent, never at the relay. |
| What is the account's identity? | A recovery key that only the recovery code holds. It signs a statement naming the current account key, so a lost device can be answered with a new account key ([Q1](#open-questions)). |
| What is the recovery code? | Twelve words that hold 128 random bits; the recovery key derives from them ([Q2](#open-questions)). |
| What does it restore? | The account, not the spaces: a new device under the account, which other members then add to their spaces ([Q3](#open-questions)). |
| How does a device join? | Two codes: the new device shows its invitation, a device of the account reads it, the person confirms, and the answer carries the certificate and the sealed account key back ([Q5](#open-questions), [Q6](#open-questions)). |
| How are per-database accounts linked? | A database with its own account pairs like a new device; both account keys sign a link; the old key is deleted; memberships already written keep their certificate ([Q9](#open-questions)). |
| What does a certificate mean for RFC 0008? | Who answers for a member. It is checked once, when the member enters a space, and gives no right of its own. |

## Terms

| Term | Meaning in this RFC |
| --- | --- |
| Member, member key, member id | As in RFC 0003 and RFC 0008: a device, an agent or a sub-agent, its two key pairs, and the id that covers both public keys. |
| Account | Whatever answers for a set of members. Today it is one key pair; [Q1](#open-questions) asks whether it becomes two. |
| Account key | The Ed25519 signing key that signs member certificates and erasure sign-offs (RFC 0003). |
| Recovery key | Proposed in [Q1](#open-questions): an Ed25519 key derived from the recovery code, held by no device, that names the current account key. |
| Account key statement | Proposed in [Q1](#open-questions): the recovery key's signature on "the account key is K, number n". |
| Recovery code | What the person keeps on paper or in a password manager to recover the account when every device is lost. |
| Device | A member of kind 1 in its certificate: an app's database on a phone, a laptop or in a browser, run by a person. |
| Agent | A member of kind 2: software that runs for an account. It never holds the account key. |
| Pairing | Adding a device or an agent to an account: it receives a certificate, and a device also receives the account key. |
| Account space | Proposed in [Q4](#open-questions): an ordinary space whose members are the account's devices, holding the account's own records. |
| Per-database account | The account key each database draws for itself on first open today (NOC-95, choice 2 A). |
| Linking | Turning a per-database account into a part of another account. |
| Fingerprint | A short, readable digest of the account's identity, which people compare once ([Q12](#open-questions)). |

## Where things stand today

- [RFC 0003](0003-keys-and-cryptography.md#the-account-key) defines the account key and decides where it lives: on every device of the account, plus a recovery copy the person keeps ([decision 8](0003-keys-and-cryptography.md#decisions-for-the-owner), Carsten, 2026-11-08). It defines the member certificate's bytes (decision 15) and leaves revoking a certificate to this RFC ([Not defined here](0003-keys-and-cryptography.md#not-defined-here)).
- [RFC 0008](0008-membership.md) carries the creator's certificate in the [genesis](0008-membership.md#genesis) and each new member's in its [membership entry](0008-membership.md#membership-entry). One rule reads the account: an erasure is signed off by another account that holds an admin member in the space ([Roles and rights](0008-membership.md#roles-and-rights), decision M9).
- [RFC 0007](0007-relay.md) lets members of allowlisted accounts create spaces (decision R6), and states that the relay sees which members share an account across spaces ([What the relay sees](0007-relay.md#what-the-relay-sees)).
- [RFC 0009](0009-sdk-api.md#accounts-and-devices) names three entry points, `db.account.pair`, `nc.open({ pairing })` and `db.account.removeMember`, and leaves what happens behind them to this RFC.
- Since NOC-95, each database draws its own account key once and keeps it next to its member key; an `agent()` option marks the member as an agent. So a person's laptop and phone are two accounts, and an agent is an account of its own. This RFC is what links them.

## The account key and where it lives

The account key is an Ed25519 key pair, as RFC 0003 defines it. It is drawn on the account's first device when the account is created; today, that is every database's first open.

**Where it lives** is decided: on every device of the account ([RFC 0003, decision 8](0003-keys-and-cryptography.md#decisions-for-the-owner)). The SDK keeps it where it keeps the member key, in the platform keystore, and never hands its bytes to the app ([RFC 0003, "How the SDK hides keys"](0003-keys-and-cryptography.md#how-the-sdk-hides-keys)). In the browser, a non-extractable WebCrypto key wraps it in IndexedDB, and a browser that evicts the origin's data takes the account key with it ([RFC 0006, "Durability of a browser copy"](0006-local-view.md#durability-of-a-browser-copy)). The recovery code is then the way back, as for a lost phone.

**Who never holds it.** An agent or a sub-agent never holds the account key, following RFC 0003, whose holders are "the account's devices" and which keeps the account key from a parent agent ([Sub-agents get an ad-hoc space](0003-keys-and-cryptography.md#sub-agents-get-an-ad-hoc-space)). An agent holds only the certificate its account signed for it on a device. The relay never holds it. An organisation that runs agents does so under an account whose key lives on the devices of the people who answer for those agents.

**One database is one member** ([RFC 0006, decision 1](0006-local-view.md#1-how-many-databases-per-member)). Two apps on one laptop are two databases, so two members. Whether they share one copy of the account key on that laptop is [Q8](#open-questions).

**What it signs.** Member certificates of the account's devices and agents ([RFC 0003, "Member certificate"](0003-keys-and-cryptography.md#member-certificate)); the sign-off of an erasure; and, as this draft proposes, the link statements of [Linking accounts drawn per database](#linking-accounts-drawn-per-database).

### The account's identity

Today, the account key is the account's identity: a certificate names it, and RFC 0008 compares accounts by it. Because it lives on every device, every device that is lost or stolen carries the identity away with it, and nothing can take it back. This RFC asks the owner to choose between two models ([Q1](#open-questions)):

- **The account key is the identity**, as RFC 0003 writes the certificate today. The recovery code is a copy of the account key. Nothing in any format changes. A lost device can certify new members and sign off erasures in the account's name for as long as the account exists; the answer to a stolen device is a new account.
- **A recovery key above the account key.** The recovery code derives a recovery key that no device keeps. It signs an account key statement that names the current account key and a number. The account's fingerprint is the recovery key's, so it stays the same when the account key changes. A lost device is answered by a new account key, with the next number, signed with the recovery key. This draft recommends it.

The proposed statement, for the owner's review:

```text
account key statement (CBOR map, canonical)          proposed, Q1 B
├── 1 recovery key     32 bytes, the account's identity, an Ed25519 public key
├── 2 account key      32 bytes, the current account key
├── 3 number           uint: 1 for the first account key, one more with each new one
└── 4 signature        64 bytes: the recovery key signs
                       H("nocentraldb/account-key-statement" || canonical map of keys 1 to 3)
```

The member certificate would carry the statement as a new key, so a member checks a chain of two: the statement against the recovery key, then the certificate against the account key it names. The statement is the only new thing that is signed; it follows RFC 0003's rule for every signed input, a context string of its own and a canonical map ([Formats of what keys sign and derive](0003-keys-and-cryptography.md#formats-of-what-keys-sign-and-derive)). It is a change to RFC 0003's certificate format and to RFC 0008's check, which the owner's decision would carry out in those RFCs.

What the higher number does, and where it stops, is in [Losing a device](#losing-a-device).

## What a member certificate means for RFC 0008

RFC 0003 defines the certificate's bytes and RFC 0008 where it stands. This is what it means, under either model:

1. **It says who answers for a member. It gives no right.** RFC 0008 gives rights only through membership entries. A valid certificate admits nobody: an admin's add does, after its SDK checked the certificate ([RFC 0003, "Sharing a space key with a new member"](0003-keys-and-cryptography.md#sharing-a-space-key-with-a-new-member)). Pairing a device to an account therefore puts it in no space; [Adding a device or an agent to an account](#adding-a-device-or-an-agent-to-an-account) says which spaces it joins and who adds it.
2. **It is checked once, when the member enters a space,** in the genesis that creates the space or in the add that admits the member, and it stays fixed in that entry. The fold never reads it again. Linking two accounts, or a new account key, does not rewrite an entry already written.
3. **One rule of RFC 0008 reads the account:** the sign-off of an erasure, by an account that holds an admin member in the space and is not the performer's account (decision M9). "Account" there means the key that signed the certificates. Two accounts of one person count as two. That holds without per-database accounts too, because anybody can draw a second account at any time: what protects the rule is that an admin made that account's member an admin, not that accounts are people. Under the recovery-key model, the account RFC 0008 compares would be the recovery key, which is part of the change [Q1](#open-questions) names.
4. **A sub-agent answers through its parent.** Its certificate is signed by its parent agent and names the parent's account ([RFC 0008, decision M25](0008-membership.md#decisions-for-the-owner)), so the chain leads from the sub-agent through the parent to the account. Nothing in this RFC changes that.
5. **Its kind is shown, not enforced,** except where RFC 0008 already uses it: an agent added without a role joins as a writer (decision M14), and `members.list()` shows every member's kind ([RFC 0009, "Invitations and members"](0009-sdk-api.md#invitations-and-members)).
6. **There is still no revocation of a certificate.** Removing a member from every space it belongs to stays the revocation, as RFC 0003 and RFC 0008 decided. What this RFC adds is a way to stop new certificates in the account's name after a device is lost, under the recovery-key model.
7. **The relay reads it** to apply decision R6, whose allowlist names accounts, and so learns which members share an account (see [What the relay learns](#what-the-relay-learns)).

## The recovery code

The recovery code is made when the account is created, on its first device. The SDK hands it to the app once, for the person to write down or store in a password manager ([Q11](#open-questions)). It is the one way a private key leaves a device that RFC 0003 allows: "the account recovery flow, which a person runs, not an app".

**Its form** is [Q2](#open-questions). This draft recommends twelve words from the BIP-39 English word list, which encode 128 random bits and a 4-bit checksum. The words are only an encoding: BIP-39's step that stretches the words with PBKDF2 into a wallet seed is not used. The recovery key derives from the 128 bits with HKDF-SHA256 under its own context string, `"nocentraldb/recovery-key"`, as RFC 0003 derives its other subkeys. 128 bits match the strength of Ed25519 itself, and twelve words are short enough to copy by hand without a mistake the checksum misses.

**What the device keeps.** Under the recovery-key model, the device forgets the recovery code once the app confirms that the person stored it, and keeps only the recovery key's public half. A stolen device then holds no recovery code. Under the other model, the code is the account key, which every device holds anyway.

**Recovering.** On a new device, the person enters the code. The SDK derives the recovery key, draws a new account key, signs its statement with the next number, certifies the new device's member key and forgets the recovery key again. The account comes back with its fingerprint unchanged. Under the other model, the code restores the old account key itself.

**What it restores** is [Q3](#open-questions). This draft recommends: the account, not the spaces. The new device is a member of nothing yet. A space comes back when another member of it, a person's other account or an agent, adds the new device. A space whose every member is lost stays unreadable, because the relay holds only ciphertext and nobody holds a key to it. The SDK says so where it matters: when an account's only device is the only member of a space, the space's status says that losing this device loses the space.

**Losing the code.** While a device of the account remains, the person can still use the account. Under the recovery-key model, nobody can then make a new account key, so the account can no longer be answered after a lost device; the person makes a new account and links the old one into it. A new recovery code for the same account is not possible without the old one, because the old one is the identity. Under the other model, a device can show the account key as a new code, which is the same secret.

## Adding a device or an agent to an account

Pairing uses two codes and no server ([Q5](#open-questions)). It reuses RFC 0009's invitation as the first code:

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

The names are this draft's proposal and are [Q10](#open-questions); RFC 0009 names `db.account.pair` and `nc.open({ pairing })` and leaves the rest to this RFC.

What happens underneath:

1. **The new device** draws its member key, as on every first open, and shows its invitation without a certificate, as a sub-agent's invitation already is ([RFC 0009, "Sub-agents and ad-hoc spaces"](0009-sdk-api.md#sub-agents-and-ad-hoc-spaces)). Nothing in it is secret.
2. **The device of the account** reads it. The person confirms on that device that this is their new device or their agent ([Q6](#open-questions)). For a device, this draft recommends that only a scanned code or a full fingerprint compared on both screens confirms, because the answer carries the account key. An agent may be paired through any channel the person trusts, such as text copied from the agent's terminal, because it receives no account key.
3. **The answer** carries the certificate the account key signed, and the account key statement under the recovery-key model. For a device, it also carries the account key, sealed with HPKE to the new device's agreement key, the same construction as RFC 0003's [key envelope](0003-keys-and-cryptography.md#key-envelope) under its own `info` label, `"nocentraldb/account-key"`. A camera that films the answer learns nothing it can open.
4. **The new device** checks the certificate against its own member id, opens the account key, and is a device of the account. Through the [account space](#the-account-space), it learns the account's other devices and spaces.

On pairing, the account key passes neither through the relay nor through the account space: it travels once, sealed, from one device to the other. Pairing works in island mode, because the two codes need no network.

**Which spaces a new device joins** is [Q7](#open-questions). This draft recommends: the SDK adds a new device to every space where a device of the account is an admin, with the role the account's devices hold there, and returns one notice per space, because it changes who can read ([GENESIS.md, "What it must never do"](../../factory/GENESIS.md#what-it-must-never-do)). Where no device of the account is an admin, the SDK lists the spaces, and the app asks those spaces' admins with the new device's invitation, which their SDK accepts without a fingerprint check, because the account is known to it. An agent joins no space automatically: it gets the spaces its task needs and no more.

### The account space

The account's devices need records they share: which devices and agents the account certified, which spaces each device belongs to, which accounts are linked into it, and the account key statements. [Q4](#open-questions) asks whether they live in an **account space**: an ordinary space under RFC 0008, created by the account's first device, whose members are the account's devices, each an admin. Its model is one the SDK ships. It needs no new format: its genesis, its adds and its key envelopes are RFC 0008's, and its records are sealed content the relay cannot read.

Its content never holds the account key or the recovery code in a form its members can read. Content is sealed under the space key, so every device that joins later would walk the key chain back ([RFC 0003, decision 5](0003-keys-and-cryptography.md#decisions-for-the-owner)) and read every account key the account ever had. When a new account key must reach the remaining devices ([Losing a device](#losing-a-device)), the account space carries it wrapped with HPKE to each device's agreement key, as a key envelope wraps a space key, so that only that device opens it.

## Linking accounts drawn per database

Under NOC-95, each database has an account of its own, and each such account has exactly one member. Linking makes a per-database account part of another account, the one whose recovery code the person keeps.

1. **The database with the old account pairs** with a device of the account it joins, as a new device does, with the same two codes. Its member key and member id stay what they are: the member id covers the public keys, not the certificate ([RFC 0002, "Author and author sequence"](0002-change-and-log-format.md#author-and-author-sequence)).
2. **Both account keys sign a link statement**, "account X is part of account Y", which the account space keeps. The old key's signature shows that the holder of X agreed; nobody else can link X into their account.
3. **The old account key is deleted** from the database. Nobody can make a new certificate in the old account's name. An agent that drew its own account key keeps none: after linking, it holds an agent certificate of the account it joined, as any agent does.
4. **New spaces and new adds use the new certificate.**

**Memberships already written** keep the old certificate, because RFC 0008's entries are fixed (see [What a member certificate means for RFC 0008](#what-a-member-certificate-means-for-rfc-0008)). What happens to them is [Q9](#open-questions). This draft recommends to leave them as written: the person's own SDK shows them as one account through the link statement, and an invitation carries the link statements of its account, so that other members' SDKs can show "linked to" next to the old fingerprint. This is local display only: the fold and the relay keep counting the old account as an account of its own, for the sign-off of an erasure and for the allowlist of decision R6. It costs no rotation and changes no format.

Linking needs the old database. A per-database account whose database is gone cannot be linked, and its memberships stay as they are.

## Losing a device

`db.account.removeMember(memberId)`, run on another device of the account ([RFC 0009, "Accounts and devices"](0009-sdk-api.md#accounts-and-devices)):

1. Removes the lost member from every space where a device of the account is an admin, which rotates each space key ([RFC 0003, "Removing a member and rotating the space key"](0003-keys-and-cryptography.md#removing-a-member-and-rotating-the-space-key)), and from the account space. The account space says which spaces those are, including ones the running device is not in, which it lists for the app.
2. Lists the spaces where the lost member is a member and no device of the account is an admin, so the app can ask their admins to remove it.
3. Under the recovery-key model, asks for the recovery code, draws a new account key with the next number, and hands it to each remaining device through the account space, wrapped to that device's agreement key as a pairing answer is (see [The account space](#the-account-space)). Each remaining device receives a new certificate.

**What the higher number does.** An honest member's SDK that holds statement n refuses to write an add for a certificate under an older statement, and refuses an erasure sign-off by an older account key. That is a check the SDK makes before it writes, not a rule of the fold: the fold of RFC 0008 may read only a change's causal past, and no control entry carries a statement into a space yet. A member that has not yet learnt statement n still accepts the old key, and a change folded before stays folded. Writing statements into spaces, so that the fold itself refuses the old key, needs a control entry RFC 0008 does not have; it is part of what [Q1](#open-questions) would change.

**What the lost device keeps:** its copy of every space it was in, as RFC 0003 says plainly, and the account key. Under the identity model, that key stays valid for as long as the account exists.

## What the relay learns

The relay already sees each member's certificate, so which members share an account across spaces, and the sub-agent tree ([RFC 0007, "What the relay sees"](0007-relay.md#what-the-relay-sees)). This RFC adds:

- **The account space**: one more space per account, whose members are exactly the account's devices, and when a device joins it or leaves it, so roughly when the person pairs a device or loses one.
- **Account key statements**, under the recovery-key model, once certificates carry them: how often an account changed its account key, which most often means a lost device.
- **Links**, indirectly: the link statements are sealed in the account space, but a member that still carries an old account's certificate joining another account's space links the two accounts for the relay.

It learns no key. The account key travels sealed between two devices, mostly not through the relay at all, and the recovery code never leaves the person. Pairing with two codes involves no relay, so the relay learns nothing of a pairing until the account space changes.

## What this does not do

- **Recovery does not bring back a space whose every member was lost.** The relay holds only ciphertext and nobody holds its key ([Q3](#open-questions)).
- **A lost device keeps its copy**, of the spaces and of the account key. Removal stops it from reading what is written next; nothing takes back what it holds. Under the identity model, it can sign in the account's name for good.
- **The new account key does not reach a member that has not learnt it**, and the fold does not refuse the old key until a statement can be written into a space.
- **Linking does not rewrite memberships.** Other members, the fold and the relay may see two accounts where the person sees one.
- **An account is not a person.** Anybody can draw any number of accounts. The rule that a second account signs off an erasure rests on the admin role, not on accounts being people.
- **Losing every device and the recovery code loses the account.** Nobody can restore it, by design: no server holds anything that could.
- **No directory.** Nothing finds an account by name, address or phone number; people compare fingerprints once, the way messaging apps compare safety numbers.
- **Not post-quantum**, as RFC 0003 says for v1 (decision 9).

## Rejected options

| Option | Why not |
| --- | --- |
| An account at the relay, with a password that unlocks the account key held there | The relay or a backup of it would hold a key wrapped by a secret people choose, which can be guessed offline; that breaks the first principle ([GENESIS.md, "Principles, in priority order"](../../factory/GENESIS.md#principles-in-priority-order)). |
| An account key derived from a passkey | RFC 0003, decision 8, option (c), not taken by the owner. |
| The account key only on designated admin devices | RFC 0003, decision 8, option (b), not taken for v1; it may come back as an option for organisations. |
| The account key or the recovery code as readable content of the account space | Every device that joins later would read, back through the key chain, every account key the account ever had, and an append-only log never forgets one. A key wrapped to one device's agreement key reaches only that device. |
| A short code compared on both screens to confirm a pairing | An attacker who draws keys until a short digest matches defeats it; making it safe needs a commitment protocol we would design ourselves, which GENESIS.md forbids. A full fingerprint or a scanned code needs none. |
| Splitting the recovery code among other people (social recovery) | More for people to manage and to explain; it can come later on top of a recovery key. |

## Agreement with GENESIS.md and the decided RFCs

| GENESIS.md says | This RFC |
| --- | --- |
| "The relay never reads content." | No key reaches the relay; the account space is sealed content; pairing passes beside it. |
| "Never invent cryptography." | Ed25519, HKDF-SHA256 and HPKE as RFC 0003 uses them; BIP-39 only as a word list. No short-code protocol. |
| "Never give an agent more than its spaces." | An agent never holds the account key and joins no space automatically. |
| "A developer doesn't need to care about keys." | The app handles invitations, answers and a recovery code it shows once; never key bytes. |
| "Never claim more than the design delivers." | [What this does not do](#what-this-does-not-do) lists what recovery, removal and linking do not reach. |
| "Never change how permissions or spaces behave without telling them plainly." | Every automatic add returns a notice; the status says when losing a device loses a space. |

**Conflicts.** None with GENESIS.md. This draft changes no decided RFC. Some answers would, and the change then follows the owner's decision:

- [Q1](#open-questions), the recovery-key model: RFC 0003's member certificate gains the account key statement, its key table names the recovery key, and its sentence that the person keeps "a recovery copy" of the account key changes; RFC 0008 checks the chain and compares accounts by the recovery key; a control entry that carries a statement into a space is new in RFC 0008. The FAQ's answer that the recovery code "restores your account key" changes with it.
- [Q2](#open-questions): the recovery key's derivation joins RFC 0003's formats.
- [Q10](#open-questions) and [Q11](#open-questions): the new names join RFC 0009's "Public names".

## Open questions

GENESIS.md reserves crypto, key handling, formats and public names for the owner. Each question names its options and one recommendation.

**Q1. The account's identity.** See [The account's identity](#the-accounts-identity).

- A. The account key is the identity, as today; the recovery code is a copy of it. Cost: a lost device signs in the account's name for good; the answer is a new account.
- B. A recovery key, derived from the recovery code and kept by no device, signs an account key statement; a lost device is answered by a new account key with the next number. *(recommended)* Cost: changes to RFC 0003's certificate and RFC 0008's check, and the person needs the code to answer a lost device.

Recommendation: B. Phones get lost, and an account key that can never be replaced turns every lost phone into a permanent signer of certificates and erasure sign-offs in the person's name. The change to the formats costs least before the first release, while no certificate outside development exists.

**Q2. The form of the recovery code.**

- A. Twelve words from the BIP-39 English list: 128 random bits and a 4-bit checksum; the key derives with HKDF-SHA256 under `"nocentraldb/recovery-key"`. *(recommended)*
- B. Twenty-four BIP-39 words that encode the 32-byte Ed25519 seed itself.
- C. 26 characters in Crockford's base32: the same 128 bits and a checksum.

Recommendation: A. 128 bits match Ed25519's own strength; twelve words are copied by hand with fewer mistakes than 24 words or 26 characters, and the word list is used widely enough that people recognise it. Only the list and its checksum are used, not BIP-39's PBKDF2 step.

**Q3. What the recovery code restores.**

- A. The account only: a new device under the account, which other members add to their spaces; a space whose every member is lost stays lost, and the status warns before that happens. *(recommended)*
- B. Also a recovery member: a member key derived from the code, added as a reader to every space the account administers, so recovery can read those spaces again. Cost: a member nobody runs in every member list, and a leaked code reads everything instead of only signing.
- C. A backup of the account's space keys at the relay, sealed under a key derived from the code. Cost: a new store at the relay, and the same exposure as B.

Recommendation: A. It needs no new member and no new store, and a leaked code then cannot read anything by itself. An app that must not lose a space adds a second member to it: another person, or an agent.

**Q4. An account space for the account's records.**

- A. An ordinary space, created by the account's first device, whose members are the account's devices, each an admin; it holds the certified devices and agents, the spaces of each device, the links and the statements, never a key. *(recommended)*
- B. No shared record: each device knows only what it did itself.

Recommendation: A. Without it, `removeMember` cannot find the spaces of a lost device it is not in, and a new device cannot learn which spaces to ask for. It needs no new format. Cost: the relay sees one more space per account and when devices join it.

**Q5. How a pairing travels.**

- A. Two codes, the new device's invitation and the answer, shown as QR codes or as text; nothing goes through the relay. *(recommended)*
- B. One code, and a mailbox at the relay for the rest. Cost: a new request in RFC 0007, and the relay learns when pairings happen.
- C. A short code typed on the new device, with a password-authenticated key exchange through a relay mailbox. Cost: the mailbox, and a PAKE crate whose audit we would have to establish first.

Recommendation: A. It uses only RFC 0009's invitation and RFC 0003's sealing, works in island mode, and the relay learns nothing. Cost: two scans, or two copy-pastes for an agent.

**Q6. How the person confirms a pairing.**

- A. A device is confirmed only by a scanned code, or by a full fingerprint compared on both screens; an agent through any channel the person trusts. *(recommended)*
- B. Any channel, for devices and agents alike, with the fingerprint shown.

Recommendation: A. A device receives the account key, so a swapped invitation would hand the account to an attacker; scanning gives proof of presence. An agent receives only a certificate, which a person can remove again.

**Q7. Which spaces a new device joins.**

- A. Every space where a device of the account is an admin, added by the SDK with the role the account's devices hold there, with a notice each; the others listed for the app to ask; agents none. *(recommended)*
- B. None automatically: the app adds the device space by space.
- C. Every space of the account, with other admins' SDKs adding a certified device of a known account on their own.

Recommendation: A. A person expects their new phone to show their data. C lets a certificate admit a member without any admin deciding, which RFC 0008 never allows.

**Q8. One account key per app, or per device.**

- A. Per database: each app's database keeps its own copy of the account key and pairs on its own. *(recommended)*
- B. Per device where the platform lets apps share a keystore entry, per database elsewhere.

Recommendation: A. Phones and browsers keep apps apart, so B would make accounts behave differently by platform, which principle 7 rules out. Cost: a person pairs each app once.

**Q9. Memberships written under a per-database account, after linking.**

- A. They stay as written; the person's SDK shows one account through the link statement, and invitations carry it so others can show "linked to". *(recommended)*
- B. Re-added: where the account administers a space, the SDK removes and re-adds the member with the new certificate, one rotation per space; elsewhere as A.
- C. A new membership action that replaces a member's certificate. Cost: a change to RFC 0008's control bodies.

Recommendation: A. It costs no rotation and no format, and the limit it leaves, two accounts in the fold for one person, is no weaker than what anybody can do by drawing a second account.

**Q10. The public names of pairing, linking and recovery.**

- A. `nc.open({ pairing: "request" })` for a database that will be paired, `db.invitation()` for its request, `db.account.pair({ kind, invitation })` for the answer, `db.account.completePairing(answer)` on the new device, the same calls on a database that has its own account to link it, and `nc.open({ recover: words })` to recover. *(recommended)*
- B. As A, with a separate `db.account.link(answer)` for linking.

Recommendation: A. Linking is pairing a database that already has data, so one flow and one set of names covers both; fewer names to learn.

**Q11. How the recovery code reaches the person.**

- A. The SDK hands the app the code once, when the account is created, as an object whose string, JSON and debug forms are redacted; the app shows the words and calls `confirm()`, and the device then forgets the code. *(recommended)*
- B. The SDK ships its own screen for it.
- C. The SDK stores it in the platform's password manager.

Recommendation: A. Apps own their screens, and the redacted forms keep the code out of logs ([GENESIS.md, "What it must never do"](../../factory/GENESIS.md#what-it-must-never-do)). B would give the SDK a user interface on every platform; C puts the code on the devices it is meant to outlive.

**Q12. The fingerprint.**

- A. SHA-256 over `"nocentraldb/account-fingerprint"` and the identity's public key, shown as 60 digits in twelve blocks of five. *(recommended)*
- B. The first 16 bytes of the same hash in hexadecimal.
- C. Words from the recovery code's word list.

Recommendation: A. Digits read aloud the same in every language, and messaging apps have taught people to compare them. Words from the recovery list would invite people to confuse the two.

## Decisions

None yet. The owner's answers to the open questions go here, in short form, as [How a decided RFC changes](README.md#how-a-decided-rfc-changes) describes.

## History

- **2026-11-10** · NOC-107 · Draft 0.1: the account key and its identity, the recovery code, pairing a device or an agent, linking per-database accounts, losing a device, and what a certificate means for RFC 0008, with open questions Q1 to Q12.

## How this RFC is tested

`scripts/rfc-0010.test.mjs` checks the shape of this draft: that the RFC index lists it, that it says it is a draft and decides nothing, that it links GENESIS.md and the RFCs it builds on and every link resolves, that it has a section for every part NOC-107 names, that every open question has options, one recommended option and a recommendation and is cited where it arises, and that "Decisions" records none yet. Whether the design is right is the owner's review.
