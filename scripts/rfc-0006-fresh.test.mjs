// Holds RFC 0006 as a fresh text (NOC-165). The owner wants every RFC to read
// as if it had just been written (Carsten, 2026-11-10): each choice stands in
// the section it belongs to, with a sentence on why and the options not taken,
// and there is no list of decisions, rejected options or open questions, no
// reference such as "decision 4", "D11" or "S8", and no History of the drafts.
// The tests below pin the content of every former decision 1 to 17 and every
// former rejected option where it now stands, and the owner's answer on
// NOC-173 that was written in with this rewrite: in Node.js the TypeScript SDK
// runs the core as a native addon. Whether the wording is right is the review
// of the PR.
import assert from "node:assert/strict";
import { readdirSync } from "node:fs";
import { join, normalize } from "node:path";
import { test } from "node:test";
import { headings, read, root, section, slug } from "./lib/rfc-text.mjs";

const RFC = "docs/rfc/0006-local-view.md";
const DISCUSSION = "https://github.com/nocentraldb/nocentraldb/discussions/146";
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

test("RFC 0006 has no list of decisions, rejected options or open questions", () => {
  const found = headings(read(RFC)).map(slug);
  for (const gone of ["decisions", "rejected-options", "open-questions"]) {
    assert.ok(!found.includes(gone), `the section #${gone} is left`);
  }
  assert.ok(
    !found.some((anchor) => /^\d+-/.test(anchor)),
    "a numbered decision heading is left",
  );
});

test("RFC 0006 names no decision by its id, its own or another RFC's", () => {
  const text = read(RFC).replace(part("history"), "");
  for (const pattern of [
    /\bdecisions? \d/i,
    /\bD\d+\b/,
    /\bS\d+\b/,
    /\bM\d+\b/,
    /\*\*Conflicts\.\*\*/,
    /\bfor the owner\b/,
  ]) {
    const hit = text.match(pattern);
    assert.ok(!hit, `RFC 0006 still names "${hit?.[0]}"`);
  }
});

test("no document links a removed anchor of RFC 0006", () => {
  const removed =
    /0006-local-view\.md#(\d+-|decisions|rejected-options|open-questions)/;
  const found = documents().filter((path) => removed.test(read(path)));
  assert.deepEqual(found, []);
});

test("the status, the discussion and History", () => {
  assert.equal(
    line(read(RFC), "**Status.**"),
    "**Status.** Decided by the owner (Carsten, 2026-11-10).",
  );
  const feedback = line(read(RFC), "**How to give feedback.**");
  assert.ok(links(feedback, DISCUSSION), "the discussion #146 is not linked");
  assert.ok(links(feedback, "README.md#giving-feedback"));
  const lines = part("history")
    .split("\n")
    .filter((l) => l.trim());
  assert.deepEqual(lines, [`- ${PUBLISHED}`]);
});

// Each former decision stands in its section. The comment names the decision
// as the old text numbered it, so the PR's mapping table can be checked
// against this file.

test("one database per member, an internal layout, and the lock in meta", () => {
  // 1, 2, 17; rejected: one database per space, a stable layout
  holds(
    part("where-the-view-lives"),
    [
      /[Oo]ne database per space was not taken/,
      /freeze the implementation/,
      /versioned and migrated like the log/,
      /[Aa] table of its own for the lock/,
      /[Rr]aising the view format version/,
    ],
    "Where the view lives",
  );
});

test("the SQLite stack, and the stacks not taken", () => {
  // 3; rejected: the official build, wa-sqlite, a VFS on IndexedDB, the opfs
  // VFS, the main thread, the system's SQLite
  holds(
    part("sqlite-on-each-target"),
    [
      /two young crates/,
      /official SQLite WebAssembly build/,
      /wa-sqlite/,
      /VFS on IndexedDB/,
      /`opfs` VFS/,
      /main thread/,
      /system's SQLite/,
    ],
    "SQLite on each target",
  );
});

test("Node.js runs the core as a native addon, with the wasm core as fallback", () => {
  // NOC-173
  holds(
    part("sqlite-on-each-target"),
    [/Node\.js/, /napi-rs/, /prebuilt binary/],
    "SQLite on each target",
  );
});

test("several tabs, a commit's durability, and deleted content", () => {
  // 4, 7, 9
  holds(
    part("the-browser"),
    [/refusing all but one would surprise/, /show a notice/],
    "The browser",
  );
  holds(
    part("desktop-and-mobile"),
    [/`synchronous = NORMAL`/],
    "Desktop and mobile",
  );
  holds(
    part("what-the-view-stores-in-plaintext"),
    [/SQLite's defaults, where free pages keep old content/],
    "What the view stores in plaintext",
  );
});

test("the order of text and of pages, and the forms not taken", () => {
  // 5, 16; rejected: locale-aware collation, OFFSET, a row-value comparison,
  // raw SQL
  holds(
    part("the-order-of-values"),
    [/[Ll]ocale-aware collation/, /ICU/],
    "The order of values",
  );
  holds(
    part("declared-indexes"),
    [
      /[Rr]ow values with ascending indexes only/,
      /two code paths/,
      /[Rr]aw SQL/,
    ],
    "Declared indexes",
  );
  holds(part("ordered-pages"), [/`OFFSET`/], "Ordered pages");
});

test("encryption at rest relies on the device, and the options not taken", () => {
  // 6; rejected: read values encrypted in the view
  holds(
    part("what-the-view-stores-in-plaintext"),
    [
      /SQLCipher/,
      /protects little more than disk encryption/,
      /[Dd]eciding later/,
      /[Ss]toring read values encrypted/,
    ],
    "What the view stores in plaintext",
  );
});

test("the durability of a browser copy: asking, reporting, falling back and telling", () => {
  // 8, 10 to 15
  holds(
    part("durability-of-a-browser-copy"),
    [
      /[Ll]eaving it to the app/,
      /`db\.storage\.persistent`/,
      /`db\.isPersistent`/,
      /hanging in Firefox/,
      /forgets the call/,
      /VFS on IndexedDB for the length of the private session/,
      /API reference only/,
      /`nc\.text\.durability\.unsaved/,
      /production included/,
      /[Nn]o warning/,
    ],
    "Durability of a browser copy",
  );
});

test("the agreement states the browser's limit plainly, without a Conflicts heading", () => {
  holds(
    part("agreement-with-genesismd-and-rfc-0001"),
    [/complete working set/, /cache the browser may drop/],
    "Agreement with GENESIS.md and RFC 0001",
  );
});
