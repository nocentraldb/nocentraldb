// Asserts RFC 0009 as one complete text (NOC-101): no "Amendment" section,
// mark or in-page link is left, and every link other files use into RFC 0009
// still resolves. The rules the amendments made are checked where they now
// stand: groups leave the design (NOC-76), an nc.int and an object id in
// TypeScript (NOC-80), and references (NOC-92). Its form as a fresh text, with
// each decision in its section, is held by rfc-0009-fresh.test.mjs (NOC-165).
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join, normalize, relative } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const RFC = "docs/rfc/0009-sdk-api.md";

const read = (path = RFC) => readFileSync(join(root, path), "utf8");

/** GitHub's heading slug: lower case, punctuation dropped, spaces to dashes. */
const slug = (heading) =>
  heading
    .trim()
    .toLowerCase()
    .replace(/[^\p{L}\p{N} _-]/gu, "")
    .replace(/ /g, "-");

/** The headings of a Markdown text, outside code blocks. */
const headings = (markdown) =>
  markdown
    .replace(/```[\s\S]*?```/g, "")
    .split("\n")
    .filter((l) => /^#{1,6} /.test(l))
    .map((l) => l.replace(/^#+ /, ""));

/** The text under the heading whose anchor is `anchor`, up to the next heading of its level or higher. */
const section = (markdown, anchor) => {
  const lines = markdown.split("\n");
  const start = lines.findIndex(
    (l) => /^#{1,6} /.test(l) && slug(l.replace(/^#+ /, "")) === anchor,
  );
  assert.ok(start >= 0, `${RFC} has no heading #${anchor}`);
  const level = lines[start].match(/^#+/)[0].length;
  const rest = lines.slice(start + 1);
  const end = rest.findIndex((l) => {
    const m = l.match(/^(#+) /);
    return m && m[1].length <= level;
  });
  return (end < 0 ? rest : rest.slice(0, end)).join("\n");
};

/** The first line of `markdown` that starts with `start`. */
const line = (markdown, start) =>
  markdown.split("\n").find((l) => l.startsWith(start)) ?? "";

test("RFC 0009 has no amendment section, mark, in-page link or changelog left", () => {
  const rfc = read();
  for (const heading of headings(rfc)) {
    assert.doesNotMatch(heading, /amendment/i, `heading "${heading}"`);
  }
  assert.doesNotMatch(rfc, /\*\(?Amendment \d+\.?\)?\*/, "a mark is left");
  assert.doesNotMatch(
    rfc,
    /\]\(#amendment-/,
    "an in-page link to an amendment",
  );
  assert.doesNotMatch(rfc, /^\*\*Changelog\.\*\*/m, "the changelog is left");
  assert.doesNotMatch(line(rfc, "**Status.**"), /Amendment/);
});

test("groups, nc.int, the object id and references stand in the design (NOC-76, NOC-80, NOC-92)", () => {
  const rfc = read();
  assert.match(
    line(rfc, "- **`members.add(invitation, { role })`**"),
    /list of invitations/,
  );
  assert.match(rfc, /safe integer/);
  assert.match(line(rfc, "| `$raw` |"), /2\^53/);
  assert.match(section(rfc, "get-by-id"), /32 lowercase hex digits/);
  assert.match(section(rfc, "get-by-id"), /`\{ space, id \}`/);
  assert.match(line(rfc, "| `$conflicts` |"), /never for its state/);
  assert.match(line(rfc, "- **References:**"), /"notAccessible"/);
  assert.match(
    line(rfc, "- **A subscription does not follow a reference's target.**"),
    /target's space/,
  );
});

test("every in-page link of RFC 0009 resolves to a heading", () => {
  const rfc = read();
  const anchors = new Set(headings(rfc).map(slug));
  for (const [, anchor] of rfc.matchAll(/\]\(#([^)\s]+)\)/g)) {
    assert.ok(anchors.has(anchor), `${RFC} links to missing #${anchor}`);
  }
});

/** Every Markdown and test file of the repository outside dependencies and builds. */
const files = (dir = root) =>
  readdirSync(dir, { withFileTypes: true }).flatMap((e) => {
    if (["node_modules", "target", ".git", "dist"].includes(e.name)) return [];
    const path = join(dir, e.name);
    if (e.isDirectory()) return files(path);
    return /\.(md|mjs)$/.test(e.name) ? [relative(root, path)] : [];
  });

test("every link other files use into RFC 0009 names one of its headings", () => {
  const anchors = new Set(headings(read()).map(slug));
  for (const path of files()) {
    if (normalize(path) === normalize(RFC)) continue;
    for (const [, anchor] of read(path).matchAll(
      /0009-sdk-api\.md#([a-z0-9-]+)/g,
    )) {
      assert.ok(anchors.has(anchor), `${path} links to missing #${anchor}`);
    }
  }
});
