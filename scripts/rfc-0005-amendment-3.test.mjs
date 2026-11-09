// Asserts RFC 0005's Amendment 3 (NOC-73), the follow-ups from building
// `ncdb model build` (NOC-70): the lock example is in the canonical form the
// RFC defines, byte for byte what the builder writes (the Rust tests hold the
// fixture to the builder's output); and the two points the build found
// open, how `nc.Infer` types an `nc.int` field and how a collection is
// deprecated, go to the owner as decisions with options and a
// recommendation, announced in a dated changelog line.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const RFC = "docs/rfc/0005-schema-and-model-lock.md";
const FIXTURE = "crates/ncdb-core/tests/model_lock/rfc-0005-v2.lock.json";
const AMENDMENT = "amendment-3-follow-ups-from-building-the-lock";

const read = (path = RFC) => readFileSync(join(root, path), "utf8");

const jsonBlocks = (markdown) =>
  [...markdown.matchAll(/^```json\n([\s\S]*?)^```$/gm)].map((m) => m[1]);

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

/** Every object's keys, at any depth, in the order the text writes them. */
const keyOrders = (value, path = "$") => {
  if (Array.isArray(value)) {
    return value.flatMap((v, i) => keyOrders(v, `${path}[${i}]`));
  }
  if (value === null || typeof value !== "object") return [];
  const keys = Object.keys(value);
  return [
    { path, keys },
    ...keys.flatMap((k) => keyOrders(value[k], `${path}.${k}`)),
  ];
};

/** RFC 8785 sorts keys by their UTF-16 code units, as `<` compares strings. */
const sorted = (keys) => [...keys].sort((a, b) => (a < b ? -1 : a > b ? 1 : 0));

test("RFC 0005's lock example writes every object's keys in canonical order", () => {
  const example = JSON.parse(jsonBlocks(read())[0]);
  for (const { path, keys } of keyOrders(example)) {
    assert.deepEqual(keys, sorted(keys), `${RFC}: keys of ${path} not sorted`);
  }
});

test("RFC 0005's lock example is the bytes ncdb model build writes", () => {
  assert.equal(jsonBlocks(read())[0], read(FIXTURE));
});

test("RFC 0005 announces Amendment 3 in a dated NOC-73 changelog line", () => {
  const top = read().split(/^## /m)[0];
  const line = top
    .split("\n")
    .find((l) => /^- \*\*\d{4}-\d{2}-\d{2}\*\* · NOC-73 · /.test(l));
  assert.ok(line, "no dated changelog line for NOC-73 above the first section");
  assert.ok(line.includes(`(#${AMENDMENT})`), `it does not link #${AMENDMENT}`);
});

test("Amendment 3 asks the owner about nc.Infer's int and deprecating a collection", () => {
  const body = sectionByAnchor(read(), AMENDMENT);
  assert.ok(body, `${RFC} has no section #${AMENDMENT}`);
  assert.match(body, /NOC-70/);
  const found = decisions(body);
  assert.deepEqual(
    found.map((d) => d.id),
    ["D16", "D17"],
    "Amendment 3 does not ask D16 and D17",
  );
  const [int, collection] = found;
  assert.match(int.text, /nc\.Infer/);
  assert.match(int.text, /`number`/);
  assert.match(int.text, /`bigint`/);
  assert.match(collection.text, /collection/);
  assert.match(collection.text, /\.deprecated\(/);
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
  }
});

test("RFC 0005's status names Amendment 3's open decisions", () => {
  const status = read()
    .split("\n")
    .find((l) => l.startsWith("**Status.**"));
  assert.ok(
    status.includes(`(#${AMENDMENT})`),
    "the status does not link Amendment 3",
  );
  assert.match(status, /D16/);
  assert.match(status, /D17/);
});
