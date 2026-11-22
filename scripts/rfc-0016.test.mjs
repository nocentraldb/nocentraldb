// RFC 0016 designs moving an object from one space to another (NOC-447), so
// that a single note can be shared. It is decided and published as a fresh
// text, as docs/rfc/README.md asks of a newly published RFC: each section
// states what the design does and why, then the options not taken, in plain
// words. It has no list of decisions, no option labels to decode and no
// Decisions section; who decided each choice stays in factory/decisions/.
// It names the GENESIS principles it serves and touches. These tests check
// that each part is there; whether the design is right was the owner's
// review.
import assert from "node:assert/strict";
import { readdirSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { historyLines } from "./lib/rfc-record.mjs";
import { headings, read, root, section } from "./lib/rfc-text.mjs";

const file = () => {
  const found = readdirSync(join(root, "docs/rfc")).filter(
    (f) => f.startsWith("0016-") && f.endsWith(".md"),
  );
  assert.equal(found.length, 1, "docs/rfc holds one RFC 0016");
  return `docs/rfc/${found[0]}`;
};

const text = () => read(file());

/** The prose of the RFC: no fenced code. */
const prose = () => text().replace(/^```[\s\S]*?^```$/gm, "");

test("RFC 0016 is decided and is about moving an object", () => {
  const markdown = text();
  assert.match(markdown.split("\n")[0], /^# RFC 0016: .*[Mm]ov/);
  assert.equal(
    markdown.split("\n")[2],
    "REQUEST FOR COMMENTS · NOVEMBER 2026 · Ticket NOC-447",
  );
  assert.match(
    markdown,
    /^\*\*Status\.\*\* Decided\. .*\(Carsten, 2026-11-22\)/m,
  );
});

test("RFC 0016 states its design directly, with no list of decisions", () => {
  const found = headings(text());
  for (const heading of [
    "What the owner decided",
    "Questions to the owner",
    "Decisions",
  ]) {
    assert.ok(!found.includes(heading), `it has a section "${heading}"`);
  }
  const body = prose();
  assert.doesNotMatch(body, /\*\*Chosen\b/);
  assert.doesNotMatch(body, /\bRecommended\b/);
  assert.doesNotMatch(body, /[Dd]ecided by the owner/);
  assert.doesNotMatch(body, /NOC-449/, "who decided stays in the decision log");
  const label = body.match(/\b(?:[HLKMRPET]\d|option [A-Z]\b)/);
  assert.ok(!label, `an option label the reader must decode: ${label?.[0]}`);
});

test("RFC 0016's History is its one publication line", () => {
  const lines = historyLines(text());
  assert.deepEqual(lines, ["- 2026-11-22 · First published (NOC-447)"]);
});

// Each open choice has its own section, which states what the design does
// and then the options not taken, each with why.
const CHOICES = [
  ["the-objects-identity", "identity"],
  ["its-history", "history"],
  ["later-edits", "later edits"],
  ["links-in-the-moved-object", "links in the moved object"],
  ["links-to-the-moved-object", "links to the moved object"],
  ["references-to-the-moved-object", "references"],
  ["its-personal-fields", "personal fields"],
  ["erasure", "erasure"],
  ["what-the-members-of-the-old-space-keep", "what the old space keeps"],
];

for (const [anchor, name] of CHOICES) {
  test(`RFC 0016 states its design on ${name}, then the options not taken`, () => {
    const part = section(text(), anchor);
    const at = part.indexOf("**Options not taken.**");
    assert.ok(at > 0, 'no "**Options not taken.**" after the design');
    const options = part
      .slice(at)
      .split("\n")
      .filter((l) => l.startsWith("- "));
    assert.ok(options.length >= 1, "no option not taken is listed");
  });
}

test("RFC 0016 names the GENESIS principles it serves and where it touches them", () => {
  const part = section(text(), "agreement-with-genesismd");
  for (const n of [1, 2, 3, 4, 5, 6, 7]) {
    assert.match(part, new RegExp(`\\*\\*Principle ${n}\\b`), `principle ${n}`);
  }
  assert.match(part, /[Tt]ouches/);
});

test("the decision log holds one file per decision of RFC 0016", () => {
  const files = readdirSync(join(root, "factory/decisions")).filter((f) =>
    f.startsWith("2026-11-22-NOC-447-"),
  );
  assert.equal(files.length, 8, files.join("\n"));
  for (const f of files) {
    const entry = read(`factory/decisions/${f}`);
    assert.match(entry, /^- Decided by: Carsten, 2026-11-22\b.*NOC-449/m);
    assert.match(entry, /^- Topics: .*\brfc-0016\b/m);
  }
});

test("the RFC lists name RFC 0016 without calling it a draft", () => {
  const index = read("docs/rfc/README.md");
  const row = index.split("\n").find((l) => l.startsWith("| [0016]"));
  assert.ok(row, "the RFC index has no row for RFC 0016");
  assert.doesNotMatch(row, /draft/);
  const site = read("site/src/content/docs/docs/rfcs.md");
  const item = site.split("\n").find((l) => l.includes("[RFC 0016: "));
  assert.ok(item, "the site's RFC list has no item for RFC 0016");
  assert.doesNotMatch(item, /draft/);
  assert.match(item, /\/docs\/rfc\/0016-/);
});
