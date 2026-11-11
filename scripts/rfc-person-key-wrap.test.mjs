// Holds the owner's answer on NOC-209 (Carsten, 2026-11-11, A) as RFC 0003
// records it (NOC-206): the wrapped bytes of a person-key record are the
// person key under a subkey of the record's generation, with the label
// "nocentraldb/person-key-wrap", the space id, the generation id and the
// person-key id as associated data, in the 72-byte layout of everything a
// space key seals, and a test vector written with independent libraries.
// Whether the wording is right is the review of the PR.
import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { read, root, section } from "./lib/rfc-text.mjs";

const RFC = "docs/rfc/0003-keys-and-cryptography.md";
const VECTORS = "crates/ncdb-core/tests/vectors/membership";

test("RFC 0003 gives the person-key subkey", () => {
  const record = section(read(RFC), "person-key-record");
  assert.match(
    record,
    /person-key subkey = HKDF-SHA256\(ikm = the space key of the record's generation, salt = none,\s+info = "nocentraldb\/person-key-wrap" \|\| space id \(16 bytes\) \|\|\s+generation id \(8 bytes, big-endian\)\)/,
  );
});

test("RFC 0003 gives the associated data and the 72 wrapped bytes", () => {
  const record = section(read(RFC), "person-key-record");
  assert.match(
    record,
    /associated data\s+= space id \(16 bytes\) \|\| generation id \(8 bytes, big-endian\) \|\|\s+person-key id \(16 bytes\)\s+40 bytes/,
  );
  assert.match(
    record,
    /wrapped bytes\s+= nonce \(24 bytes\) \|\| ciphertext \(32 bytes\) \|\| tag \(16 bytes\)\s+72 bytes/,
  );
  assert.match(record, /XChaCha20-Poly1305, random nonce/);
  assert.match(record, /Carsten, 2026-11-11/);
});

test("RFC 0003 names the options not taken for the wrap", () => {
  const record = section(read(RFC), "person-key-record");
  assert.match(record, /"nocentraldb\/person-key"` was not taken/);
  assert.match(record, /person-key id alone as associated data was not taken/);
  assert.match(record, /content key/);
});

test("the person key's store links the record's format", () => {
  const person = section(read(RFC), "the-person-key");
  assert.match(person, /\(#person-key-record\)/);
});

test("the vector of a wrapped person key is committed beside its generator", () => {
  const generator = read(`${VECTORS}/generate.py`);
  assert.match(generator, /b"nocentraldb\/person-key-wrap"/);
  assert.match(generator, /write\("person-key-wrapped", PERSON_KEY_WRAPPED\)/);
  assert.ok(existsSync(join(root, VECTORS, "person-key-wrapped.hex")));
  const bytes = read(`${VECTORS}/person-key-wrapped.hex`).replace(/\s/g, "");
  assert.equal(bytes.length, 144);
});
