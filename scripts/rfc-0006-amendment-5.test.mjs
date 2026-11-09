// Asserts RFC 0006 Amendment 5 (NOC-72): "Declared indexes" follows the
// cursor and the plan check that #81 (NOC-69) built and the owner decided by
// merging it. The page after a cursor is written key by key, NULL-aware and
// per direction, with a bound on the first key; the check on open accepts a
// SEARCH or SCAN on the declared index and refuses a temporary B-tree or any
// other index. The RFC states the conditions exactly as
// crates/ncdb-core/src/view/query.rs builds them.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const RFC = "docs/rfc/0006-local-view.md";
const QUERY = "crates/ncdb-core/src/view/query.rs";
const DECISION =
  "2026-11-09-NOC-69-a-page-after-a-cursor-is-written-key-by-key.md";
const read = (path) => readFileSync(join(root, path), "utf8");

/** The text of the section headed `heading` (any level), up to the next heading of that level or higher. */
const section = (markdown, heading) => {
  const lines = markdown.split("\n");
  const start = lines.findIndex(
    (l) => l.replace(/^#+ /, "") === heading && /^#+ /.test(l),
  );
  assert.ok(start >= 0, `no section "${heading}"`);
  const level = lines[start].match(/^#+/)[0].length;
  const end = lines.findIndex(
    (l, i) => i > start && /^#+ /.test(l) && l.match(/^#+/)[0].length <= level,
  );
  return lines.slice(start + 1, end < 0 ? undefined : end).join("\n");
};

/** The four conditions of `after_key`, with `{name}` as `k` and `{rest}` as `rest`. */
const codeConditions = () => {
  const source = read(QUERY);
  const body = source.slice(source.indexOf("fn after_key("));
  const end = body.indexOf("\n}\n");
  const formats = [...body.slice(0, end).matchAll(/format!\("([^"]+)"\)/g)].map(
    (m) => m[1].replaceAll("{name}", "k").replaceAll("{rest}", "rest"),
  );
  assert.equal(
    formats.length,
    4,
    `${QUERY}: after_key has not four conditions`,
  );
  return formats;
};

test("RFC 0006 announces Amendment 5 and records the owner's decision", () => {
  const rfc = read(RFC);
  const top = rfc.split(/^## /m)[0];
  assert.match(
    top,
    /^- \*\*2026-11-09\*\* · NOC-72 · \[Amendment 5: [^\]]+\]\(#amendment-5-[^)]+\)/m,
    "no changelog line for Amendment 5",
  );
  const body = rfc
    .split(/^(?=## )/m)
    .find((s) => s.startsWith("## Amendment 5: "));
  assert.ok(body, "no section Amendment 5");
  assert.match(
    body,
    /\(Carsten, 2026-11-09\)/,
    "the owner's decision is not recorded",
  );
  assert.match(body, /#81/, "Amendment 5 does not name #81");
  assert.ok(
    body.includes(DECISION),
    "Amendment 5 does not link NOC-69's decision file",
  );
  assert.match(body, /NULL/, "Amendment 5 does not name the unset value case");
  assert.match(
    body,
    /descending/i,
    "Amendment 5 does not name the descending key case",
  );
  assert.match(
    rfc,
    /\*\(Amendment 5\.\)\*/,
    "no sentence marked *(Amendment 5.)*",
  );
});

test("Declared indexes writes the cursor key by key, as the code does", () => {
  const text = section(read(RFC), "Declared indexes");
  assert.ok(
    !/\(([a-z_]+, )+[a-z_]+\) > \(\?/.test(text),
    "Declared indexes still pages with a row-value comparison",
  );
  for (const condition of codeConditions()) {
    assert.ok(
      text.includes(condition),
      `Declared indexes does not state the condition \`${condition}\``,
    );
  }
  assert.match(text, /k >= \?/, "no bound on the first key");
  assert.match(text, /seek/, "the bound does not say it lets SQLite seek");
  assert.match(text, /\*\(Amendment 5\.\)\*/);
});

test("the check accepts SEARCH or SCAN on the declared index and refuses a temporary B-tree or another index", () => {
  const text = section(read(RFC), "Declared indexes");
  const check = text.split("\n").find((l) => l.startsWith("**The check.**"));
  assert.ok(check, "no paragraph The check");
  assert.ok(
    !/is not a `SEARCH \.\.\. USING \[COVERING\] INDEX`/.test(check),
    "the check still accepts only SEARCH",
  );
  assert.match(check, /`SEARCH`/);
  assert.match(check, /`SCAN`/);
  assert.match(check, /TEMP B-TREE/);
  assert.match(check, /another index/);
  assert.match(check, /\*\(Amendment 5\.\)\*/);
});

test("Ordered pages says the next page follows the cursor in index order", () => {
  const text = section(read(RFC), "Ordered pages");
  assert.ok(
    !/whose key is greater than the cursor's/.test(text),
    "Ordered pages still says the key is greater than the cursor's",
  );
  assert.match(text, /\*\(Amendment 5\.\)\*/);
});
