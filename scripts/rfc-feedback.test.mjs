// Holds the follow-up of the owner's decision on where RFC feedback goes
// (NOC-53, merged as #65; NOC-55): the decision log records it, pm's role
// reads the RFC feedback category on every tick and reports new replies in the
// daily note, and RFCs 0002 to 0005, 0007 and 0008 each tell readers how to
// give feedback. RFC 0006's line belongs to NOC-34, and RFC 0001 already has
// its own section. The test checks that each rule is there and where it
// points; whether the wording is right is the review of the PR.
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (path) => readFileSync(join(root, path), "utf8");

const CATEGORY =
  "https://github.com/nocentraldb/nocentraldb/discussions/categories/rfc-feedback";
const DISCUSSION =
  /https:\/\/github\.com\/nocentraldb\/nocentraldb\/discussions\/\d+/;
const RFCS = ["0002", "0003", "0004", "0005", "0007", "0008"];

/** The text under `## <heading>` in `markdown`, up to the next `## ` heading. */
const section = (markdown, heading) => {
  const [, rest] = markdown.split(`\n## ${heading}\n`);
  assert.ok(rest !== undefined, `no "## ${heading}" section`);
  return rest.split("\n## ")[0];
};

test("the decision log records the owner's decision on where RFC feedback goes", () => {
  const files = readdirSync(join(root, "factory/decisions")).filter((name) =>
    /^\d{4}-\d{2}-\d{2}-NOC-55-/.test(name),
  );
  assert.equal(files.length, 1, "expected one NOC-55 decision file");
  const entry = read(`factory/decisions/${files[0]}`);
  assert.match(entry, /^- Decided by: .*Carsten.*#65/m);
  assert.match(entry, /GitHub Discussions/);
  assert.match(entry, /RFC feedback/);
  assert.match(entry, /pointer/, "the RFC's discussion holds a pointer");
  assert.match(entry, /copy/, "instead of a copy of the RFC");
  assert.match(entry, /public/, "announce only once the repo is public");
  assert.match(entry, /current|up to date/, "and the discussion is current");
  assert.match(entry, /week/, "and someone can answer within a week");
});

test("pm lists the RFC feedback category over REST on every tick", () => {
  const tick = section(read("factory/roles/pm.md"), "On every tick");
  const rule = tick.split("\n").find((line) => line.includes("RFC feedback"));
  assert.ok(rule, 'no tick step names the "RFC feedback" category');
  assert.match(rule, /GET \/repos\/\{owner\}\/\{repo\}\/discussions/);
  assert.match(rule, /draft/, "pm drafts the answers");
  assert.match(rule, /owner/, "the owner reviews and posts them");
  assert.match(rule, /never post|does not post|not post/);
});

test('the daily note has an "RFC feedback" heading with links to new replies', () => {
  const note = section(read("factory/roles/pm.md"), "The daily note");
  assert.match(note, /"RFC feedback"/);
  assert.match(note, /link/);
  assert.match(note, /new repl/);
});

for (const number of RFCS) {
  test(`RFC ${number} says how to give feedback`, () => {
    const [file] = readdirSync(join(root, "docs/rfc")).filter(
      (name) => name.startsWith(`${number}-`) && name.endsWith(".md"),
    );
    const line = read(`docs/rfc/${file}`)
      .split("\n")
      .find((text) => text.startsWith("**How to give feedback.**"));
    assert.ok(line, `RFC ${number} has no "How to give feedback" line`);
    assert.match(line, /\(README\.md#giving-feedback\)/);
    // Its own discussion where one exists, else a new one in the category.
    assert.ok(
      DISCUSSION.test(line) || line.includes(`(${CATEGORY})`),
      `RFC ${number} links neither its discussion nor the category`,
    );
  });
}
