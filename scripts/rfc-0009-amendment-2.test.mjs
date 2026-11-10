// Asserts RFC 0009's Amendment 2 (NOC-80), which follows RFC 0005's D16
// (Amendment 4): on the TypeScript SDK an `nc.int` reads and writes as a
// `number`, so a write must be a safe integer or is refused with
// `invalidValue`, and a stored int beyond that range reads `undefined` and
// is named in `$raw`. The amendment decides nothing; it is announced in a
// dated changelog line, and the sentences it changes are marked.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const RFC = "docs/rfc/0009-sdk-api.md";
const AMENDMENT = "amendment-2-following-rfc-0005-amendment-4";
const D16 =
  "0005-schema-and-model-lock.md#amendment-4-follow-ups-from-building-the-lock";

const rfc = readFileSync(join(root, RFC), "utf8");
const MARK = "*(Amendment 2.)*";

/** The level-2 section that starts with `## <title>`. */
const section = (title) =>
  rfc.split(/^(?=## )/m).find((part) => part.startsWith(`## ${title}`));

/** The lines of the text that carry the amendment's mark. */
const marked = (text) => text.split("\n").filter((l) => l.includes(MARK));

test("RFC 0009 announces Amendment 2 in a dated NOC-80 changelog line", () => {
  const top = rfc.split(/^## /m)[0];
  const line = top
    .split("\n")
    .find((l) => /^- \*\*\d{4}-\d{2}-\d{2}\*\* · NOC-80 · /.test(l));
  assert.ok(line, "no dated changelog line for NOC-80 above the first section");
  assert.ok(line.includes(`(#${AMENDMENT})`), `it does not link #${AMENDMENT}`);
});

test("Amendment 2 follows RFC 0005's D16 and decides nothing", () => {
  const body = section("Amendment 2: following RFC 0005 Amendment 4");
  assert.ok(body, "no section Amendment 2");
  assert.match(body, /NOC-80/);
  assert.ok(body.includes(`(${D16})`), "it does not link RFC 0005's D16");
  assert.match(body, /decides nothing/);
});

test("a write refuses an int outside the safe range with invalidValue", () => {
  const write = section("Write");
  const lines = marked(write);
  assert.ok(
    lines.some(
      (l) =>
        l.includes("invalidValue") &&
        /safe integer/.test(l) &&
        l.includes("2^53 − 1"),
    ),
    "no marked refusal of an unsafe integer with invalidValue in Write",
  );
});

test("$raw names a stored int beyond the safe range, and a set of ints holding one", () => {
  const read = section("Read and query");
  const row = marked(read).find((l) => l.startsWith("| `$raw` |"));
  assert.ok(row, "the $raw row carries no Amendment 2 mark");
  assert.match(row, /nc\.int/);
  assert.match(row, /2\^53 − 1/);
  assert.match(row, /set of ints/);
  assert.match(row, /bigint/);
});
