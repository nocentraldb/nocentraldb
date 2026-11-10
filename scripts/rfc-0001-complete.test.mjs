// Holds RFC 0001 as one complete text (NOC-100). The owner wants every RFC to
// state the current design directly (Carsten, 2026-11-10): the rules its six
// amendments brought in stand where they belong, no amendment section, mark or
// changelog is left, "Decisions" keeps each decision in short form, and a
// dated "History" names the pull request of each change. The rules of the
// amendments are checked where they now stand; the rules that other tests
// already check where they stand (the Spaces bullet, the personal fields, the
// erasure limits, the example's index) stay with those tests. Whether the
// wording is right is the review of the PR.
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join, normalize } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (path) => readFileSync(join(root, path), "utf8");

const RFC_0001 = "docs/rfc/0001-nocentraldb.md";
const OWNER = /\(Carsten, 2026-11-\d{2}\)/;

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

/** The line of RFC 0001 that starts with the bullet `- **<title>**`. */
const bullet = (title) => {
  const line = read(RFC_0001)
    .split("\n")
    .find((l) => l.startsWith(`- **${title}**`));
  assert.ok(line, `RFC 0001 has no bullet "${title}"`);
  return line;
};

/** Every Markdown file a reader can follow a link from. */
const documents = () => [
  "README.md",
  "CONTRIBUTING.md",
  ".github/SECURITY.md",
  ...readdirSync(join(root, "docs"), { recursive: true })
    .filter((file) => file.endsWith(".md"))
    .map((file) => normalize(join("docs", file))),
];

test("RFC 0001 has no amendment section, mark or changelog", () => {
  const rfc = read(RFC_0001);
  assert.doesNotMatch(rfc, /^## Amendment/m, "an amendment section is left");
  assert.doesNotMatch(rfc, /\(Amendment \d+/, "an amendment mark is left");
  assert.doesNotMatch(rfc, /\*\*Changelog\.\*\*/, "the changelog is left");
  assert.doesNotMatch(rfc, /amendment/i, "the text still speaks of amendments");
});

test("no document links an amendment section of RFC 0001", () => {
  const found = documents().filter((path) =>
    /0001-nocentraldb\.md#amendment-/.test(read(path)),
  );
  assert.deepEqual(found, []);
});

test('"Decisions" records that groups leave the design, and keeps M10 as withdrawn', () => {
  const decisions = section(read(RFC_0001), "decisions");
  assert.ok(decisions, 'no section "Decisions"');
  const groups = section(decisions, "groups-leave-the-design");
  assert.ok(groups, 'no decision "Groups leave the design"');
  assert.match(groups, /\(Carsten, 2026-11-09\)/, "no owner and date");
  assert.match(groups, /NOC-76/);
  assert.match(groups, /later RFC/, "does not say groups may come back");
  assert.match(groups, /Not taken/, "no options not taken");
  assert.match(groups, /M10.*withdrawn by the owner \(Carsten, 2026-11-09\)/);
  assert.ok(links(groups, "0008-membership.md#groups"));
});

test('"Decisions" names each decision of another RFC that the text follows, with owner, date and link', () => {
  const followed = section(read(RFC_0001), "decisions-this-rfc-follows");
  assert.ok(followed, 'no section "Decisions this RFC follows"');
  const lines = followed.split("\n").filter((l) => l.startsWith("- **"));
  for (const [id, target] of [
    ["M11", "0008-membership.md#how-the-relay-applies-the-rules"],
    ["M20", "0008-membership.md#effects-that-wait-until-a-change-is-stable"],
    ["M21", "0008-membership.md#effects-that-wait-until-a-change-is-stable"],
    ["conflict 1", "0007-relay.md#what-the-relay-sees"],
    ["decision 13", "0006-local-view.md#the-browser"],
    ["D9", "0005-schema-and-model-lock.md#personal-fields"],
    ["D15", "0005-schema-and-model-lock.md#personal-data-in-free-text"],
    ["S7", "0009-sdk-api.md#queries-and-indexes"],
  ]) {
    const line = lines.find((l) => new RegExp(`\\b${id}\\b`).test(l));
    assert.ok(line, `no decision followed names ${id}`);
    assert.match(line, OWNER, `${id}: no owner and date`);
    assert.ok(links(line, target), `${id}: no link to ${target}`);
  }
});

test('"History" lists each change by date, oldest first, with its pull request', () => {
  const history = section(read(RFC_0001), "history");
  assert.ok(history, 'no section "History"');
  const lines = history.split("\n").filter((l) => l.startsWith("- **"));
  const dates = lines.map(
    (l) => l.match(/^- \*\*(\d{4}-\d{2}-\d{2})\*\*/)?.[1],
  );
  assert.ok(dates.every(Boolean), "a History line has no date");
  assert.deepEqual([...dates].sort(), dates, "History is not oldest first");
  for (const line of lines.slice(1)) {
    assert.match(
      line,
      /\]\(https:\/\/github\.com\/nocentraldb\/nocentraldb\/pull\/\d+\)/,
      `no pull request: ${line.slice(0, 60)}`,
    );
  }
  for (const ticket of [
    "NOC-20",
    "NOC-44",
    "NOC-47",
    "NOC-34",
    "NOC-35",
    "NOC-76",
    "NOC-75",
    "NOC-100",
  ]) {
    assert.ok(
      lines.some((l) => l.includes(ticket)),
      `History does not name ${ticket}`,
    );
  }
});

test("the relay bullet links both lists of what the relay sees, and what it reads of control changes", () => {
  const relay = bullet("A relay that holds no keys.");
  assert.ok(
    links(
      relay,
      "0002-change-and-log-format.md#what-the-relay-can-see-and-what-it-cannot",
    ),
  );
  assert.ok(links(relay, "0007-relay.md#what-the-relay-sees"));
  assert.ok(links(relay, "0008-membership.md#how-the-relay-applies-the-rules"));
  assert.match(relay, /role/);
  assert.match(relay, /account/);
});

test("Shred and audit destroys the key once every admin device has seen the erasure", () => {
  assert.match(
    bullet("Shred and audit."),
    /once every admin device of the space has seen the erasure/,
  );
});

test("an evicted browser loses its member key and is paired again as a new device", () => {
  const evicted = bullet("Not a durable store in every browser.");
  assert.match(evicted, /member key/);
  assert.match(evicted, /new device/);
  assert.ok(links(evicted, "0003-keys-and-cryptography.md#the-account-key"));
  assert.ok(links(evicted, "../browser-durability.md"));
});

test("the paragraph after the example links RFC 0009's queries", () => {
  const rfc = read(RFC_0001);
  const line = rfc
    .split("\n")
    .find((l) => l.startsWith("Computed fields and views,"));
  assert.ok(
    line,
    "no paragraph on computed fields and views after the example",
  );
  assert.ok(links(line, "0009-sdk-api.md#queries-and-indexes"));
});
