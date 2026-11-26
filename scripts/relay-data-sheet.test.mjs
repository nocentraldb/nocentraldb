// Asserts that docs/relay-data-sheet.md, the data sheet for relay operators
// (NOC-527), does what the ticket asks: one row per item the relay stores or
// sees, each with its purpose, its default retention, where an operator
// changes it, and the RFC text it comes from; a plain statement that the
// operator never sees content or keys; the gaps where no RFC text decides;
// and that the page is a template, not legal advice. The page decides
// nothing new, so every retention period it states stands in the RFC it
// cites.
import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { assertLinksResolve, read, root, section } from "./lib/rfc-text.mjs";

const SHEET = "docs/relay-data-sheet.md";

/** The rows of the first Markdown table in `markdown`, as arrays of cells, without the header and the rule. */
const tableRows = (markdown) => {
  const lines = markdown.split("\n").filter((l) => l.startsWith("|"));
  return lines
    .slice(2)
    .map((l) =>
      l
        .slice(1, -1)
        .split(" | ")
        .map((c) => c.trim()),
    );
};

const sheet = () => read(SHEET);
const rows = () => tableRows(section(sheet(), "what-the-relay-keeps"));

test("the data sheet exists and every link in it resolves", () => {
  assert.ok(existsSync(join(root, SHEET)), `${SHEET} exists`);
  assertLinksResolve(SHEET);
});

test("the table names each item, why, how long, where to change it, and its source", () => {
  const header = section(sheet(), "what-the-relay-keeps")
    .split("\n")
    .find((l) => l.startsWith("|"));
  assert.equal(
    header,
    "| What | Why the relay keeps it | How long, by default | Where an operator changes it | Source |",
  );
  for (const row of rows()) {
    assert.equal(row.length, 5, `five cells: ${row[0]}`);
    for (const cell of row) assert.ok(cell.length > 0, `no empty cell: ${row[0]}`);
    assert.match(
      row[4],
      /\]\(rfc\/00(01|02|03|07|08|10|15)-[^)]+#[^)]+\)/,
      `${row[0]} cites an RFC section`,
    );
  }
});

test("the table covers every item the ticket lists", () => {
  const items = rows().map((r) => r[0].toLowerCase());
  const has = (pattern, what) =>
    assert.ok(
      items.some((i) => pattern.test(i)),
      `a row for ${what}; rows: ${items.join(" / ")}`,
    );
  has(/member keys/, "member keys");
  has(/membership/, "memberships");
  has(/admission/, "admissions");
  has(/encrypted changes/, "encrypted changes");
  has(/sizes/, "the sizes of changes");
  has(/connection records/, "connection records");
  has(/network address/, "network addresses");
  has(/logs/, "logs");
  has(/backup/, "backups");
});

test("every retention period the sheet states stands in the RFC it comes from", () => {
  const rfc7 = read("docs/rfc/0007-relay.md");
  const periods = {
    "14 days": rfc7,
    "35 days": rfc7,
    "30 days": rfc7,
    "15 minutes": rfc7,
    "2 hours": rfc7,
    "one day": rfc7,
  };
  const text = sheet();
  for (const [period, source] of Object.entries(periods)) {
    assert.ok(text.includes(period), `the sheet states ${period}`);
    assert.ok(source.includes(period), `RFC 0007 states ${period}`);
  }
  const found = [...text.matchAll(/\b(\d+) (days?|hours?|minutes?)\b/g)].map(
    (m) => m[0],
  );
  for (const period of found) {
    assert.ok(
      rfc7.includes(period) || read("docs/rfc/0015-relay-admission.md").includes(period),
      `${period} in the sheet stands in RFC 0007 or RFC 0015`,
    );
  }
});

test("it says plainly that the operator never sees content or keys", () => {
  const never = section(sheet(), "what-the-operator-never-sees");
  assert.match(never, /content/);
  assert.match(never, /keys?/);
  assert.match(never, /\]\(rfc\/0007-relay\.md#what-the-relay-sees\)/);
  assert.match(never, /\]\(rfc\/0001-nocentraldb\.md#[^)]+\)/);
});

test("it names the gaps where no RFC text decides, for pm to ticket", () => {
  const gaps = section(sheet(), "where-the-rfcs-say-nothing-yet");
  assert.ok(
    gaps.split("\n").filter((l) => l.startsWith("- ")).length >= 3,
    "at least three gaps, one per bullet",
  );
});

test("it is written for an operator to adapt, and says it is not legal advice", () => {
  const text = sheet();
  assert.match(text, /not legal advice/);
  assert.match(text, /adapt/);
});

test("it stays an operator doc in the repository, not a page of the site", () => {
  assert.doesNotMatch(read("site/src/repo-docs.mjs"), /relay-data-sheet/);
  assert.match(read("docs/relay-deploy.md"), /\]\(relay-data-sheet\.md\)/);
});
