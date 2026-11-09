// Asserts RFC 0004's Amendment 4 (NOC-63): the field type `ncdb/ref`, a
// reference into another space, which the owner decided in NOC-29 (Carsten,
// 2026-11-09). The amendment gives its state, operation, merge, resolution
// and encoding in Amendment 1's tables and style; its byte-exact examples
// sit beside E1 to E14 and are checked by rfc-0004-examples.test.mjs. Every
// point beyond NOC-29's decision goes to the owner as a numbered decision
// with options and a recommendation, still open.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const RFC = "docs/rfc/0004-merge-rules-basic-types.md";
const AMENDMENT = "amendment-4-a-reference-into-another-space";
const MARK = "*(Amendment 4)*";
const MARK_SENTENCE = "*(Amendment 4.)*";

const read = () => readFileSync(join(root, RFC), "utf8");

/** GitHub's heading slug: lower case, punctuation dropped, spaces to dashes. */
const slug = (heading) =>
  heading
    .trim()
    .toLowerCase()
    .replace(/[^\p{L}\p{N} _-]/gu, "")
    .replace(/ /g, "-");

/** The text of the level-`level` section whose anchor is `anchor`, heading included. */
const section = (markdown, anchor, level = 2) => {
  const hashes = "#".repeat(level);
  const part = markdown
    .split(new RegExp(`^(?=${hashes} )`, "m"))
    .find((p) => slug(p.split("\n")[0].slice(level + 1)) === anchor);
  if (!part) return undefined;
  const [heading, ...rest] = part.split("\n");
  return [
    heading,
    rest.join("\n").split(new RegExp(`^#{1,${level}} `, "m"))[0],
  ].join("\n");
};

/** The first line of `markdown` that starts with `start`. */
const line = (markdown, start) =>
  markdown.split("\n").find((l) => l.startsWith(start)) ?? "";

/** The numbered decisions ("15. **Title.**") of a text, each up to the next. */
const decisions = (text) =>
  [
    ...text.matchAll(/^(\d+)\. \*\*[\s\S]*?(?=^\d+\. \*\*|^#|(?![\s\S]))/gm),
  ].map((m) => ({ number: Number(m[1]), text: m[0] }));

const amendment = () => {
  const body = section(read(), AMENDMENT);
  assert.ok(body, `${RFC} has no section #${AMENDMENT}`);
  return body;
};

test("RFC 0004 announces Amendment 4 in a dated NOC-63 changelog line and its status", () => {
  const top = read().split(/^## /m)[0];
  const ours = top
    .split("\n")
    .find((l) => /^- \*\*\d{4}-\d{2}-\d{2}\*\* · NOC-63 · /.test(l));
  assert.ok(ours, "no dated changelog line for NOC-63");
  assert.ok(ours.includes(`(#${AMENDMENT})`), `it does not link #${AMENDMENT}`);
  assert.match(ours, /NOC-29/);
  const status = line(top, "**Status.**");
  assert.ok(
    status.includes(`(#${AMENDMENT})`),
    "the status does not link Amendment 4",
  );
  assert.match(status, /Amendment 4[^.]*decisions 15 to 20[^.]*open/);
});

test("Amendment 4 records what the owner decided in NOC-29, with name and date", () => {
  const body = amendment();
  assert.match(body, /NOC-29/);
  assert.match(body, /\(Carsten, 2026-11-09\)/);
  for (const decided of [
    /space id and (its|the) object id/,
    /last writer wins/,
    /readable/,
    /not accessible/,
    /missing/,
    /indexed/,
    /[Cc]omputed fields and views never follow/,
    /no access to the target space/,
  ]) {
    assert.match(body, decided, `the amendment does not say ${decided}`);
  }
});

test("Amendment 4 has the sections of Amendment 1 that a field type needs", () => {
  const body = amendment();
  const headings = body
    .split("\n")
    .filter((l) => l.startsWith("### "))
    .map((l) => l.slice(4));
  for (const wanted of [
    "State and operations",
    "Resolution",
    "The value and its encoding",
    "Byte-exact examples (E15 to E17)",
    "What the relay and the members see",
    "What this does not do",
    "What this means for the RFCs and tickets built on it",
    "Decisions for the owner (15 to 20)",
  ]) {
    assert.ok(headings.includes(wanted), `no section "### ${wanted}"`);
  }
});

test("the resolution table defines the three states by the member's copy", () => {
  const resolution = section(amendment(), "resolution", 3);
  assert.ok(resolution, "no Resolution section in Amendment 4");
  const rows = resolution.split("\n").filter((l) => /^\| \*\*/.test(l));
  assert.deepEqual(
    rows.map((r) => r.match(/^\| \*\*([^*]+)\*\*/)[1]),
    ["ok", "not accessible", "missing"],
  );
  assert.match(resolution, /differ(s)? from member to member/);
  assert.match(resolution, /principle 2/);
  assert.match(resolution, /principle 3/);
});

test("the summary and the operation table name the reference", () => {
  const rfc = read();
  assert.match(line(rfc, "| Reference "), /basic value/i);
  assert.match(line(rfc, "| Reference "), /\(Amendment 4\)/);
  const set = line(rfc, "| 1         | **set**");
  assert.match(set, /reference/, "set (1) does not list the reference");
  assert.ok(set.includes(MARK), "the set row is not marked (Amendment 4)");
});

test("the link section points to nc.ref for a target in another space", () => {
  const rfc = read();
  const paragraph = line(rfc, "A link cannot point into another space");
  assert.match(paragraph, /`nc\.ref`/);
  assert.match(paragraph, new RegExp(`\\(#${AMENDMENT}\\)`));
  assert.ok(paragraph.includes(MARK_SENTENCE), "it is not marked");
});

test("Amendment 4's encoding names the type id, the operation and the value", () => {
  const body = section(amendment(), "the-value-and-its-encoding", 3);
  assert.ok(body, "no section The value and its encoding");
  assert.match(body, /`ncdb\/ref`/);
  assert.match(body, /set \(1\)/);
  assert.match(body, /\[space id, object id\]/);
  assert.match(body, /16-byte byte string/);
  assert.match(body, /decision 1[5-9]|decision 20/);
});

test("decisions 15 to 20 each have options and one recommendation, and stay open", () => {
  const body = section(amendment(), "decisions-for-the-owner-15-to-20", 3);
  assert.ok(body, "no section Decisions for the owner (15 to 20)");
  const found = decisions(body);
  assert.deepEqual(
    found.map((d) => d.number),
    [15, 16, 17, 18, 19, 20],
  );
  for (const { number, text } of found) {
    const options = text.match(/^ {3,4}- [A-Z]: /gm) ?? [];
    assert.ok(
      options.length >= 2,
      `decision ${number} has fewer than two options`,
    );
    assert.equal(
      (text.match(/Recommended\b/g) ?? []).length,
      1,
      `decision ${number} does not recommend exactly one option`,
    );
    assert.match(
      text,
      /Open: waits for the owner\./,
      `decision ${number} does not say it is open`,
    );
  }
  const topics = found.map((d) => d.text);
  assert.match(topics[0], /`ncdb\/ref`/);
  assert.match(topics[1], /\[space id, object id\]/);
  assert.match(topics[2], /does not fit/);
  assert.match(topics[3], /collection the schema does not list/);
  assert.match(topics[4], /own space/);
  assert.match(topics[5], /not accessible/);
});

test("every in-page link of RFC 0004 resolves to a heading", () => {
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
