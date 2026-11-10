// Holds RFC 0002 as a fresh text (NOC-165). The owner wants every RFC to read
// as if it had just been written (Carsten, 2026-11-10): each choice stands in
// the section it belongs to, with a sentence on why and the options not taken,
// and there is no list of decisions, no reference such as "D13" or "M7", and
// no History of the drafts. The tests below pin the content of every former
// decision where it now stands; the rules that came in later are held by
// rfc-0002-rules.test.mjs. Whether the wording is right is the review of the
// PR.
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join, normalize } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (path) => readFileSync(join(root, path), "utf8");

const RFC = "docs/rfc/0002-change-and-log-format.md";
const DISCUSSION = "https://github.com/nocentraldb/nocentraldb/discussions/125";
const PUBLISHED =
  "2026-11-10 · First published (RFC 0001 to RFC 0010 rewritten as fresh texts, NOC-165)";

/** GitHub's heading anchor: lower case, punctuation dropped, spaces to dashes. */
const slug = (heading) =>
  heading
    .trim()
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s_-]/gu, "")
    .replace(/\s/g, "-");

/** The text under the heading whose anchor is `anchor`, up to the next heading of the same or a higher level. */
const section = (anchor, markdown = read(RFC)) => {
  const lines = markdown.split("\n");
  const start = lines.findIndex(
    (line) =>
      /^#{1,6} /.test(line) && slug(line.replace(/^#+ /, "")) === anchor,
  );
  assert.ok(start >= 0, `RFC 0002 has no section #${anchor}`);
  const level = lines[start].match(/^#+/)[0].length;
  const rest = lines.slice(start + 1);
  const end = rest.findIndex((line) => {
    const m = line.match(/^(#+) /);
    return m && m[1].length <= level;
  });
  return (end < 0 ? rest : rest.slice(0, end)).join("\n");
};

/** The anchors of every heading in `markdown`. */
const anchors = (markdown) =>
  markdown
    .split("\n")
    .filter((line) => /^#{1,6} /.test(line))
    .map((line) => slug(line.replace(/^#+ /, "")));

/** The Markdown link targets in `text`. */
const targets = (text) =>
  [...text.matchAll(/\]\(([^)\s]+)\)/g)].map((m) => m[1]);
const links = (text, target) => targets(text).some((t) => t.endsWith(target));

/** The RFC without its History and its byte-exact examples, whose names (M1, N2) look like decision ids. */
const prose = () =>
  read(RFC)
    .replace(section("byte-exact-examples"), "")
    .replace(section("history"), "");

/** Every Markdown file a reader can follow a link from. */
const documents = () => [
  "README.md",
  "CONTRIBUTING.md",
  ...readdirSync(join(root, "docs"), { recursive: true })
    .filter((file) => file.endsWith(".md"))
    .map((file) => normalize(join("docs", file))),
];

test("RFC 0002 has no amendment section, mark or changelog", () => {
  const rfc = read(RFC);
  assert.doesNotMatch(rfc, /^## Amendment/m, "an amendment section is left");
  assert.doesNotMatch(rfc, /\(Amendment \d+/, "an amendment mark is left");
  assert.doesNotMatch(rfc, /\*\*Changelog\.\*\*/, "the changelog is left");
  assert.doesNotMatch(rfc, /amendment/i, "the text still speaks of amendments");
});

test("RFC 0002 has no list of decisions and no section of drafting bookkeeping", () => {
  const found = anchors(read(RFC));
  for (const gone of [
    "decisions",
    "decisions-this-rfc-follows",
    "conflicts-with-genesismd-and-rfc-0001",
  ]) {
    assert.ok(!found.includes(gone), `the section #${gone} is left`);
  }
  assert.ok(
    !found.some((anchor) => /^d\d+-/.test(anchor)),
    "a decision heading such as D1 is left",
  );
});

test("RFC 0002 names no decision by its id, its own or another RFC's", () => {
  const text = prose();
  for (const pattern of [
    /\bD\d+\b/,
    /\bM\d+\b/,
    /\bdecisions? [A-Z]?\d/i,
    /\bconflict \d/i,
    /Conflicts\]/,
  ]) {
    const hit = text.match(pattern);
    assert.ok(!hit, `RFC 0002 still names ${hit?.[0]}`);
  }
});

test("no document links a removed anchor of RFC 0002", () => {
  const removed =
    /0002-change-and-log-format\.md#(amendment-|decisions|d\d+-|the-nesting-limit|a-full-clock-counter|the-writers-model-in-payload-key-64|conflicts-with-genesismd-and-rfc-0001)/;
  const found = documents().filter((path) => removed.test(read(path)));
  assert.deepEqual(found, []);
});

test("the status says in one sentence that the owner decided it, and when", () => {
  const status = read(RFC)
    .split("\n")
    .find((line) => line.startsWith("**Status.**"));
  assert.equal(
    status,
    "**Status.** Decided by the owner (Carsten, 2026-11-10).",
  );
});

test("the feedback line links the RFC's discussion", () => {
  const line = read(RFC)
    .split("\n")
    .find((l) => l.startsWith("**How to give feedback.**"));
  assert.ok(line, "no feedback line");
  assert.ok(links(line, DISCUSSION), "the discussion #125 is not linked");
  assert.ok(links(line, "README.md#giving-feedback"));
  assert.doesNotMatch(line, /no discussion of its own/);
});

test("History has one line: the first publication", () => {
  const lines = section("history")
    .split("\n")
    .filter((l) => l.trim());
  assert.deepEqual(lines, [`- ${PUBLISHED}`]);
});

// Each former decision stands in its section: what was chosen, why, and the
// options not taken. The comment names the decision as the old text did, so
// the PR's mapping table can be checked against this file.

test("ordering metadata: a hybrid logical clock and dependencies, and why", () => {
  // D1
  const text = section("ordering-metadata");
  assert.match(text, /hybrid logical clock/);
  assert.match(text, /dependencies/);
  assert.match(text, /expect/, "no word on what users expect");
  assert.match(text, /Lamport/);
  assert.match(text, /[Vv]ersion vectors/);
});

test("change ids: the hash of the signed header bytes, and the ids not taken", () => {
  // D2
  const text = section("change-ids");
  assert.match(text, /without a coordinator|no coordinator/);
  assert.match(text, /backup/);
  assert.match(text, /author sequence/);
  assert.match(text, /random id/);
});

test("object ids are random, space ids derived, and why", () => {
  // D3, and RFC 0008's derived space ids
  const objects = section("object-ids");
  assert.match(objects, /128 bits/);
  assert.match(objects, /UUIDv7/);
  assert.match(objects, /links, URLs and exports/);
  const space = section("space");
  assert.match(space, /derived from its creator's member id/);
  assert.match(space, /does not bind the space to its creator/);
  assert.ok(links(space, "0008-membership.md#what-it-names"));
});

test("the encoding is deterministic CBOR, and why", () => {
  // D4
  const text = section("the-cbor-profile");
  assert.match(text, /self-describing/);
  assert.match(text, /Rust and TypeScript/);
  assert.match(text, /postcard/);
  assert.match(text, /hand-written binary format/i);
});

test("an operation names its field by name, and numeric ids are not taken", () => {
  // D5
  const text = section("operations-object-field-and-value");
  assert.match(text, /id registry/);
  assert.match(text, /[Nn]umeric ids/);
  assert.match(text, /ncdb model build/);
});

test("the clock sits inside the payload, and not in the header", () => {
  // D6
  const text = section("what-the-change-does-not-carry");
  assert.match(text, /clock sits inside the payload/);
  assert.match(text, /island mode/);
  assert.match(text, /far ahead/, "the option not taken is not named");
});

test("the log is a table in the SQLite database of the local view", () => {
  // D7
  const text = section("on-a-member");
  assert.match(text, /one SQLite transaction/);
  assert.match(text, /segment files/);
});

test("every payload is padded to a multiple of 256 bytes, and why", () => {
  // D8
  const text = section("padding");
  assert.match(text, /multiple of 256 bytes/);
  assert.match(text, /128 bytes per change on average/);
  assert.match(text, /"yes" against "no"/);
  assert.match(text, /[Nn]o padding/);
  assert.match(text, /power of two/);
  assert.match(text, /rich-text pastes/);
  assert.ok(
    links(section("the-writers-model"), "#padding"),
    "the writer's model does not link Padding",
  );
});

test("a change a client does not understand is parked, and principle 2 is read for it", () => {
  // D9, and the reading of principle 2 the owner accepted with it
  const text = section(
    "versioning-old-clients-keep-what-they-do-not-understand",
  );
  assert.match(text, /[Rr]efus\w+ to sync/);
  assert.match(text, /principle 2/);
  assert.match(text, /every member that understands all the changes it holds/);
});

test("the atomic unit says why an unknown operation parks on its own", () => {
  // D10
  const text = section("the-atomic-unit");
  assert.match(text, /fall further behind/);
  assert.match(text, /parks the whole change/);
  assert.ok(
    links(text, "#versioning-old-clients-keep-what-they-do-not-understand"),
  );
});

test("rejected changes say why they have a state of their own, and the options not taken", () => {
  // D11
  const text = section("rejected-changes");
  assert.match(text, /one trigger/);
  assert.match(text, /"not yet" from "never"/);
  assert.match(text, /dropped and reported/);
});

test("a change the relay refuses says why it stays folded, and the options not taken", () => {
  // D12, and RFC 0008's pushing again
  const text = section("a-change-the-relay-refuses-after-the-member-folded-it");
  assert.match(text, /no other member sees/);
  assert.match(text, /drop any of its own changes/);
  assert.match(text, /never ends for a removed author/);
  assert.ok(links(text, "0008-membership.md#how-the-relay-applies-the-rules"));
});

test("a payload that does not decode says why it is parked, and the options not taken", () => {
  // D13
  const text = section("a-payload-that-decrypts-but-does-not-decode");
  assert.match(text, /without freezing the payload profile/);
  assert.match(text, /"undecodable payload"/);
  assert.match(text, /[Ss]plit by cause/);
  assert.ok(
    links(text, "#versioning-old-clients-keep-what-they-do-not-understand"),
  );
});

test("the writer's model links RFC 0005 by its sections", () => {
  // The writer's model in payload key 64
  const text = section("the-writers-model");
  assert.match(text, /One map, not two payload keys/);
  assert.ok(
    links(text, "0005-schema-and-model-lock.md#data-from-a-newer-model"),
  );
});

test("the nesting limit says why 32 levels, and the limits not taken", () => {
  // The nesting limit
  const text = section("the-cbor-profile");
  assert.match(text, /overflow/);
  assert.match(text, /128 levels/);
  assert.match(text, /iterative decoders/);
});

test("a full clock counter carries, and the waits not taken", () => {
  // A full clock counter
  const text = section("the-clock");
  assert.match(text, /far-future clock/);
  assert.match(text, /[Rr]efus\w+ to write until the wall clock catches up/);
  assert.match(text, /capped wait/);
});

test("the terms say that envelope means only a key envelope", () => {
  // The third reading of the former Conflicts
  const terms = section("terms");
  const row = terms.split("\n").find((l) => l.startsWith("| Header |"));
  assert.ok(row, "no Header term");
  assert.match(row, /key envelope/);
  assert.match(row, /opaque envelope/);
});
