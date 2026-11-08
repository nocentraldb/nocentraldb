// Asserts RFC 0004 (NOC-8), the merge rules for basic values, sets and links:
// the file exists, links back to RFC 0001, has the sections the ticket's
// acceptance criteria ask for, and every relative link in it resolves to a
// file. The RFC is a design document; this keeps the gate meaningful for it.
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, normalize } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const RFC = "docs/rfc/0004-merge-rules-basic-types.md";
const RFC_0001 = "docs/rfc/0001-nocentraldb.md";

const read = () => readFileSync(join(root, RFC), "utf8");

/** Relative Markdown link targets, without anchors; external links skipped. */
const relativeLinks = (markdown) =>
  [...markdown.matchAll(/\]\(([^)\s]+)\)/g)]
    .map((match) => match[1])
    .filter((href) => !/^[a-z]+:/i.test(href) && !href.startsWith("#"))
    .map((href) => href.split("#")[0]);

const headings = (markdown) =>
  markdown
    .split("\n")
    .filter((line) => /^#{2,3} /.test(line))
    .map((line) => line.replace(/^#+ /, "").trim().toLowerCase());

test("RFC 0004 exists", () => {
  assert.ok(existsSync(join(root, RFC)), `${RFC} is missing`);
});

test("RFC 0004 links back to RFC 0001", () => {
  const targets = relativeLinks(read()).map((href) =>
    normalize(join(dirname(RFC), href)),
  );
  assert.ok(
    targets.includes(normalize(RFC_0001)),
    `${RFC} has no relative link to ${RFC_0001}`,
  );
});

test("every relative link in RFC 0004 resolves to a file", () => {
  for (const href of relativeLinks(read())) {
    const target = join(root, dirname(RFC), href);
    assert.ok(existsSync(target), `${RFC} links to missing ${href}`);
  }
});

test("RFC 0004 has the sections its ticket asks for", () => {
  const found = headings(read());
  for (const wanted of [
    "basic values",
    "sets",
    "links",
    "the plugin contract",
    "rejected options",
    "decisions for the owner",
    "agreement with genesis.md and rfc 0001",
  ]) {
    assert.ok(
      found.some((heading) => heading.startsWith(wanted)),
      `${RFC} has no section starting with "${wanted}"`,
    );
  }
});

// Amendment 1 (NOC-37): how the field operations of basic values, sets and
// links travel in an RFC 0002 change. The test checks the amendment's shape;
// whether the encoding is right is the owner's review of the pull request.
const RFC_0002 = "docs/rfc/0002-change-and-log-format.md";

/** The text of the amendment: from its heading to the next level-2 heading. */
const amendment = () => {
  const [, rest = ""] = read().split(/^## Amendment 1\b.*$/m);
  return rest.split(/^## /m)[0];
};

test("RFC 0004 links RFC 0002", () => {
  const targets = relativeLinks(read()).map((href) =>
    normalize(join(dirname(RFC), href)),
  );
  assert.ok(
    targets.includes(normalize(RFC_0002)),
    `${RFC} has no relative link to ${RFC_0002}`,
  );
});

test("RFC 0004 has a dated changelog line for Amendment 1 (NOC-37)", () => {
  assert.match(
    read(),
    /^- \*\*\d{4}-\d{2}-\d{2}\*\* · NOC-37 · .*Amendment 1/m,
    `${RFC} has no changelog line "- **YYYY-MM-DD** · NOC-37 · ... Amendment 1"`,
  );
});

test("RFC 0004 has Amendment 1 with the sections NOC-37 asks for", () => {
  assert.ok(amendment().trim(), `${RFC} has no "## Amendment 1" section`);
  const found = headings(`## x\n${amendment()}`);
  for (const wanted of [
    "where field operations sit in a change",
    "operation numbers",
    "the arguments of each operation",
    "operations a member does not understand",
    "how a field type adds operations",
    "byte-exact examples",
    "what the relay sees",
    "rejected options",
    "decisions for the owner",
  ]) {
    assert.ok(
      found.some((heading) => heading.startsWith(wanted)),
      `${RFC}: Amendment 1 has no section starting with "${wanted}"`,
    );
  }
});

test("Amendment 1 covers basic values, sets, links and deletion, and points NOC-16 and NOC-17 at its examples", () => {
  const text = amendment();
  for (const operation of ["set", "add", "remove", "delete"]) {
    assert.match(
      text,
      new RegExp(`\\*\\*${operation}\\*\\*`),
      `${RFC}: Amendment 1 does not define the operation **${operation}**`,
    );
  }
  for (const ticket of ["NOC-16", "NOC-17"]) {
    assert.ok(
      text.includes(ticket),
      `${RFC}: Amendment 1 does not name ${ticket}`,
    );
  }
  const examples = text.split(/^### Byte-exact examples/m)[1] ?? "";
  assert.match(
    examples,
    /```text\n(?:[0-9a-f]{2}\s?)+\n```/,
    `${RFC}: the byte-exact examples hold no hex block`,
  );
});
