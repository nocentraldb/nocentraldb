// Holds the owner's answer on NOC-222 point 2 (Carsten, 2026-11-11, A) as
// RFC 0009 records it (NOC-206): the Rust SDK reports the accounts whose
// person-key records failed their checks in the space status, as
// `SpaceStatus::bad_person_key_records: Vec<Fingerprint>`. Whether the
// wording is right is the review of the PR.
import assert from "node:assert/strict";
import { test } from "node:test";
import { read } from "./lib/rfc-text.mjs";

const RFC_0009 = "docs/rfc/0009-sdk-api.md";

/** The bullet of RFC 0009 that starts with `start`, up to the next line. */
function bullet(start) {
  const text = read(RFC_0009);
  const at = text.indexOf(start);
  assert.notEqual(at, -1, `RFC 0009 has no bullet ${start}`);
  const end = text.indexOf("\n", at);
  return text.slice(at, end === -1 ? undefined : end);
}

test("the Rust space status names the accounts whose person-key records failed", () => {
  const text = bullet("- **The space status.**");
  assert.match(text, /`bad_person_key_records: Vec<Fingerprint>`/);
  assert.match(text, /\(0003-keys-and-cryptography\.md#the-person-key\)/);
  assert.match(text, /Carsten, 2026-11-11, NOC-222/);
});

test("RFC 0009's public names list the field with SpaceStatus", () => {
  const row = read(RFC_0009)
    .split("\n")
    .find((line) => line.startsWith("| `SpaceStatus` |"));
  assert.ok(row, "RFC 0009 has no public-names row for SpaceStatus");
  assert.match(row, /`bad_person_key_records`/);
});
