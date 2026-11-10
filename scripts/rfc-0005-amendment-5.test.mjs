// Asserts RFC 0005's Amendment 5 (NOC-63): `nc.ref("collection")`, a
// reference into another space, which the owner decided in NOC-29 (Carsten,
// 2026-11-09), joins the schema language, the lock's type ids and "Public
// names", marked where it changes the RFC. Every point beyond NOC-29's
// decision (the schema form, the index, how a read and a view show the three
// states, subscriptions, modifiers) goes to the owner as a decision with
// options and a recommendation; the owner accepted every recommendation
// (Carsten, 2026-11-10). RFC 0004's Amendment 4 defines
// the type's state, merge and encoding.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const RFC = "docs/rfc/0005-schema-and-model-lock.md";
const AMENDMENT = "amendment-5-a-reference-into-another-space";
const RFC_0004_AMENDMENT =
  "0004-merge-rules-basic-types.md#amendment-4-a-reference-into-another-space";
const MARK = "*(Amendment 5)*";
const MARK_SENTENCE = "*(Amendment 5.)*";

const read = () => readFileSync(join(root, RFC), "utf8");

/** GitHub's heading slug: lower case, punctuation dropped, spaces to dashes. */
const slug = (heading) =>
  heading
    .trim()
    .toLowerCase()
    .replace(/[^\p{L}\p{N} _-]/gu, "")
    .replace(/ /g, "-");

/** The body of the level-2 section whose GitHub anchor is `anchor`. */
const sectionByAnchor = (markdown, anchor) =>
  markdown
    .split(/^(?=## )/m)
    .find((part) => slug(part.split("\n")[0].replace(/^## /, "")) === anchor);

/** Each "**Dn." decision of a text, up to the next one or a heading. */
const decisions = (text) =>
  [
    ...text.matchAll(/^\*\*(D\d+)\.[\s\S]*?(?=^\*\*D\d+\.|^#|(?![\s\S]))/gm),
  ].map((m) => ({ id: m[1], text: m[0] }));

/** The first line of `markdown` that starts with `start`. */
const line = (markdown, start) =>
  markdown.split("\n").find((l) => l.startsWith(start)) ?? "";

const amendment = () => {
  const body = sectionByAnchor(read(), AMENDMENT);
  assert.ok(body, `${RFC} has no section #${AMENDMENT}`);
  return body;
};

test("RFC 0005 announces Amendment 5 in a dated NOC-63 changelog line and its status", () => {
  const rfc = read();
  const top = rfc.split(/^## /m)[0];
  const entry = top
    .split("\n")
    .find((l) => /^- \*\*\d{4}-\d{2}-\d{2}\*\* · NOC-63 · /.test(l));
  assert.ok(entry, "no dated changelog line for NOC-63");
  assert.ok(
    entry.includes(`(#${AMENDMENT})`),
    `it does not link #${AMENDMENT}`,
  );
  assert.match(entry, /NOC-29/);
  const status = line(rfc, "**Status.**");
  assert.ok(status.includes(`(#${AMENDMENT})`), "the status omits Amendment 5");
  assert.match(status, /D18 to D22[^.]*\(Carsten, 2026-11-10\)/);
  assert.match(entry, /\(Carsten, 2026-11-10\)/);
  assert.doesNotMatch(top, /are open and wait for the owner/);
});

test("Amendment 5 records NOC-29's decision and links RFC 0004's Amendment 4", () => {
  const body = amendment();
  assert.match(body, /NOC-29/);
  assert.match(body, /\(Carsten, 2026-11-09\)/);
  assert.ok(
    body.includes(`(${RFC_0004_AMENDMENT})`),
    "it does not link RFC 0004's Amendment 4",
  );
});

test("the field types table and the type ids list nc.ref as ncdb/ref", () => {
  const rfc = read();
  const row = line(rfc, '| `nc.ref("collection")` |');
  assert.match(row, /`ncdb\/ref`/);
  assert.ok(row.includes(MARK), "the field type row is not marked");
  const builtIns = line(rfc, "- **Built-in types**");
  assert.match(builtIns, /`ncdb\/ref`/);
  const field = line(rfc, "| `targets` |");
  assert.match(field, /`ncdb\/ref`/);
});

test("the schema text says what nc.ref is, and that computed fields never follow it", () => {
  const rfc = read();
  const paragraph = line(rfc, "`nc.ref` takes");
  assert.match(paragraph, /another space/);
  assert.match(paragraph, /never follow/);
  assert.ok(paragraph.includes(MARK_SENTENCE), "it is not marked");
  assert.match(line(rfc, "- An index can use"), /`nc\.ref`/);
});

test("the additive-change rule treats a reference's targets as a link's", () => {
  const rfc = read();
  assert.match(
    line(rfc, "| Add a collection to a link's or a reference's `targets`"),
    /missing/,
  );
  assert.match(
    line(rfc, "| Remove a collection from a link's or a reference's `targets`"),
    /\|/,
  );
});

test("Public names lists nc.ref and the type id ncdb/ref", () => {
  const names = sectionByAnchor(read(), "public-names");
  assert.match(line(names, "| `nc.ref` |"), /another space/);
  assert.match(line(names, "| The type id namespace"), /`ncdb\/ref`/);
});

test("D18 to D22 each have options and one recommendation, and record the owner's decision", () => {
  const found = decisions(amendment());
  assert.deepEqual(
    found.map((d) => d.id),
    ["D18", "D19", "D20", "D21", "D22"],
  );
  for (const { id, text } of found) {
    const options = text.match(/^- [A-Z]\. /gm) ?? [];
    assert.ok(options.length >= 2, `${id} has fewer than two options`);
    assert.equal(
      (text.match(/\*\(recommended\)\*/g) ?? []).length,
      1,
      `${id} does not mark exactly one option recommended`,
    );
    assert.match(
      text,
      /^Recommendation: [A-Z]\b/m,
      `${id} has no recommendation`,
    );
    assert.ok(
      text.includes("Decided (Carsten, 2026-11-10): the recommendation, (A)."),
      `${id} does not record the owner's decision`,
    );
    assert.doesNotMatch(text, /Open: waits for the owner/, `${id} is open`);
  }
  const [schema, index, read_, subscribe, modifiers] = found.map((d) => d.text);
  assert.match(schema, /`targets`/);
  assert.match(index, /index/);
  assert.match(read_, /`"notAccessible"`/);
  assert.match(read_, /view/);
  assert.match(subscribe, /subscription/);
  assert.match(modifiers, /\.personal\(/);
});

test("every in-page link of RFC 0005 resolves to a heading", () => {
  const rfc = read();
  const anchors = new Set(
    rfc
      .split("\n")
      .filter((l) => /^#{1,6} /.test(l))
      .map((l) => slug(l.replace(/^#+ /, ""))),
  );
  for (const [, anchor] of rfc.matchAll(/\]\(#([^)\s]+)\)/g)) {
    assert.ok(anchors.has(anchor), `${RFC} links to missing #${anchor}`);
  }
});

test("Amendment 5 no longer calls any of its points open", () => {
  const rfc = read();
  for (const open of [
    /Everything beyond NOC-29's decision is open/,
    /before the owner decides/,
    /once the owner decides/,
    /as \[D19\]\([^)]*\) proposes/,
  ]) {
    assert.doesNotMatch(rfc, open, `RFC 0005 still says ${open}`);
  }
});
