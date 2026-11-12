// Asserts that RFC 0007 says how a backend keeps the one space of an
// account the relay does not list (NOC-286, built in NOC-283): a backend
// rule, its row in the table of how each backend keeps the rules, and the
// item in DynamoDB's `spaces` table under the 32-byte account key. Each
// passage is marked as proposed until the owner's tick on the PR. Whether
// the wording is right is the owner's review of the PR.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const RFC = "docs/rfc/0007-relay.md";
const MARK = "(proposed in NOC-283; the owner decides with the PR's tick)";
const lines = () => readFileSync(join(root, RFC), "utf8").split("\n");
const line = (start) => {
  const found = lines().find((l) => l.startsWith(start));
  assert.ok(found, `${RFC} has no line starting "${start}"`);
  return found;
};

test("a backend rule keeps one space per account the relay does not list", () => {
  const rule = line("- **One space for an account the relay does not list.**");
  assert.match(rule, /effect of its genesis append/);
  assert.match(rule, /nothing of the append is applied/);
  assert.ok(rule.includes(MARK));
});

test("each backend says how it keeps the rule", () => {
  const row = line("| One space per unlisted account |");
  assert.match(row, /`guest_spaces`/);
  assert.match(row, /`attribute_not_exists`/);
  assert.ok(row.includes(MARK));
});

test("DynamoDB keeps it as an item of spaces under the 32-byte account key", () => {
  const row = line("| `spaces` |");
  assert.match(row, /32-byte account key/);
  assert.match(row, /`guest_space`/);
  assert.ok(row.includes(MARK));
});
