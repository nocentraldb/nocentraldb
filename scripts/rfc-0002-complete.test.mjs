// Holds RFC 0002 as one complete text (NOC-100). The owner wants every RFC to
// state the current design directly (Carsten, 2026-11-10): no amendment
// section, mark or changelog is left, "Decisions" keeps each decision in short
// form (what was chosen, by whom and when, why, and the options not taken),
// and a dated "History" names the pull request of each change. The rules the
// six amendments brought in are checked where they now stand by
// rfc-0002-rules.test.mjs. Whether the wording is right is the review of the
// PR.
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join, normalize } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (path) => readFileSync(join(root, path), "utf8");

const RFC = "docs/rfc/0002-change-and-log-format.md";

/** GitHub's heading anchor: lower case, punctuation dropped, spaces to dashes. */
const slug = (heading) =>
  heading
    .trim()
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s_-]/gu, "")
    .replace(/\s/g, "-");

/** The text under the heading whose anchor is `anchor`, up to the next heading of the same or a higher level. */
const section = (markdown, anchor) => {
  const lines = markdown.split("\n");
  const start = lines.findIndex(
    (line) =>
      /^#{1,6} /.test(line) && slug(line.replace(/^#+ /, "")) === anchor,
  );
  if (start < 0) return undefined;
  const level = lines[start].match(/^#+/)[0].length;
  const rest = lines.slice(start + 1);
  const end = rest.findIndex((line) => {
    const m = line.match(/^(#+) /);
    return m && m[1].length <= level;
  });
  return (end < 0 ? rest : rest.slice(0, end)).join("\n");
};

/** The Markdown link targets in `text`. */
const targets = (text) =>
  [...text.matchAll(/\]\(([^)\s]+)\)/g)].map((m) => m[1]);
const links = (text, target) => targets(text).some((t) => t.endsWith(target));

/** The decision under "Decisions" whose heading starts with `prefix`, such as "D3.". */
const decision = (prefix) => {
  const decisions = section(read(RFC), "decisions");
  assert.ok(decisions, 'no section "Decisions"');
  const heading = decisions
    .split("\n")
    .find((line) => line.startsWith(`### ${prefix}`));
  assert.ok(heading, `no decision "${prefix}"`);
  return section(decisions, slug(heading.slice(4)));
};

/** Every Markdown file a reader can follow a link from. */
const documents = () => [
  "README.md",
  "CONTRIBUTING.md",
  ...readdirSync(join(root, "docs"), { recursive: true })
    .filter((file) => file.endsWith(".md"))
    .map((file) => normalize(join("docs", file))),
];

test("RFC 0002 has no amendment section, mark or changelog", () => {
  const rfc = read(RFC);
  assert.doesNotMatch(rfc, /^## Amendment/m, "an amendment section is left");
  assert.doesNotMatch(rfc, /\(Amendment \d+/, "an amendment mark is left");
  assert.doesNotMatch(rfc, /\*\*Changelog\.\*\*/, "the changelog is left");
  assert.doesNotMatch(rfc, /amendment/i, "the text still speaks of amendments");
});

test("no document links an amendment section of RFC 0002", () => {
  const found = documents().filter((path) =>
    /0002-change-and-log-format\.md#amendment-/.test(read(path)),
  );
  assert.deepEqual(found, []);
});

test("the status says the RFC is decided and points to Decisions and History", () => {
  const status = read(RFC)
    .split("\n")
    .find((line) => line.startsWith("**Status.**"));
  assert.ok(status, "no status");
  assert.match(status, /^\*\*Status\.\*\* Decided\./);
  assert.match(status, /#28/);
  assert.match(status, /\(Carsten, 2026-11-0\d\)/);
  assert.ok(links(status, "#decisions"), "the status does not link Decisions");
  assert.ok(links(status, "#history"), "the status does not link History");
  assert.doesNotMatch(status, /proposal for the owner/);
});

test("D1 to D13 each say what was chosen, by the owner and when, why, and what was not taken", () => {
  for (let n = 1; n <= 13; n += 1) {
    const text = decision(`D${n}.`);
    const date = n <= 9 ? "2026-11-07" : "2026-11-08";
    assert.match(
      text,
      new RegExp(`^Decided \\(Carsten, ${date}\\): `, "m"),
      `D${n}: no owner and date`,
    );
    assert.match(text, /^Why: /m, `D${n}: no reason`);
    assert.match(text, /^Not taken:$/m, `D${n}: no options not taken`);
    assert.match(text, /^- /m, `D${n}: lists no option not taken`);
    assert.doesNotMatch(text, /\(recommended\)/, `D${n} is not in short form`);
  }
});

test("D3 says that RFC 0008's M7 superseded it for space ids", () => {
  const text = decision("D3.");
  assert.match(text, /M7/);
  assert.match(text, /space ids?/);
  assert.ok(links(text, "0008-membership.md#what-it-names"));
});

test("D13 parks a payload that does not decode, and names the two options not taken", () => {
  const text = decision("D13.");
  assert.match(
    text,
    /^Decided \(Carsten, 2026-11-08\): .*parked.*"unknown payload format"/im,
  );
  assert.match(text, /^- Rejected/m, "B is not listed as not taken");
  assert.match(text, /^- Split by cause/m, "C is not listed as not taken");
});

test("the owner's decisions without a number are recorded: key 64, the nesting limit and the clock's carry", () => {
  const model = decision("The writer's model in payload key 64");
  assert.match(model, /\(Carsten, 2026-11-0\d\)/);
  assert.match(model, /RFC 0005's D10/);
  assert.match(model, /D13/);
  assert.match(model, /#67/);
  const nesting = decision("The nesting limit");
  assert.match(nesting, /^Decided \(Carsten, 2026-11-09\): /m);
  assert.match(nesting, /NOC-24/);
  assert.match(nesting, /32 levels/);
  assert.match(nesting, /128 levels/, "the limit not taken is not named");
  const clock = decision("A full clock counter");
  assert.match(clock, /^Decided \(Carsten, 2026-11-09\): /m);
  assert.match(clock, /NOC-25/);
  assert.match(clock, /next millisecond/);
});

test("\"Decisions this RFC follows\" names RFC 0008's and RFC 0007's decisions, each linked", () => {
  const followed = section(read(RFC), "decisions-this-rfc-follows");
  assert.ok(followed, 'no section "Decisions this RFC follows"');
  const lines = followed.split("\n").filter((l) => l.startsWith("- **"));
  for (const [id, target] of [
    ["M3", "0008-membership.md#the-cutoff"],
    ["M6", "0008-membership.md#generation-ids"],
    ["M7", "0008-membership.md#what-it-names"],
    ["M11", "0008-membership.md#how-the-relay-applies-the-rules"],
    ["M18", "0008-membership.md#the-control-bodies"],
    ["M19", "0008-membership.md#how-the-relay-applies-the-rules"],
    ["M24", "0008-membership.md#how-the-relay-and-members-recognise-it"],
    ["conflict 1", "0007-relay.md#what-the-relay-sees"],
    ["conflict 4", "0007-relay.md#what-the-relay-checks"],
  ]) {
    const line = lines.find((l) => new RegExp(`\\b${id}\\b`).test(l));
    assert.ok(line, `no decision followed names ${id}`);
    assert.match(line, /\(Carsten, 2026-11-0\d\)/, `${id}: no owner and date`);
    assert.ok(links(line, target), `${id}: no link to ${target}`);
  }
});

test('"History" lists each change by date, oldest first, with its pull request', () => {
  const history = section(read(RFC), "history");
  assert.ok(history, 'no section "History"');
  const lines = history.split("\n").filter((l) => l.startsWith("- **"));
  const dates = lines.map(
    (l) => l.match(/^- \*\*(\d{4}-\d{2}-\d{2})\*\*/)?.[1],
  );
  assert.ok(dates.every(Boolean), "a History line has no date");
  assert.deepEqual([...dates].sort(), dates, "History is not oldest first");
  for (const line of lines) {
    assert.match(
      line,
      /\]\(https:\/\/github\.com\/nocentraldb\/nocentraldb\/pull\/\d+\)/,
      `no pull request: ${line.slice(0, 60)}`,
    );
  }
  for (const ticket of [
    "NOC-6",
    "NOC-22",
    "NOC-38",
    "NOC-44",
    "NOC-47",
    "NOC-40",
    "NOC-62",
    "NOC-100",
  ]) {
    assert.ok(
      lines.some((l) => new RegExp(`\\b${ticket}\\b`).test(l)),
      `History does not name ${ticket}`,
    );
  }
});
