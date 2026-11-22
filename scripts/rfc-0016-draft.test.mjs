// RFC 0016 drafts moving an object from one space to another (NOC-447), so
// that a single note can be shared. The ticket asks for a draft that weighs
// options and recommends one for each open choice: the object's history, its
// links and references, its personal fields, erasure, and what the members
// of the old space keep. It names the GENESIS principles it serves and
// touches, and its questions go to the owner. These tests check that each
// part is there; whether the design is right is the owner's review.
import assert from "node:assert/strict";
import { readdirSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { headings, read, root, section } from "./lib/rfc-text.mjs";

const file = () => {
  const found = readdirSync(join(root, "docs/rfc")).filter(
    (f) => f.startsWith("0016-") && f.endsWith(".md"),
  );
  assert.equal(found.length, 1, "docs/rfc holds one RFC 0016");
  return `docs/rfc/${found[0]}`;
};

const text = () => read(file());

test("RFC 0016 is a draft about moving an object between spaces", () => {
  const markdown = text();
  assert.match(markdown.split("\n")[0], /^# RFC 0016: .*[Mm]ov/);
  assert.match(markdown.split("\n")[2], /NOC-447 · Draft$/);
  assert.match(markdown, /^\*\*Status\.\*\* Draft\b/m);
});

// Each open choice the ticket names has its own section, which weighs at
// least two options and recommends one.
const CHOICES = [
  ["its-history", "history"],
  ["its-links-and-references", "links and references"],
  ["its-personal-fields", "personal fields"],
  ["erasure", "erasure"],
  ["what-the-members-of-the-old-space-keep", "what the old space keeps"],
];

for (const [anchor, name] of CHOICES) {
  test(`RFC 0016 weighs options on ${name} and recommends one`, () => {
    const part = section(text(), anchor);
    const options = part.match(/^\| \*\*[A-Z]\d?\.?/gm) ?? [];
    assert.ok(options.length >= 2, `fewer than two options: ${options}`);
    assert.match(part, /\*\*Recommended[:.]/);
  });
}

test("RFC 0016 names the GENESIS principles it serves and where it touches them", () => {
  const part = section(text(), "agreement-with-genesismd");
  for (const n of [1, 2, 3, 4, 5, 6, 7]) {
    assert.match(part, new RegExp(`\\*\\*Principle ${n}\\b`), `principle ${n}`);
  }
  assert.match(part, /[Tt]ouches/);
});

test("RFC 0016 lists its questions to the owner, each with a recommendation", () => {
  const found = headings(text());
  assert.ok(found.includes("Questions to the owner"));
  const questions = section(text(), "questions-to-the-owner")
    .split("\n")
    .filter((l) => /^\d+\. /.test(l));
  assert.ok(questions.length >= 5, `${questions.length} questions`);
  for (const q of questions) assert.match(q, /Recommended/, q.slice(0, 60));
});

test("the RFC lists name RFC 0016 as a draft", () => {
  assert.match(
    read("docs/rfc/README.md"),
    /\| \[0016\]\(0016-[a-z0-9-]+\.md\) \| .*\(draft\) \|/,
  );
  assert.match(
    read("site/src/content/docs/docs/rfcs.md"),
    /\[RFC 0016: .*\(draft\)\]\(https:\/\/github\.com\/nocentraldb\/nocentraldb\/blob\/main\/docs\/rfc\/0016-/,
  );
});
