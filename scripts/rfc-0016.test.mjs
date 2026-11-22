// RFC 0016 designs moving an object from one space to another (NOC-447), so
// that a single note can be shared. It weighs options for each open choice:
// the object's identity, its history, its links and references, its
// personal fields, erasure, and what the members of the old space keep, and
// names the GENESIS principles it serves and touches. The owner followed
// every recommendation on NOC-449 (Carsten, 2026-11-22), so the RFC is
// decided: each choice names the option chosen, "What the owner decided"
// records the answers, and the decision log holds one file per decision.
// These tests check that each part is there; whether the design is right
// was the owner's review.
import assert from "node:assert/strict";
import { readdirSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { headings, read, root, section } from "./lib/rfc-text.mjs";

const DECIDED = "(Carsten, 2026-11-22, NOC-449)";

const file = () => {
  const found = readdirSync(join(root, "docs/rfc")).filter(
    (f) => f.startsWith("0016-") && f.endsWith(".md"),
  );
  assert.equal(found.length, 1, "docs/rfc holds one RFC 0016");
  return `docs/rfc/${found[0]}`;
};

const text = () => read(file());

test("RFC 0016 is decided by the owner and is about moving an object", () => {
  const markdown = text();
  assert.match(markdown.split("\n")[0], /^# RFC 0016: .*[Mm]ov/);
  assert.equal(
    markdown.split("\n")[2],
    "REQUEST FOR COMMENTS · NOVEMBER 2026 · Ticket NOC-447",
  );
  assert.match(
    markdown,
    /^\*\*Status\.\*\* Decided by the owner \(Carsten, 2026-11-22\), on NOC-449\b/m,
  );
  assert.doesNotMatch(markdown, /\*\*Recommended[:.]/);
});

// Each open choice has its own section, which weighs at least two options
// and names the one the owner chose.
const CHOICES = [
  ["the-objects-identity", "identity", "A"],
  ["its-history", "history", "H1"],
  ["later-edits", "later edits", "L1"],
  ["links-in-the-moved-object", "links in the moved object", "K1"],
  ["links-to-the-moved-object", "links to the moved object", "M1"],
  ["references-to-the-moved-object", "references", "R1"],
  ["its-personal-fields", "personal fields", "P1"],
  ["erasure", "erasure", "E1"],
  ["what-the-members-of-the-old-space-keep", "what the old space keeps", "T1"],
];

for (const [anchor, name, chosen] of CHOICES) {
  test(`RFC 0016 weighs options on ${name} and names ${chosen} as chosen`, () => {
    const part = section(text(), anchor);
    const options = part.match(/^\| \*\*[A-Z]\d?\.?/gm) ?? [];
    assert.ok(options.length >= 2, `fewer than two options: ${options}`);
    assert.ok(
      part.includes(`**Chosen: ${chosen}.** ${DECIDED}`),
      `no "**Chosen: ${chosen}.** ${DECIDED}"`,
    );
  });
}

test("RFC 0016 names the GENESIS principles it serves and where it touches them", () => {
  const part = section(text(), "agreement-with-genesismd");
  for (const n of [1, 2, 3, 4, 5, 6, 7]) {
    assert.match(part, new RegExp(`\\*\\*Principle ${n}\\b`), `principle ${n}`);
  }
  assert.match(part, /[Tt]ouches/);
});

test("RFC 0016 records the owner's eight answers, each dated, and asks nothing open", () => {
  const found = headings(text());
  assert.ok(found.includes("What the owner decided"));
  assert.ok(!found.includes("Questions to the owner"));
  const answers = section(text(), "what-the-owner-decided")
    .split("\n")
    .filter((l) => /^\d+\. /.test(l));
  assert.equal(answers.length, 8);
  for (const a of answers) assert.ok(a.includes(DECIDED), a.slice(0, 60));
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
