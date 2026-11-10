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
    "agreement with genesis.md and rfc 0001",
  ]) {
    assert.ok(
      found.some((heading) => heading.startsWith(wanted)),
      `${RFC} has no section starting with "${wanted}"`,
    );
  }
});

// RFC 0004 builds on RFC 0002's operation map (NOC-37); the rules of its
// former amendments are held by rfc-0004-complete.test.mjs (NOC-100).
const RFC_0002 = "docs/rfc/0002-change-and-log-format.md";

test("RFC 0004 links RFC 0002", () => {
  const targets = relativeLinks(read()).map((href) =>
    normalize(join(dirname(RFC), href)),
  );
  assert.ok(
    targets.includes(normalize(RFC_0002)),
    `${RFC} has no relative link to ${RFC_0002}`,
  );
});
