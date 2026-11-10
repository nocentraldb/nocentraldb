// Asserts RFC 0007 as one complete text (NOC-101), as docs/rfc/README.md's
// "How a decided RFC changes" asks. RFC 0007 never had amendment sections, so
// what is checked is the rest of the rule: no amendment section or mark;
// "Open questions" says whether one is left; "Decisions" keeps R1 to R13 in
// short form, with the owner's name and date, why, and the options not taken;
// "History" names this rewrite's pull request; the status says what is built;
// and every link other files use into RFC 0007 still resolves.
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join, normalize, relative } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const RFC = "docs/rfc/0007-relay.md";

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

/** The decisions under "## Decisions", one "### Mn. Title" heading each. */
const decisions = () =>
  [
    ...section(read(), "decisions").matchAll(
      /^### (R\d+)\. [^\n]+\n([\s\S]*?)(?=^### |(?![\s\S]))/gm,
    ),
  ].map((m) => ({ id: m[1], text: m[2] }));
const decision = (id) => {
  const found = decisions().find((d) => d.id === id);
  assert.ok(found, `${RFC} has no decision ${id}`);
  return found.text;
};

test("RFC 0007 has no amendment section, mark, in-page link or changelog", () => {
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

test("the status says what is built, and names the owner's decisions with their dates", () => {
  const status = line(read(), "**Status.**");
  assert.match(status, /^\*\*Status\.\*\* Decided\. /);
  for (const date of ["2026-11-08", "2026-11-10"]) {
    assert.ok(status.includes(`(Carsten, ${date})`), `no decision of ${date}`);
  }
  assert.doesNotMatch(status, /Nothing here is built/);
  assert.match(status, /crates\/ncdb-relay/);
});

test("History names this rewrite's pull request", () => {
  const entries = section(read(), "history")
    .split("\n")
    .filter((l) => l.startsWith("- "));
  const entry = entries.find((l) => l.includes(" · NOC-101 · "));
  assert.ok(entry, "History has no line for NOC-101");
  assert.match(entry, /\(#\d+\)/);
});

test("Open questions says whether the owner still has one", () => {
  assert.match(section(read(), "open-questions"), /\S/);
});

test("Decisions keeps R1 to R13 in short form: decided, why, and the options not taken", () => {
  const found = decisions();
  assert.deepEqual(
    found.map((d) => d.id),
    Array.from({ length: 13 }, (_, i) => `R${i + 1}`),
  );
  for (const { id, text } of found) {
    const date = ["R12", "R13"].includes(id) ? "2026-11-10" : "2026-11-08";
    assert.match(text, new RegExp(`^Decided \\(Carsten, ${date}\\)`, "m"), id);
    assert.match(text, /^Why: /m, `${id} gives no reason`);
    assert.match(
      text,
      /^Not taken:\n\n?- /m,
      `${id} lists no option not taken`,
    );
    assert.doesNotMatch(text, /\*\(recommended/, id);
  }
  assert.match(decision("R7"), /M3/);
  assert.match(decision("R12"), /NOC-98/);
  assert.match(decision("R13"), /NOC-143/);
});

test("every in-page link of RFC 0007 resolves to a heading", () => {
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

test("every link other files use into RFC 0007 names one of its headings", () => {
  const anchors = new Set(headings(read()).map(slug));
  for (const path of files()) {
    if (normalize(path) === normalize(RFC)) continue;
    for (const [, anchor] of read(path).matchAll(
      /0007-relay\.md#([a-z0-9-]+)/g,
    )) {
      assert.ok(anchors.has(anchor), `${path} links to missing #${anchor}`);
    }
  }
});
