// Holds the rule of NOC-100 for how a decided RFC changes: docs/rfc/README.md
// says that a decided change is written into the RFC's text, with a dated
// History line that names its pull request and the decision under
// "Decisions", and that an RFC has no amendment sections. Its "What happens to
// your feedback" list and CONTRIBUTING.md say the same. The test
// checks that each rule is there and where it points; whether the wording is
// right is the review of the PR.
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (path) => readFileSync(join(root, path), "utf8");
const readme = () => read("docs/rfc/README.md");

const HEADING = "How a decided RFC changes";
const ANCHOR = "#how-a-decided-rfc-changes";

/** The text under `## <heading>` in `markdown`, up to the next `## ` heading. */
const section = (markdown, heading) => {
  const [, rest] = markdown.split(`\n## ${heading}\n`);
  assert.ok(rest !== undefined, `no "## ${heading}" section`);
  return rest.split("\n## ")[0];
};

test(`the RFC README has a "${HEADING}" section with the rule`, () => {
  const rule = section(readme(), HEADING);
  assert.match(
    rule,
    /written into the (RFC's )?text/,
    "the change goes into the text",
  );
  assert.match(rule, /"History"/, "a History line records the change");
  assert.match(rule, /dated/, "the History line is dated");
  assert.match(rule, /pull request/, "the History line names the pull request");
  assert.match(rule, /"Decisions"/, "the decision goes under Decisions");
  assert.match(
    rule,
    /no amendment section/,
    "an RFC has no amendment sections",
  );
  assert.match(rule, /withdrawn/, "a withdrawn decision stays listed");
  assert.match(rule, /owner/, "a change to a decision waits for the owner");
});

test("the RFC README speaks of amendments only to rule them out", () => {
  const rest = readme().replace(section(readme(), HEADING), "");
  assert.doesNotMatch(rest, /amendment/i);
});

test("the intro says each RFC is one complete text with a History section", () => {
  const intro = readme().split("\n| RFC |")[0];
  assert.match(intro, /complete text/);
  assert.match(intro, /"History"/);
});

test("the first end of a feedback point is a change to the RFC's text", () => {
  const ends = section(readme(), "What happens to your feedback");
  const first = ends.split("\n").find((line) => line.startsWith("1. "));
  assert.ok(first, "the list has no first item");
  assert.match(first, /change to the RFC's text/);
  assert.ok(first.includes(`(${ANCHOR})`), `the item does not link ${ANCHOR}`);
  assert.match(first, /discussion/, "its History line links the discussion");
});

test("CONTRIBUTING.md names the same ends, without amendments", () => {
  const contributing = read("CONTRIBUTING.md")
    .split("\n")
    .find((line) => line.includes("What happens to your feedback"));
  assert.ok(
    contributing,
    'CONTRIBUTING.md does not link "What happens to your feedback"',
  );
  assert.doesNotMatch(contributing, /amendment/i);
  assert.match(
    contributing,
    /change to an RFC's text|change to the RFC's text/,
  );
});

test("every RFC is one complete text, so the README no longer says some are being rewritten", () => {
  const rfcs = readdirSync(join(root, "docs/rfc")).filter((f) =>
    /^\d{4}-.*\.md$/.test(f),
  );
  for (const file of rfcs) {
    assert.doesNotMatch(
      read(`docs/rfc/${file}`),
      /^## Amendment/m,
      `${file} still has an amendment section`,
    );
  }
  assert.doesNotMatch(readme(), /being rewritten/);
});
