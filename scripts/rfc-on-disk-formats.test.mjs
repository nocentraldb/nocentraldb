// Asserts that RFC 0003 and RFC 0009 record the on-disk formats the owner
// decided on NOC-144 (Carsten, 2026-11-10, option A for all three points),
// as #134 builds them in crates/nocentraldb: the 113-byte key record and the
// key file's mode 0600 in RFC 0003, "The member key"; the keystore entry,
// the one SQLite file in `path(dir)` and the `storageFailed` of an existing
// database whose keys are gone in RFC 0009, "Where the database lives" and
// "Errors". RFC 0003 with a decision in short form and a dated History line
// naming NOC-144, NOC-149 and #134 (NOC-149); RFC 0009, a fresh text
// (NOC-165), with the reasons and the options not taken in its bullet.
import assert from "node:assert/strict";
import { test } from "node:test";
import { read, section, slug } from "./lib/rfc-text.mjs";

const RFC_0003 = "docs/rfc/0003-keys-and-cryptography.md";
const RFC_0009 = "docs/rfc/0009-sdk-api.md";

/** The decision of `path` whose text matches `pattern`. */
const decisionMatching = (path, pattern) => {
  const decisions = section(read(path), "decisions");
  const headings = decisions
    .split("\n")
    .filter((l) => l.startsWith("### "))
    .map((l) => l.slice(4));
  const found = headings
    .map((h) => ({ heading: h, text: section(decisions, slug(h)) }))
    .find(({ text }) => pattern.test(text));
  assert.ok(found, `${path} has no decision matching ${pattern}`);
  return found;
};

const historyLine = (path) =>
  section(read(path), "history")
    .split("\n")
    .find((l) => l.startsWith("- **2026-11-10** · ") && l.includes("NOC-149"));

const rustWhereItLives = () =>
  section(read(RFC_0009), "the-rust-sdk")
    .split("\n")
    .find((l) => l.startsWith("- **Where the database lives"));

// RFC 0003, "The member key"

test("RFC 0003's member key gives the 113 bytes of the key record", () => {
  const key = section(read(RFC_0003), "the-member-key");
  assert.match(key, /113 bytes/);
  assert.match(key, /`nocentraldb-keys`/);
  assert.match(key, /format byte `1`/);
  assert.match(
    key,
    /member signing secret, the member key-agreement secret and the account secret, 32 bytes each/,
  );
  assert.match(key, /the same in the keystore and in a key file/);
  assert.match(key, /mode 0600 on Unix/);
});

test("RFC 0003 records the key record as a decision of the owner on NOC-144", () => {
  const { text } = decisionMatching(RFC_0003, /`nocentraldb-keys`/);
  assert.match(text, /^Decided \(Carsten, 2026-11-10\): /m);
  assert.match(text, /NOC-144/);
  assert.match(text, /113 bytes/);
  assert.match(text, /^Not taken:\n\n- /m);
  assert.match(text, /CBOR/);
  assert.match(text, /96 bytes/);
});

test("RFC 0003's History names NOC-144, NOC-149 and #134", () => {
  const line = historyLine(RFC_0003);
  assert.ok(line, `${RFC_0003} has no History line for NOC-149`);
  assert.match(line, /NOC-144/);
  assert.match(line, /#134/);
});

// RFC 0009, "Where the database lives" and "Errors"

test("RFC 0009 names the keystore entry and the database file", () => {
  const where = rustWhereItLives();
  assert.ok(
    where,
    "RFC 0009's Rust SDK has no bullet on where the database lives",
  );
  assert.match(where, /service `nocentraldb`/);
  assert.match(where, /canonical absolute path of the database directory/);
  assert.match(where, /one SQLite file, `nocentraldb\.sqlite`/);
  // NOC-165: the reasons stand in the bullet, not in a decision S30 or S31.
  assert.match(where, /copied database then shares/);
  assert.match(where, /names the SQLite file itself/);
});

test("RFC 0009 says an existing database whose keys are gone fails with storageFailed", () => {
  const where = rustWhereItLives();
  assert.match(where, /`storageFailed`/);
  assert.match(where, /draws no new keys/);
  assert.match(where, /none of its spaces can be read/);
  const row = section(read(RFC_0009), "errors")
    .split("\n")
    .find((l) => l.startsWith("| `storageFailed` |"));
  assert.match(row, /key file or keystore entry is gone/);
  assert.match(row, /#the-rust-sdk/);
});
