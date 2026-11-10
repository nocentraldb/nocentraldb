// Asserts RFC 0009 as one complete text (NOC-101), as docs/rfc/README.md's
// "How a decided RFC changes" asks: every amendment is folded into the design
// where its rule belongs, so no "Amendment" section, mark or in-page link is
// left; "Decisions" keeps S1 to S29 in short form, with the owner's name and
// date, why, and the options not taken, S14 stays listed as withdrawn, and
// S25 is the object id the owner decided on #91 (S26 to S28 followed on
// NOC-114, S29 on NOC-116); "History" lists dated lines
// that name their ticket and pull request; the status says what is built; and
// every link other files use into RFC 0009 still resolves. The rules the
// amendments made are checked where they now stand: groups leave the design
// (NOC-76), an nc.int and an object id in TypeScript (NOC-80), and references
// (NOC-92).
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

/** The decisions under "## Decisions", one "### Mn. Title" heading each. */
const decisions = () =>
  [
    ...section(read(), "decisions").matchAll(
      /^### (S\d+)\. [^\n]+\n([\s\S]*?)(?=^### |(?![\s\S]))/gm,
    ),
  ].map((m) => ({ id: m[1], text: m[2] }));
const decision = (id) => {
  const found = decisions().find((d) => d.id === id);
  assert.ok(found, `${RFC} has no decision ${id}`);
  return found.text;
};

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

test("the status says what is built, and names the owner's decisions with their dates", () => {
  const status = line(read(), "**Status.**");
  assert.match(status, /^\*\*Status\.\*\* Decided\. /);
  for (const date of ["2026-11-09", "2026-11-10"]) {
    assert.ok(status.includes(`(Carsten, ${date})`), `no decision of ${date}`);
  }
  assert.doesNotMatch(status, /Nothing here is built/);
  assert.doesNotMatch(
    line(read(), "- **Code.**"),
    /Nothing in `crates\/` or `packages\/` changes/,
  );
  assert.match(status, /crates\/nocentraldb/);
});

test("History lists dated lines, each with its ticket and pull request", () => {
  const entries = section(read(), "history")
    .split("\n")
    .filter((l) => l.startsWith("- "));
  for (const [ticket, pr] of [
    ["NOC-64", 78],
    ["NOC-35", 76],
    ["NOC-76", 87],
    ["NOC-75", 87],
    ["NOC-80", 91],
    ["NOC-92", 98],
    ["NOC-106", 113],
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

test("Decisions keeps S1 to S29 in short form: decided, why, and the options not taken", () => {
  const found = decisions();
  assert.deepEqual(
    found.map((d) => d.id),
    Array.from({ length: 29 }, (_, i) => `S${i + 1}`),
  );
  for (const { id, text } of found) {
    const date = Number(id.slice(1)) <= 18 ? "2026-11-09" : "2026-11-10";
    assert.match(text, new RegExp(`^Decided \\(Carsten, ${date}\\)`, "m"), id);
    assert.match(text, /^Why: /m, `${id} gives no reason`);
    assert.match(
      text,
      /^Not taken:\n\n?- /m,
      `${id} lists no option not taken`,
    );
    assert.doesNotMatch(text, /\*\(recommended/, id);
    assert.doesNotMatch(text, /^Recommendation:/m, id);
  }
  assert.match(
    decision("S14"),
    /Withdrawn by the owner \(Carsten, 2026-11-09\)/,
  );
  assert.match(decision("S25"), /#91/);
  assert.match(decision("S25"), /numbered in the rewrite/);
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
