// Asserts that RFC 0003 and RFC 0009 record the on-disk formats the owner
// decided on NOC-144 (Carsten, 2026-11-10, option A for all three points),
// as #134 builds them in crates/nocentraldb: the 113-byte key record and the
// key file's mode 0600 in RFC 0003, "The member key"; the keystore entry,
// the one SQLite file in `path(dir)` and the `storageFailed` of an existing
// database whose keys are gone in RFC 0009, "Where the database lives" and
// "Errors". Both are fresh texts (NOC-165), with the reasons and the options
// not taken where the format stands.
import assert from "node:assert/strict";
import { test } from "node:test";
import { read, section } from "./lib/rfc-text.mjs";

const RFC_0003 = "docs/rfc/0003-keys-and-cryptography.md";
const RFC_0009 = "docs/rfc/0009-sdk-api.md";

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
  // NOC-165: the options not taken stand with the format.
  assert.match(key, /CBOR/);
  assert.match(key, /96 bytes/);
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
