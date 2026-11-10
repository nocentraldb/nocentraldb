// Asserts that RFC 0007 records the owner's answer in NOC-98 (Carsten,
// 2026-11-10), as NOC-106 writes it into the text: a protocol message holds
// its type as text at key 1, its request id at key 2 (absent when there is
// none), then each field at keys 3, 4, ... in the order the RFC lists it;
// a nested map numbers its fields from 1 in the RFC's order; message types
// and error codes are text. The full table stands in the protocol section;
// RFC 0007 is a fresh text (NOC-165), whose rfc-0007-fresh.test.mjs holds the
// options not taken.
import assert from "node:assert/strict";
import { test } from "node:test";
import {
  assertLinksResolve,
  headings,
  read,
  section,
} from "./lib/rfc-text.mjs";

const RFC = "docs/rfc/0007-relay.md";
const TABLE = "the-keys-of-each-message";

/** The rows of the first Markdown table in `text`, as arrays of trimmed cells, header and rule left out. */
const rows = (text) =>
  (text.split("\n\n").find((block) => block.startsWith("|")) ?? "")
    .split("\n")
    .slice(2)
    .map((l) =>
      l
        .slice(1, -1)
        .split("|")
        .map((c) => c.trim().replaceAll("`", "")),
    );

/** Every message, by type and direction, with the keys after key 1, as NOC-98's rule gives them. */
const MESSAGES = [
  ["hello", "member → relay", "3 versions, 4 client"],
  ["challenge", "relay → member", "3 version, 4 relay id, 5 nonce, 6 limits"],
  ["auth", "member → relay", "3 signing key, 4 agreement key, 5 signature"],
  ["welcome", "relay → member", "3 member id, 4 relay epoch, 5 spaces"],
  ["push", "member → relay", "2 req, 3 space, 4 frames"],
  ["pushed", "relay → member", "2 req, 3 results"],
  ["pull", "member → relay", "2 req, 3 space, 4 epoch, 5 after, 6 limit"],
  ["entries", "relay → member", "2 req, 3 space, 4 entries, 5 head"],
  ["subscribe", "member → relay", "2 req, 3 space, 4 epoch, 5 after"],
  ["unsubscribe", "member → relay", "2 req, 3 space"],
  ["added", "relay → member", "3 space"],
  ["keys", "member → relay", "2 req, 3 space"],
  ["keys", "relay → member", "2 req, 3 space, 4 envelopes, 5 chain"],
  ["person-keys", "member → relay", "2 req, 3 space"],
  ["person-keys", "relay → member", "2 req, 3 space, 4 records"],
  ["put-person-keys", "member → relay", "2 req, 3 space, 4 records"],
  ["ok", "relay → member", "2 req"],
  ["upload", "member → relay", "2 req, 3 space, 4 size"],
  ["upload", "relay → member", "2 req, 3 id, 4 url"],
  ["ping", "member → relay", "2 req"],
  ["pong", "relay → member", "2 req"],
  ["error", "relay → member", "2 req, 3 code, 4 retry after"],
];

/** Every nested map, with its keys from 1 in the RFC's order. */
const NESTED = [
  ["limits", "1 largest message, 2 changes per push, 3 largest frame"],
  ["upload reference", "1 upload id"],
  ["result", "1 accepted position, 2 duplicate position, 3 rejected code"],
  ["entry", "1 position, 2 accepted at, 3 frame or download"],
  ["download", "1 url, 2 size"],
  [
    "record",
    "1 person-key id, 2 generation, 3 writer member id, 4 wrapped, 5 signature",
  ],
];

test("RFC 0007 lists the keys of every message, by NOC-98's rule", () => {
  const table = rows(section(read(RFC), TABLE));
  for (const [type, direction, keys] of MESSAGES) {
    const row = table.find((r) => r[0] === type && r[1] === direction);
    assert.ok(row, `${RFC} has no row for ${type}, ${direction}`);
    assert.equal(row[2], keys, `the keys of ${type}, ${direction}`);
  }
  assert.equal(table.length, MESSAGES.length, "a row no message has");
});

test("RFC 0007 numbers the fields of every nested map from 1, in its order", () => {
  const text = section(read(RFC), TABLE);
  const tables = text.split(/\n(?=\| Map \|)/);
  const nested = rows(tables.find((t) => t.startsWith("| Map |")) ?? "");
  for (const [map, keys] of NESTED) {
    const row = nested.find((r) => r[0] === map);
    assert.ok(row, `${RFC} has no row for the nested map ${map}`);
    assert.equal(row[1], keys, `the keys of ${map}`);
  }
});

test("RFC 0007 states the rule: type at 1, req at 2 or absent, types and codes as text", () => {
  const text = section(read(RFC), TABLE);
  assert.match(text, /key 1/);
  assert.match(text, /key 2 is absent/i);
  assert.match(text, /3, 4, …/);
  assert.match(text, /in the order this RFC lists/i);
  assert.match(text, /number[s]? (its|their) fields from 1/i);
  assert.match(text, /types and error codes are text/i);
});

test("RFC 0007 has no amendment section or mark", () => {
  for (const heading of headings(read(RFC))) {
    assert.doesNotMatch(heading, /amendment/i, `heading "${heading}"`);
  }
  assert.doesNotMatch(read(RFC), /\*\(Amendment \d+\.?\)\*/);
});

test("every relative link in RFC 0007 resolves, anchors included", () => {
  assertLinksResolve(RFC);
});
