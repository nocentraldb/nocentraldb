// Asserts that the account names and open formats proposed in NOC-262 read
// as the owner decided them on NOC-273 (Carsten, 2026-11-12): RFC 0009's
// Rust forms, notices and public names, and RFC 0010's pairing answer, the
// account space's model and the link statement each carry his mark, and no
// passage of either RFC still says the owner has not decided it.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const MARK = "(Carsten, 2026-11-12, NOC-273)";
const read = (rfc) => readFileSync(join(root, "docs/rfc", rfc), "utf8");

/** The line of `rfc` that starts with `start`. */
const line = (rfc, start) => {
  const found = read(rfc)
    .split("\n")
    .find((l) => l.startsWith(start));
  assert.ok(found, `${rfc} has no line starting "${start}"`);
  return found;
};

test("neither RFC still marks an account passage as undecided", () => {
  for (const rfc of ["0009-sdk-api.md", "0010-accounts.md"]) {
    const text = read(rfc);
    assert.doesNotMatch(text, /proposed, NOC-262/, rfc);
    assert.doesNotMatch(text, /the owner has not decided/, rfc);
  }
});

test("RFC 0009's account forms, notices and names carry the owner's mark", () => {
  const rfc = "0009-sdk-api.md";
  assert.ok(
    line(rfc, "**The Rust forms and what the calls return**").includes(MARK),
  );
  assert.ok(line(rfc, "**The notices of accounts**").includes(MARK));
  assert.ok(line(rfc, "| `OpenOptions::pairing`").includes(MARK));
});

test("RFC 0010's pairing answer, model and link statement carry the owner's mark", () => {
  const rfc = "0010-accounts.md";
  assert.ok(line(rfc, "The answer travels as text").includes(MARK));
  assert.ok(line(rfc, "**Its model** is one the SDK ships").includes(MARK));
  assert.ok(line(rfc, "Both keys sign the same input").includes(MARK));
});
