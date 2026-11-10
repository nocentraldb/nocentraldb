// Asserts RFC 0009's Amendment 4 (NOC-92): the SDK follows RFC 0005's D20
// and D21, which the owner decided as recommended (Carsten, 2026-11-10).
// `nc.Infer` types a reference as `{ space, id }`; `space.inspect` gives its
// state as `status`, `"ok"`, `"notAccessible"` or `"missing"`; `$conflicts`
// never names a field for its state; a subscription does not follow a
// reference's target. The amendment decides nothing: it names D20 and D21,
// is announced in a dated changelog line, and marks the sentences it changes.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const RFC = "docs/rfc/0009-sdk-api.md";
const TITLE = "Amendment 4: following RFC 0005's references";
const ANCHOR = "amendment-4-following-rfc-0005s-references";
const DECISIONS =
  "0005-schema-and-model-lock.md#decisions-for-the-owner-d18-to-d22";
const MARK = "*(Amendment 4.)*";
const MARK_CELL = "*(Amendment 4)*";

const rfc = readFileSync(join(root, RFC), "utf8");

/** The level-2 section that starts with `## <title>`. */
const section = (title) =>
  rfc.split(/^(?=## )/m).find((part) => part.startsWith(`## ${title}`));

/** The lines of a level-2 section that carry the amendment's mark. */
const marked = (title) =>
  section(title)
    .split("\n")
    .filter((l) => l.includes(MARK) || l.includes(MARK_CELL));

test("RFC 0009 announces Amendment 4 in a dated NOC-92 changelog line", () => {
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

test("Amendment 4 names D20 and D21, the owner's decision, and decides nothing", () => {
  const body = section(TITLE);
  assert.ok(body, `no section "${TITLE}"`);
  assert.match(body, /NOC-92/);
  assert.ok(body.includes(DECISIONS), "Amendment 4 does not link D18 to D22");
  assert.match(body, /D20/);
  assert.match(body, /D21/);
  assert.match(body, /Carsten, 2026-11-10/);
  assert.match(body, /decides nothing/);
});

test("Get by id types a reference as { space, id }", () => {
  const lines = marked("Read and query");
  assert.ok(
    lines.some((l) => l.includes("`{ space, id }`") && /reference/.test(l)),
    "no marked sentence types a reference as { space, id }",
  );
});

test("$conflicts never names a field for a reference's state", () => {
  const row = section("Read and query")
    .split("\n")
    .find((l) => l.startsWith("| `$conflicts` |"));
  assert.ok(row, "no $conflicts row");
  assert.ok(row.includes(MARK_CELL), "the $conflicts row is not marked");
  assert.match(row, /reference/);
  assert.match(row, /never/);
  assert.match(row, /state/);
});

test("space.inspect gives a reference's state as status", () => {
  const lines = marked("Read and query");
  const line = lines.find((l) => l.includes('"notAccessible"'));
  assert.ok(line, "no marked inspect bullet names notAccessible");
  assert.match(line, /`status`/);
  assert.match(line, /"ok"/);
  assert.match(line, /"missing"/);
});

test("A subscription does not follow a reference's target", () => {
  const lines = marked("Subscribe");
  assert.ok(
    lines.some((l) => /reference/.test(l) && /target/.test(l)),
    "no marked sentence on references in Subscribe",
  );
});

test("Public names list the reference states", () => {
  const row = section("Public names")
    .split("\n")
    .find((l) => l.includes('"notAccessible"'));
  assert.ok(row, "Public names do not list notAccessible");
  assert.ok(row.includes(DECISIONS), "the row does not link D20");
});
