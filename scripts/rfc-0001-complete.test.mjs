// Holds RFC 0001 as one complete text (NOC-100). The owner wants every RFC to
// state the current design directly (Carsten, 2026-11-10): the rules its six
// amendments brought in stand where they belong, no amendment section, mark or
// changelog is left. Its form as a fresh text, with groups as a section of
// the design and one History line, is held by rfc-0001-fresh.test.mjs
// (NOC-165). The rules of the
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

test("Shred and audit destroys the key as soon as a member accepts the erasure", () => {
  // RFC 0008 as merged in #220: a shred is final when accepted (NOC-224).
  const shred = bullet("Shred and audit.");
  assert.match(shred, /as soon as it accepts the erasure/);
  assert.ok(links(shred, "0008-membership.md#a-shred-is-final-when-accepted"));
  assert.doesNotMatch(shred, /every admin device/);
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
