// Asserts RFC 0006 (NOC-12), the local view on SQLite: the file exists, links
// back to RFC 0001, has the sections the ticket's acceptance criteria ask for,
// records the spike's result, and every relative link in it resolves to a
// file. The RFC is a design document; this keeps the gate meaningful for it.
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, normalize } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const RFC = "docs/rfc/0006-local-view.md";
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

test("RFC 0006 exists", () => {
  assert.ok(existsSync(join(root, RFC)), `${RFC} is missing`);
});

test("RFC 0006 links back to RFC 0001", () => {
  const targets = relativeLinks(read()).map((href) =>
    normalize(join(dirname(RFC), href)),
  );
  assert.ok(
    targets.includes(normalize(RFC_0001)),
    `${RFC} has no relative link to ${RFC_0001}`,
  );
});

test("every relative link in RFC 0006 resolves to a file", () => {
  for (const href of relativeLinks(read())) {
    const target = join(root, dirname(RFC), href);
    assert.ok(existsSync(target), `${RFC} links to missing ${href}`);
  }
});

test("RFC 0006 has the sections its ticket asks for", () => {
  const found = headings(read());
  for (const wanted of [
    "folding",
    "control changes and changes that wait for them",
    "rebuilding the view",
    "declared indexes",
    "ordered pages",
    "sqlite on each target",
    "what the view stores in plaintext",
    "reads never touch the network",
    "the spike",
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

test("RFC 0006 records the spike's result", () => {
  const spike = read().split(/^## The spike/m)[1] ?? "";
  assert.match(
    spike,
    /\*\*Result:\s*(works|does not work)/i,
    `${RFC}: the spike section states no result ("**Result: works" or "**Result: does not work")`,
  );
});
