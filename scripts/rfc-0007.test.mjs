// Asserts RFC 0007, the relay protocol and its AWS backend (NOC-13): the file
// exists, links to RFC 0001, has a section for every topic the ticket asks it
// to define, and every relative link in it resolves to a file. The test checks
// the shape of the design document, not whether the design is right; that is
// the owner's review of the pull request.
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, normalize } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const RFC = "docs/rfc/0007-relay.md";
const path = join(root, RFC);

const read = () => readFileSync(path, "utf8");

/** The level-2 headings of the RFC, in order. */
const headings = (text) =>
  [...text.matchAll(/^## (.+)$/gm)].map((match) => match[1].trim());

/** The relative link targets of the RFC, without anchors. */
const relativeLinks = (text) =>
  [...text.matchAll(/\]\(([^)\s]+)\)/g)]
    .map((match) => match[1])
    .filter((target) => !/^[a-z]+:/i.test(target) && !target.startsWith("#"))
    .map((target) => target.split("#")[0]);

test("RFC 0007 exists", () => {
  assert.ok(existsSync(path), `${RFC} is missing`);
});

test("RFC 0007 links to RFC 0001", () => {
  assert.ok(
    relativeLinks(read()).includes("0001-nocentraldb.md"),
    "no relative link to 0001-nocentraldb.md",
  );
});

test("every relative link in RFC 0007 resolves to a file", () => {
  for (const target of relativeLinks(read())) {
    const resolved = normalize(join(dirname(path), target));
    assert.ok(existsSync(resolved), `broken link: ${target}`);
  }
});

test("RFC 0007 has a section for every topic of NOC-13", () => {
  const found = headings(read());
  const required = [
    "The protocol",
    "What the relay checks",
    "What the relay sees",
    "The backend interface",
    "The AWS backend",
    "What it costs",
    "Island mode",
    "History",
  ];
  for (const heading of required) {
    assert.ok(found.includes(heading), `missing section: ## ${heading}`);
  }
});
