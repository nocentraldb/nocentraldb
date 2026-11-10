// Holds RFC 0009 as a fresh text (NOC-165). The owner wants every RFC to read
// as if it had just been written (Carsten, 2026-11-10): each choice stands in
// the section it belongs to, with a sentence on why and the options not taken,
// and there is no list of decisions, rejected options, conflicts or open
// questions, no reference such as "S13", "D20" or "decision 10", and no
// History of the drafts. The tests below pin the content of every former
// decision S1 to S34 where it now stands, and the owner's later answers that
// were written in with this rewrite: NOC-154 and NOC-171 on members.add and
// its notice, NOC-146 on an unknown relay error, NOC-173 on Node.js, the
// newest lock of RFC 0005, and RFC 0010's names for pairing and recovery. It
// also holds RFC 0010's fingerprint digits (NOC-171) and its discussion link.
// Whether the wording is right is the review of the PR.
import assert from "node:assert/strict";
import { readdirSync } from "node:fs";
import { join, normalize } from "node:path";
import { test } from "node:test";
import { headings, read, root, section, slug } from "./lib/rfc-text.mjs";

const RFC = "docs/rfc/0009-sdk-api.md";
const RFC_0010 = "docs/rfc/0010-accounts.md";
const DISCUSSIONS = "https://github.com/nocentraldb/nocentraldb/discussions/";
const PUBLISHED =
  "2026-11-10 · First published (RFC 0001 to RFC 0010 rewritten as fresh texts, NOC-165)";

/** The text of RFC 0009's section `anchor`, which must exist. */
const part = (anchor, path = RFC) => section(read(path), anchor);

/** The Markdown link targets in `text`. */
const links = (text, target) =>
  [...text.matchAll(/\]\(([^)\s]+)\)/g)].some((m) => m[1].endsWith(target));

/** The first line of `text` that starts with `start`, which must exist. */
const line = (text, start) => {
  const found = text.split("\n").find((l) => l.startsWith(start));
  assert.ok(found, `no line starts with "${start}"`);
  return found;
};

/** Asserts that `text` matches every pattern in `patterns`. */
const holds = (text, patterns, where) => {
  for (const pattern of patterns) {
    assert.match(text, pattern, `${where} does not say ${pattern}`);
  }
};

/** Every Markdown file a reader can follow a link from. */
const documents = () => [
  "README.md",
  "CONTRIBUTING.md",
  ...readdirSync(join(root, "docs"), { recursive: true })
    .filter((file) => file.endsWith(".md"))
    .map((file) => normalize(join("docs", file))),
];

test("RFC 0009 has no list of decisions, rejected options, conflicts or open questions", () => {
  const found = headings(read(RFC)).map(slug);
  for (const gone of [
    "decisions",
    "rejected-options",
    "conflicts-with-genesismd-and-the-other-rfcs",
    "open-questions",
  ]) {
    assert.ok(!found.includes(gone), `the section #${gone} is left`);
  }
  assert.ok(
    !found.some((anchor) => /^s\d+-/.test(anchor)),
    "a decision heading such as S1 is left",
  );
});

test("RFC 0009 names no decision by its id, its own or another RFC's", () => {
  const text = read(RFC).replace(part("history"), "");
  for (const pattern of [
    /\bS\d+\b/,
    /\bD\d+\b/,
    /\bM\d+\b/,
    /\bdecisions? \d/i,
    /\bconflicts?,? points? \d/i,
    /NOC-1[15]4\b/,
  ]) {
    const hit = text.match(pattern);
    assert.ok(!hit, `RFC 0009 still names "${hit?.[0]}"`);
  }
});

test("no document links a removed anchor of RFC 0009", () => {
  const removed =
    /0009-sdk-api\.md#(s\d+-|decisions|rejected-options|conflicts-with-genesismd-and-the-other-rfcs|open-questions)/;
  const found = documents().filter((path) => removed.test(read(path)));
  assert.deepEqual(found, []);
});

test("the status says in one sentence that the owner decided it, and when", () => {
  assert.equal(
    line(read(RFC), "**Status.**"),
    "**Status.** Decided by the owner (Carsten, 2026-11-10).",
  );
});

test("the feedback lines of RFC 0009 and RFC 0010 link their discussions", () => {
  for (const [path, number] of [
    [RFC, 149],
    [RFC_0010, 154],
  ]) {
    const feedback = line(read(path), "**How to give feedback.**");
    assert.ok(
      links(feedback, `${DISCUSSIONS}${number}`),
      `${path}: #${number}`,
    );
    assert.ok(links(feedback, "README.md#giving-feedback"));
    assert.doesNotMatch(feedback, /no discussion of its own/);
  }
});

test("History has one line: the first publication", () => {
  const lines = part("history")
    .split("\n")
    .filter((l) => l.trim());
  assert.deepEqual(lines, [`- ${PUBLISHED}`]);
});

// Each former decision stands in its section. The comment names the decision
// as the old text numbered it, so the PR's mapping table can be checked
// against this file.

test("opening: the model and the lock, and where a database lives", () => {
  // S1, S26, S27, S28; rejected: building the lock in memory
  holds(
    part("open-and-close"),
    [
      /typed with a generic/,
      /generated file/,
      /builds the lock in memory/,
      /nobody chose/,
      /does not tell the developer what to do/,
      /loses its data silently/,
      /app data directory/,
    ],
    "Open and close",
  );
});

test("opening checks the newest lock, and Node.js runs the core natively", () => {
  // RFC 0005's newest lock; NOC-173
  const open = part("open-and-close");
  assert.match(
    line(open, "3. **Checks the lock on the device.**"),
    /newest lock/,
  );
  assert.doesNotMatch(open, /last opened with/);
  holds(
    open,
    [/native addon/, /napi-rs/, /prebuilt binary/, /wasm core/],
    "Open and close",
  );
});

test("objects live in a space, and a write is one change, with the forms not taken", () => {
  // S2, S3; rejected: an ORM, a write resolved by the relay, compare-and-set,
  // transactions across spaces, one handle per object type
  holds(
    part("the-api-in-one-page"),
    [/Everything on `db`/],
    "The API in one page",
  );
  holds(
    part("one-write-one-change"),
    [
      /async callback/,
      /receive apart/,
      /ORM/,
      /only when the relay accepts it/,
      /compare-and-set/,
      /[Tt]ransactions across spaces/,
      /`db\.tasks\.create`/,
    ],
    "One write, one change",
  );
});

test("field operations: sets are never written whole, and a reference is written as it reads", () => {
  // S4, S24
  holds(
    part("field-operations-per-field-type"),
    [/whole array/, /depend on a read of another space/],
    "Field operations per field type",
  );
});

test("a read: its shape and the ids, with the forms not taken", () => {
  // S5, S23, S25
  holds(
    part("get-by-id"),
    [
      /wrapped as `\{ value, conflicts \}`/,
      /typed `unknown`/,
      /one rule for both ids/,
      /base64url/,
      /`Uint8Array`/,
      /branded/,
    ],
    "Get by id",
  );
});

test("the public names of RFC 0004's reads, and the names not taken", () => {
  // S6
  holds(
    part("conflicts-lost-removals-and-link-states"),
    [/`losers`/, /`dangling`/],
    "Conflicts, lost removals and link states",
  );
});

test("queries: handles, one space, deleted objects, and the forms not taken", () => {
  // S7, S8, S9; rejected: a where clause, raw SQL
  holds(
    part("queries-and-indexes"),
    [
      /options object/,
      /filtering the cross-space page in memory/,
      /merging/,
      /skip deleted objects/,
      /flagged/,
      /`where`/,
      /[Rr]aw SQL/,
    ],
    "Queries and indexes",
  );
});

test("subscriptions pass the whole page, and a stream alone is not taken", () => {
  // S10
  holds(part("subscribe"), [/same loop/], "Subscribe");
});

test("spaces have no name in the SDK, and members join by one hop", () => {
  // S11, S12, S13, S14
  holds(part("spaces"), [/reserved core collection/], "Spaces");
  holds(
    part("invitations-and-members"),
    [/[Tt]wo hops/, /mailbox/, /`"viewer"`/, /no group object/],
    "Invitations and members",
  );
});

test("members.add returns a notice, and the cases it refuses", () => {
  // NOC-154 points 1, 2, 3, 5; NOC-171 point 2
  holds(
    part("invitations-and-members"),
    [
      /`invalidCertificate`/,
      /forged invitation/,
      /`alreadyMember`/,
      /former member/,
      /remembers each account whose fingerprint a person confirmed/,
      /`memberAdded`/,
      /`memberRemoved`/,
      /`spaceLeft`/,
      /`adHocSpaceCreated`/,
      /`erased`/,
      /named by the call/,
      /stays the same when its text is reworded/,
    ],
    "Invitations and members",
  );
  holds(
    part("the-rust-sdk"),
    [
      /`Members::add\(&Invitation, AddMember\) -> Result<Notice, Error>`/,
      /`Notice \{ code: NoticeCode, text/,
      /`NoticeCode::MemberAdded`/,
      /`Error::fingerprint\(\)/,
      /60 digits in twelve blocks of five/,
      /`Role::Writer` for an agent/,
    ],
    "The Rust SDK",
  );
});

test("accounts: RFC 0010's names, and the per-database account key", () => {
  // S15, S20, and RFC 0010's changes to RFC 0009
  holds(
    part("accounts-and-devices"),
    [
      /`nc\.open\(\{ model, lock, pairing: "request" \}\)`/,
      /`db\.account\.pair\(\{ kind, invitation \}\)`/,
      /`db\.account\.completePairing\(answer\)`/,
      /`nc\.open\(\{ model, lock, recover \}\)`/,
      /`confirm\(\)`/,
      /principle 5/,
      /blocks every space/,
    ],
    "Accounts and devices",
  );
  const names = part("public-names");
  for (const name of [
    "db.account.completePairing",
    "confirm",
    "invalidCertificate",
    "alreadyMember",
    "unknownError",
    "Notice",
    "NoticeCode",
  ]) {
    assert.ok(names.includes(`\`${name}`), `Public names has no ${name}`);
  }
});

test("sync: one relay per database, and an unknown relay code is unknownError", () => {
  // S16; NOC-146
  const sync = part("sync-and-status");
  holds(sync, [/relay per space/, /`unknownError`/], "Sync and status");
  assert.match(line(sync, "| `error` |"), /never reaches the app/);
});

test("errors: one class, the codes, and why", () => {
  // S17, S22; rejected: throwing on what syncing finds
  holds(
    part("errors"),
    [/class per error/, /Freeing space would not help/, /states of the copy/],
    "Errors",
  );
});

test("the Rust SDK: fields by name, where the database lives, and its names", () => {
  // S18, S19, S21, S29 to S34
  holds(
    part("the-rust-sdk"),
    [
      /generated from the lock/,
      /found from an app id/,
      /`\$` cannot start a Rust name/,
      /copied database then shares/,
      /names the SQLite file itself/,
      /none of its spaces can be read/,
      /names a key file, which does not help/,
      /`SpaceSummary`/,
      /builder/,
      /shadow Rust's `Sync` marker trait/,
    ],
    "The Rust SDK",
  );
});

test("RFC 0010's fingerprint says how its 60 digits are computed", () => {
  holds(
    part("the-fingerprint", RFC_0010),
    [
      /big-endian number/,
      /modulo 10\^60/,
      /leading zeros/,
      /twelve blocks of five/,
      /one space/,
    ],
    "RFC 0010, The fingerprint",
  );
});
