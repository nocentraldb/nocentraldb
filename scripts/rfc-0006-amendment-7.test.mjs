// Asserts RFC 0006's Amendment 7 (NOC-92): the view follows RFC 0005's D19,
// which the owner decided as recommended (Carsten, 2026-11-10). A reference
// (`nc.ref`, field type `ncdb/ref`) indexes as one 32-byte BLOB column, the
// space id followed by the object id, compared by its bytes; an unset
// reference and a value that does not fit index as unset. The amendment
// decides nothing: it names D19, is announced in a dated changelog line, and
// marks the sentences it changes.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const RFC = "docs/rfc/0006-local-view.md";
const TITLE = "Amendment 7: following RFC 0005's references";
const ANCHOR = "amendment-7-following-rfc-0005s-references";
const D19 = "0005-schema-and-model-lock.md#decisions-for-the-owner-d18-to-d22";
const MARK = "*(Amendment 7.)*";

const rfc = readFileSync(join(root, RFC), "utf8");

/** The level-2 section that starts with `## <title>`. */
const section = (title) =>
  rfc.split(/^(?=## )/m).find((part) => part.startsWith(`## ${title}`));

/** The level-3 section that starts with `### <title>`, up to the next heading of level 2 or 3. */
const subsection = (title) => {
  const start = rfc.indexOf(`\n### ${title}\n`);
  assert.ok(start >= 0, `no section "${title}"`);
  const rest = rfc.slice(start + 1);
  const end = rest.search(/\n#{2,3} /);
  return end < 0 ? rest : rest.slice(0, end);
};

test("RFC 0006 announces Amendment 7 in a dated NOC-92 changelog line", () => {
  const top = rfc.split(/^## /m)[0];
  const line = top
    .split("\n")
    .find(
      (l) =>
        /^- \*\*\d{4}-\d{2}-\d{2}\*\* · NOC-92 · /.test(l) &&
        l.includes(`(#${ANCHOR})`),
    );
  assert.ok(line, `no dated NOC-92 changelog line links #${ANCHOR}`);
  assert.match(line, /decides nothing/);
});

test("Amendment 7 names D19, the owner's decision, and decides nothing", () => {
  const body = section(TITLE);
  assert.ok(body, `no section "${TITLE}"`);
  assert.match(body, /NOC-92/);
  assert.ok(body.includes(D19), "Amendment 7 does not link RFC 0005's D19");
  assert.match(body, /D19/);
  assert.match(body, /Carsten, 2026-11-10/);
  assert.match(body, /decides nothing/);
});

test("Declared indexes give a reference one 32-byte BLOB column, space id then object id", () => {
  const line = subsection("Declared indexes")
    .split("\n")
    .find((l) => l.includes(MARK) && l.includes("`nc.ref`"));
  assert.ok(line, "no marked sentence on references in Declared indexes");
  assert.match(line, /one column/);
  assert.match(line, /32 bytes/);
  assert.match(line, /`BLOB`/);
  assert.match(line, /space id followed by the object id/);
  assert.match(line, /does not fit/);
  assert.match(line, /unset/);
});

test("The order of values compares a reference by its bytes", () => {
  const line = subsection("The order of values")
    .split("\n")
    .find((l) => l.includes(MARK));
  assert.ok(line, "no marked bullet in The order of values");
  assert.match(line, /reference/i);
  assert.match(line, /bytes/);
  assert.match(line, /space id/);
});
