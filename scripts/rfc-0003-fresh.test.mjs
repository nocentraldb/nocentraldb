// Holds RFC 0003 as a fresh text (NOC-165). The owner wants every RFC to read
// as if it had just been written (Carsten, 2026-11-10): each choice stands in
// the section it belongs to, with a sentence on why and the options not taken,
// and there is no list of decisions, rejected options, decisions followed or
// open questions, no reference such as "decision 14", "D14", "M26" or "S11",
// and no History of the drafts. The tests below pin the content of every
// former decision 1 to 22, of the bytes of a sealed change, and of every
// former rejected option where it now stands, and the owner's answers written
// in with this rewrite: RFC 0010's recovery key, account key statement,
// pairing label and fingerprint (NOC-132), and the first person-key entry
// that chooses the key (NOC-133). Whether the wording is right is the review
// of the PR.
import assert from "node:assert/strict";
import { readdirSync } from "node:fs";
import { join, normalize } from "node:path";
import { test } from "node:test";
import { headings, read, root, section, slug } from "./lib/rfc-text.mjs";

const RFC = "docs/rfc/0003-keys-and-cryptography.md";
const DISCUSSION = "https://github.com/nocentraldb/nocentraldb/discussions/126";
const PUBLISHED =
  "2026-11-10 · First published (RFC 0001 to RFC 0010 rewritten as fresh texts, NOC-165)";

const part = (anchor) => section(read(RFC), anchor);

const links = (text, target) =>
  [...text.matchAll(/\]\(([^)\s]+)\)/g)].some((m) => m[1].endsWith(target));

const line = (text, start) => {
  const found = text.split("\n").find((l) => l.startsWith(start));
  assert.ok(found, `no line starts with "${start}"`);
  return found;
};

const holds = (text, patterns, where) => {
  for (const pattern of patterns) {
    assert.match(text, pattern, `${where} does not say ${pattern}`);
  }
};

const documents = () => [
  "README.md",
  "CONTRIBUTING.md",
  ...readdirSync(join(root, "docs"), { recursive: true })
    .filter((file) => file.endsWith(".md"))
    .map((file) => normalize(join("docs", file))),
];

test("RFC 0003 has no list of decisions, rejected options, decisions followed or open questions", () => {
  const found = headings(read(RFC)).map(slug);
  for (const gone of [
    "decisions",
    "decisions-this-rfc-follows",
    "the-bytes-of-a-sealed-change",
    "open-questions",
  ]) {
    assert.ok(!found.includes(gone), `the section #${gone} is left`);
  }
  assert.ok(
    !found.some((anchor) => /^\d+-/.test(anchor)),
    "a numbered decision heading is left",
  );
  assert.doesNotMatch(read(RFC), /^\*\*Rejected( options)?:\*\*/m);
});

test("RFC 0003 names no decision by its id, its own or another RFC's", () => {
  const text = read(RFC).replace(part("history"), "");
  for (const pattern of [
    /\bdecisions? \d/i,
    /\bD\d+\b/,
    /\bS\d+\b/,
    /\bM\d+\b/,
    /\bconflict \d/i,
    /\bpoint \d/i,
    /Accepted \(Carsten/,
    /\bdraft 0\.\d/,
  ]) {
    const hit = text.match(pattern);
    assert.ok(!hit, `RFC 0003 still names "${hit?.[0]}"`);
  }
});

test("no document links a removed anchor of RFC 0003", () => {
  const removed =
    /0003-keys-and-cryptography\.md#(\d+-|decisions|the-bytes-of-a-sealed-change|open-questions)/;
  const found = documents().filter((path) => removed.test(read(path)));
  assert.deepEqual(found, []);
});

test("the status, the discussion and History", () => {
  assert.equal(
    line(read(RFC), "**Status.**"),
    "**Status.** Decided by the owner (Carsten, 2026-11-10).",
  );
  const feedback = line(read(RFC), "**How to give feedback.**");
  assert.ok(links(feedback, DISCUSSION), "the discussion #126 is not linked");
  assert.ok(links(feedback, "README.md#giving-feedback"));
  const lines = part("history")
    .split("\n")
    .filter((l) => l.trim());
  assert.deepEqual(lines, [`- ${PUBLISHED}`]);
});

// Each former decision stands in its section. The comment names the decision
// as the old text numbered it, so the PR's mapping table can be checked
// against this file.

test("the member key: where it lives, the key record, and relay authentication", () => {
  // 13, 16, 19, 22
  holds(
    part("the-member-key"),
    [
      /third, separate key/,
      /app data directory/,
      /[Ii]n memory only/,
      /`keyStoreUnavailable`/,
      /nobody chose/,
      /`storageFailed`/,
      /113 bytes/,
      /canonical CBOR map/,
      /never wiped/,
      /raw 96 bytes/,
    ],
    "The member key",
  );
});

test("the account key: on every device, the recovery key, and the per-database account", () => {
  // 8, 17; NOC-132
  holds(
    part("the-account-key"),
    [
      /every device of the account/,
      /designated admin devices/,
      /derived from a passkey/,
      /recovery code/,
      /recovery key/,
      /each database draws its own account key once/i,
      /[Tt]he app passes the account key/,
      /blocks every space/,
    ],
    "The account key",
  );
  assert.ok(
    links(part("the-account-key"), "0010-accounts.md#the-recovery-code"),
  );
});

test("the key table names the recovery key, held by no device", () => {
  // NOC-132
  const row = line(part("the-keys"), "| Recovery key |");
  assert.match(row, /no device/);
  assert.match(row, /account key statement/);
  assert.doesNotMatch(part("the-keys"), /recovery copy/);
});

test("the person key: one per space, at the relay, and the store's integrity", () => {
  // 7, 10, 14
  holds(
    part("the-person-key"),
    [
      /[Oo]ne key per person across all spaces/,
      /while no other member is online/,
      /members hand them to each other directly/,
      /[Uu]nsigned records that any member may overwrite/,
      /only inside control changes in the log/,
    ],
    "The person key",
  );
});

test("the primitives, and the primitives not taken", () => {
  // 2, 3, 4, and the crates of the sealed change; rejected options of Primitives
  holds(
    part("primitives"),
    [
      /AES-256-GCM/,
      /ChaCha20-Poly1305 with a 96-bit nonce/,
      /paid audit of our combination before 1\.0/,
      /`hpke-rs`/,
      /ECDSA on P-256/,
      /one behaviour on every platform/,
      /Converting Ed25519 keys/,
      /libsodium's sealed boxes/,
      /platform's own crypto/,
      /BLAKE3 or SHA-3/,
      /`chacha20poly1305 0\.10\.1`/,
      /libsodium through bindings/,
    ],
    "Primitives",
  );
});

test("v1 is not post-quantum, and a hybrid suite id is reserved", () => {
  // 9
  holds(
    part("not-post-quantum-in-v1"),
    [/X25519/, /ML-KEM/, /reserve/, /records ciphertext today/],
    "Not post-quantum in v1",
  );
});

test("a new member reads the whole history", () => {
  // 5
  holds(
    part("sharing-a-space-key-with-a-new-member"),
    [/whole history/, /[Oo]nly from the generation it joins/],
    "Sharing a space key with a new member",
  );
});

test("the space key rotates eagerly", () => {
  // 6
  holds(
    part("removing-a-member-and-rotating-the-space-key"),
    [
      /\*\*eager rotation\*\*/,
      /on the next write/,
      /nobody writes for a while/,
    ],
    "Removing a member and rotating the space key",
  );
});

test("personal fields in an ad-hoc space", () => {
  // 11
  holds(
    part("sub-agents-get-an-ad-hoc-space"),
    [
      /not copied unless the parent asks/,
      /always shared into the ad-hoc space/,
      /least it needs/,
    ],
    "Sub-agents get an ad-hoc space",
  );
});

test("a shred: the sign-off, the wait, and the first person-key entry", () => {
  // RFC 0008's sign-off and wait; NOC-133
  const shredding = part("shredding-a-persons-key");
  holds(
    shredding,
    [
      /holds an admin member in the space/,
      /once the shred is stable/,
      /first accepted person-key entry in the order creates the person key/,
      /choose the key/,
      /rejected as "commitment"/,
      /a shred never protects against a member who copied the data/,
    ],
    "Shredding a person's key",
  );
  assert.ok(links(shredding, "0008-membership.md#roles-and-rights"));
  assert.ok(
    links(
      shredding,
      "0008-membership.md#effects-that-wait-until-a-change-is-stable",
    ),
  );
});

test("the sealed payload, and the bytes not taken", () => {
  // the bytes of a sealed change
  holds(
    part("sealed-payload"),
    [
      /canonical CBOR array in `info`/,
      /HKDF salt/,
      /header key of its own/,
      /derived from author and sequence/,
      /fixed list of fields/,
      /signature alone/,
      /without key 5/,
      /encryption layer/,
      /new suite/,
    ],
    "Sealed payload",
  );
});

test("the member certificate carries the statement, and a parent signs a sub-agent's", () => {
  // 15; NOC-132
  holds(
    part("member-certificate"),
    [
      /account key statement/,
      /chain of two/,
      /parent agent does not hold the account key/,
      /[Ee]very kind of member certificate signed by the account key/,
    ],
    "Member certificate",
  );
});

test("the formats of RFC 0010 join the formats: statement, recovery key, pairing answer, fingerprint", () => {
  // NOC-132
  holds(
    part("account-key-statement"),
    [/"nocentraldb\/account-key-statement"/, /3 number/],
    "Account key statement",
  );
  holds(
    part("recovery-key"),
    [/"nocentraldb\/recovery-key"/, /HKDF-SHA256/, /128 bits/],
    "Recovery key",
  );
  holds(
    part("pairing-answer"),
    [/"nocentraldb\/account-key"/, /HPKE/],
    "Pairing answer",
  );
  holds(
    part("account-fingerprint"),
    [
      /"nocentraldb\/account-fingerprint"/,
      /60 digits/,
      /[Uu]ntil RFC 0010 is built/,
      /own account key/,
    ],
    "Account fingerprint",
  );
});

test("the bytes of an encrypted previous key and its associated data", () => {
  // 18, 20
  holds(
    part("key-chain-subkey"),
    [
      /72 bytes/,
      /second layout to read/,
      /deterministic nonce/,
      /no label/,
      /"nocentraldb\/previous-key"/,
    ],
    "Key-chain subkey",
  );
});

test("the bytes signed for a person-key record", () => {
  // 21
  holds(
    part("person-key-record"),
    [
      /canonical CBOR of the record/,
      /record hash in place of the wrapped bytes/,
    ],
    "Person-key record",
  );
});

test("invitations and member ids, and a notice on every change of who reads what", () => {
  // 12
  holds(
    part("how-the-sdk-hides-keys"),
    [/[Pp]ublic keys passed to `members\.add`/, /return a notice/],
    "How the SDK hides keys",
  );
});

test("the per-member wrapped space key, and the protocols not taken", () => {
  // 1
  holds(
    part("group-key-distribution-options-compared"),
    [
      /per-member wrapped space key/,
      /\*\*MLS\*\*/,
      /\*\*Sender keys\*\*/,
      /key server/,
      /[Oo]ne key per object/,
    ],
    "Group key distribution: options compared",
  );
});

test("where the design makes GENESIS.md more precise", () => {
  // the six agreed points
  holds(
    part("agreement-with-genesismd-and-rfc-0001"),
    [/one key per person per space/i, /modified client/, /quantum computer/],
    "Agreement with GENESIS.md and RFC 0001",
  );
});

test("the relay takes a record from a current uploader, signed perhaps by a member who has since left", () => {
  // NOC-133 point 2
  const rule = line(
    part("the-person-key"),
    "- **Every record is signed and checked against the log.**",
  );
  assert.match(rule, /uploader is a current member/);
  assert.match(rule, /has since left/);
  assert.doesNotMatch(rule, /only from a current member/);
});
