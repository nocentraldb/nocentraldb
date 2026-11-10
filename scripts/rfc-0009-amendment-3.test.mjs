// Asserts RFC 0009's Amendment 3 (NOC-80): the owner decided how an object
// id looks in TypeScript, a string of its 16 bytes as 32 lowercase hex
// digits, as the lock writes `model.id`. The amendment is announced in a
// dated changelog line, names the owner's decision, and marks the sentence
// in "Get by id" it changes.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const RFC = "docs/rfc/0009-sdk-api.md";
const AMENDMENT = "amendment-3-the-object-id-in-typescript";

const rfc = readFileSync(join(root, RFC), "utf8");
const MARK = "*(Amendment 3.)*";

/** The level-2 section that starts with `## <title>`. */
const section = (title) =>
  rfc.split(/^(?=## )/m).find((part) => part.startsWith(`## ${title}`));

test("RFC 0009 announces Amendment 3 in a dated NOC-80 changelog line", () => {
  const top = rfc.split(/^## /m)[0];
  const line = top
    .split("\n")
    .find(
      (l) =>
        /^- \*\*\d{4}-\d{2}-\d{2}\*\* · NOC-80 · /.test(l) &&
        l.includes(`(#${AMENDMENT})`),
    );
  assert.ok(line, `no dated NOC-80 changelog line links #${AMENDMENT}`);
});

test("Amendment 3 records the owner's decision on the object id", () => {
  const body = section("Amendment 3: the object id in TypeScript");
  assert.ok(body, "no section Amendment 3");
  assert.match(body, /NOC-80/);
  assert.match(body, /Carsten, 2026-11-10/);
  assert.match(body, /32 lowercase hex digits/);
  assert.match(body, /model\.id/);
});

test("Get by id says how a TypeScript object id looks", () => {
  const read = section("Read and query");
  const line = read
    .split("\n")
    .find((l) => l.includes(MARK) && l.includes("32 lowercase hex digits"));
  assert.ok(line, "no marked sentence on the object id in Read and query");
});
