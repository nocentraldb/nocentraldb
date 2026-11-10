// Asserts RFC 0006 as one complete text (NOC-101), as docs/rfc/README.md's
// "How a decided RFC changes" asks: every amendment is folded into the design
// where its rule belongs, so no "Amendment" section, mark or in-page link is
// left; "Decisions" keeps each decision in short form, with the owner's name
// and date, why, and the options not taken; "History" lists dated lines that
// name their ticket and pull request; and every link other files use into
// RFC 0006 still resolves. The rules the amendments made are checked where
// they now stand: RFC 0008's control state (NOC-44), the durability of a
// browser copy and its findings (NOC-34, NOC-35, NOC-58), the cursor and the
// plan check (NOC-72), the scope of an index and deleted objects (NOC-75),
// and a reference's column (NOC-92).
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join, normalize, relative } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const RFC = "docs/rfc/0006-local-view.md";

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

/** The decisions under "## Decisions", one "### N. Title" heading each. */
const decisions = () =>
  [
    ...section(read(), "decisions").matchAll(
      /^### (\d+)\. [^\n]+\n([\s\S]*?)(?=^### |(?![\s\S]))/gm,
    ),
  ].map((m) => ({ id: Number(m[1]), text: m[2] }));
const decision = (id) => {
  const found = decisions().find((d) => d.id === id);
  assert.ok(found, `${RFC} has no decision ${id}`);
  return found.text;
};

test("RFC 0006 has no amendment section, mark, in-page link or changelog left", () => {
  const rfc = read();
  for (const heading of headings(rfc)) {
    assert.doesNotMatch(heading, /amendment/i, `heading "${heading}"`);
  }
  assert.doesNotMatch(rfc, /\*\(?Amendment \d+\.?\)?\*/, "a mark is left");
  assert.doesNotMatch(rfc, /\(Amendment \d+\)/, "a mark is left");
  assert.doesNotMatch(
    rfc,
    /\]\(#amendment-/,
    "an in-page link to an amendment",
  );
  assert.doesNotMatch(rfc, /Amendment step/, "a step named by an amendment");
  assert.doesNotMatch(rfc, /^\*\*Changelog\.\*\*/m, "the changelog is left");
  assert.doesNotMatch(line(rfc, "**Status.**"), /Amendment/);
});

test("the status names the owner's decisions with their dates", () => {
  const status = line(read(), "**Status.**");
  assert.match(status, /^\*\*Status\.\*\* Decided\. /);
  for (const date of ["2026-11-08", "2026-11-09"]) {
    assert.ok(status.includes(`(Carsten, ${date})`), `no decision of ${date}`);
  }
  assert.doesNotMatch(status, /not built yet/);
});

test("History lists dated lines, each with its ticket and pull request", () => {
  const entries = section(read(), "history")
    .split("\n")
    .filter((l) => l.startsWith("- "));
  for (const [ticket, pr] of [
    ["NOC-12", 36],
    ["NOC-44", 61],
    ["NOC-34", 73],
    ["NOC-35", 76],
    ["NOC-58", 85],
    ["NOC-72", 85],
    ["NOC-75", 87],
    ["NOC-92", 98],
    ["NOC-101", null],
  ]) {
    const entry = entries.find((l) => l.includes(` · ${ticket} · `));
    assert.ok(entry, `History has no line for ${ticket}`);
    assert.match(entry, /^- \*\*\d{4}-\d{2}-\d{2}\*\* · NOC-\d+ · /);
    if (pr) assert.ok(entry.includes(`(#${pr})`), `${ticket} names no #${pr}`);
    else assert.match(entry, /\(#\d+\)/, `${ticket} names no pull request`);
  }
});

test("Open questions says whether the owner still has one", () => {
  assert.match(section(read(), "open-questions"), /\S/);
});

test("Decisions keeps 1 to 16 in short form: decided, why, and the options not taken", () => {
  const found = decisions();
  assert.deepEqual(
    found.map((d) => d.id),
    Array.from({ length: 16 }, (_, i) => i + 1),
  );
  for (const { id, text } of found) {
    const date = id <= 9 ? "2026-11-08" : "2026-11-09";
    assert.match(
      text,
      new RegExp(`^Decided \\(Carsten, ${date}[^)]*\\): `, "m"),
      `decision ${id}`,
    );
    assert.match(text, /^Why: /m, `decision ${id} gives no reason`);
    assert.match(
      text,
      /^Not taken:\n\n?- /m,
      `decision ${id} lists no option not taken`,
    );
    assert.doesNotMatch(text, /\*\(recommended\)\*|^\s*Recommendation:/m);
  }
  assert.match(decision(12), /Confirmed \(Carsten, 2026-11-09\)/);
  assert.match(decision(16), /#81/);
});

test("the fold follows RFC 0008: control state, resolve, un-folding, a shred that waits (NOC-44)", () => {
  const rfc = read();
  const control = line(rfc, "**The control state.**");
  for (const item of [
    /genesis/i,
    /person-key entr/i,
    /cutoff/i,
    /generation graph/i,
  ]) {
    assert.match(control, item);
  }
  assert.match(control, /`resolve`/);
  assert.match(line(rfc, "| Shred entry"), /stable/);
  assert.match(line(rfc, "**Un-folding.**"), /`resolve`/);
  assert.match(line(rfc, "- **Control change.**"), /RFC 0008/);
});

test("Durability of a browser copy carries the four layers and the findings (NOC-34, NOC-35, NOC-58)", () => {
  const durability = section(read(), "durability-of-a-browser-copy");
  for (const layer of [
    /`db\.durability`/,
    /`requestPersistent\(\)`/,
    /persistentStorage: "on-request"/,
    /`storage: "temporary"`/,
    /\(\.\.\/browser-durability\.md\)/,
    /\{appName\}/,
    /console\.warn/,
    /warnings: false/,
  ]) {
    assert.match(durability, layer, `it lacks ${layer}`);
  }
  for (const fact of [
    /member key/,
    /seven days/,
    /survives a reload/,
    /is gone once the window closes|gone after the window closes/,
    /popup/,
    /Home Screen/,
  ]) {
    assert.match(durability, fact, `it lacks ${fact}`);
  }
  assert.match(durability, /\(\.\.\/browser-storage\.md#findings\)/);
});

test("a page after a cursor is written key by key, and the plan check accepts a SCAN on the index (NOC-72)", () => {
  const rfc = read();
  const indexes = section(rfc, "declared-indexes");
  assert.match(indexes, /key by key/);
  assert.match(
    indexes,
    /\| Descending \| `\(k < \? OR k IS NULL OR \(k = \? AND rest\)\)`/,
  );
  assert.match(line(rfc, "**The check.**"), /`SEARCH` or a `SCAN`/);
  assert.match(line(rfc, "**The check.**"), /row-value/);
  assert.match(decision(16), /row value/);
  assert.match(section(rfc, "rejected-options"), /row-value comparison/);
});

test("scope space, deleted objects and a reference's column stand in Declared indexes (NOC-75, NOC-92)", () => {
  const rfc = read();
  const indexes = section(rfc, "declared-indexes");
  assert.match(indexes, /`scope: "space"`/);
  assert.match(indexes, /\(space_id, status, priority, object_id\)/);
  assert.match(indexes, /No declared index holds a deleted object/);
  assert.match(indexes, /32 bytes/);
  assert.match(indexes, /`BLOB`/);
  assert.match(line(rfc, "- **A reference sorts by its 32 bytes**"), /memcmp/);
});

test("every in-page link of RFC 0006 resolves to a heading", () => {
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

test("every link other files use into RFC 0006 names one of its headings", () => {
  const anchors = new Set(headings(read()).map(slug));
  for (const path of files()) {
    if (normalize(path) === normalize(RFC)) continue;
    for (const [, anchor] of read(path).matchAll(
      /0006-local-view\.md#([a-z0-9-]+)/g,
    )) {
      assert.ok(anchors.has(anchor), `${path} links to missing #${anchor}`);
    }
  }
});
