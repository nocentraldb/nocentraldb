// Holds the owner's answer on NOC-210 question 1 (Carsten, 2026-11-11, A)
// as RFC 0003 records it (NOC-175 slice 2): in the browser the key record
// is sealed with AES-GCM under a non-extractable WebCrypto key, and both
// sit in IndexedDB, in the database `nocentraldb/<name>`, object store
// `keys`, under the keys `wrapping-key` and `key-record`. Whether the
// wording is right is the review of the PR.
import assert from "node:assert/strict";
import { test } from "node:test";
import { read, section } from "./lib/rfc-text.mjs";

const RFC_0003 = "docs/rfc/0003-keys-and-cryptography.md";

test("RFC 0003's member key gives the browser's layout of the key record", () => {
  const member = section(read(RFC_0003), "the-member-key");
  for (const pattern of [
    /\*\*In the browser\.\*\*/,
    /`nocentraldb\/<name>`/,
    /object store `keys`/,
    /`wrapping-key`/,
    /`key-record`/,
    /non-extractable AES-GCM-256/,
    /12-byte random IV/,
    /`"nocentraldb\/browser-key-record"`/,
    /141 bytes/,
    /no passkey/i,
    /Carsten, 2026-11-11, NOC-210/,
  ]) {
    assert.match(member, pattern, `the member key section does not say ${pattern}`);
  }
});

test("RFC 0003 says an open that finds only one record, or one that does not open, fails", () => {
  const member = section(read(RFC_0003), "the-member-key");
  assert.match(member, /never draws new keys/);
  assert.match(member, /`storageFailed`/);
});

test("RFC 0003 states the JavaScript heap limit plainly", () => {
  const member = section(read(RFC_0003), "the-member-key");
  assert.match(member, /JavaScript engine/);
});

test("RFC 0003 no longer leaves browser storage out of scope", () => {
  const hides = section(read(RFC_0003), "how-the-sdk-hides-keys");
  assert.doesNotMatch(hides, /Details of passkeys and browser storage are out of scope/);
  assert.match(hides, /\(#the-member-key\)/);
});
