// Holds the owner's answer on NOC-222 point 1 (Carsten, 2026-11-11, A) as
// RFC 0003 and RFC 0009 record it (NOC-206): a member holds a space's
// person keys in memory only, and fetches the person-key records from the
// relay again on every connection, so no person key is written to the
// device. Whether the wording is right is the review of the PR.
import assert from "node:assert/strict";
import { test } from "node:test";
import { read, section } from "./lib/rfc-text.mjs";

const RFC_0003 = "docs/rfc/0003-keys-and-cryptography.md";
const RFC_0009 = "docs/rfc/0009-sdk-api.md";

test("RFC 0003 says a member holds person keys in memory, fetched on every connection", () => {
  const person = section(read(RFC_0003), "the-person-key");
  assert.doesNotMatch(person, /by each member in its local keystore/);
  assert.match(person, /in memory only/);
  assert.match(person, /every connection/);
  assert.match(person, /NOC-222/);
  assert.match(person, /Carsten, 2026-11-11/);
});

test("RFC 0009's personal fields say where the SDK holds the person keys", () => {
  const fields = section(read(RFC_0009), "personal-fields");
  assert.match(fields, /in memory only/);
  assert.match(fields, /every connection/);
  assert.match(fields, /\(0003-keys-and-cryptography\.md#the-person-key\)/);
});
