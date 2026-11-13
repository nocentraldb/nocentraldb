// Asserts that the three gaps NOC-267 raised and the owner decided on
// 2026-11-13 are recorded in the RFCs: the member fingerprint (NOC-288),
// `db.account().recovered()` (NOC-289), and the store the joining device
// keeps a link statement in until the account space holds it, with an
// agent's own account not linked in v1 (NOC-291).
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (rfc) => readFileSync(join(root, "docs/rfc", rfc), "utf8");

test("RFC 0010 gives the member fingerprint (NOC-288)", () => {
  const text = read("0010-accounts.md");
  for (const pattern of [
    /`"nocentraldb\/member-fingerprint"`/,
    /its member id, shown as 60 digits in twelve blocks of five/,
    /`Invitation::fingerprint\(\)` returns it for a version 2 invitation/,
    /\(Carsten, 2026-11-13, NOC-288\)/,
  ]) {
    assert.match(text, pattern, `RFC 0010 does not say ${pattern}`);
  }
});

test("RFC 0010 keeps the link statement beside the log and defers an agent's (NOC-291)", () => {
  const text = read("0010-accounts.md");
  for (const pattern of [
    /keeps it in a store beside its log, written in the same step that completes the pairing/,
    /forgets it only once the relay has confirmed the change that wrote it into the account space's `links`/,
    /not in the view, so that rebuilding the view never drops it/,
    /\*\*An agent that has an account of its own is not linked in v1\*\* \(Carsten, 2026-11-13, NOC-291\)/,
  ]) {
    assert.match(text, pattern, `RFC 0010 does not say ${pattern}`);
  }
});

test("RFC 0009 names db.account().recovered() (NOC-289)", () => {
  const text = read("0009-sdk-api.md");
  for (const pattern of [
    /\| `db\.account\(\)\.recovered\(\)` \|/,
    /`AccountRecovered` on the open that recovered the account, else `None`/,
    /\(Carsten, 2026-11-13, NOC-289\)/,
  ]) {
    assert.match(text, pattern, `RFC 0009 does not say ${pattern}`);
  }
  // The notices table row points at the method, not "the open".
  const row = text
    .split("\n")
    .find((l) => l.startsWith("| `AccountRecovered` |"));
  assert.ok(row?.includes("db.account().recovered()"), row);
});
