// Asserts RFC 0003, keys and cryptography (NOC-7): the file exists, links to
// RFC 0001, has a section for every topic the ticket asks it to define, and
// every relative link in it resolves to a file. The test checks the shape of
// the design document, not whether the design is right; that is the owner's
// review of the pull request.
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, normalize } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const RFC = "docs/rfc/0003-keys-and-cryptography.md";
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

test("RFC 0003 exists", () => {
  assert.ok(existsSync(path), `${RFC} is missing`);
});

test("RFC 0003 links to RFC 0001", () => {
  assert.ok(
    relativeLinks(read()).includes("0001-nocentraldb.md"),
    "no relative link to 0001-nocentraldb.md",
  );
});

test("every relative link in RFC 0003 resolves to a file", () => {
  for (const target of relativeLinks(read())) {
    const resolved = normalize(join(dirname(path), target));
    assert.ok(existsSync(resolved), `broken link: ${target}`);
  }
});

test("RFC 0003 has a section for every topic of NOC-7", () => {
  const found = headings(read());
  const required = [
    "The keys",
    "Primitives",
    "Sharing a space key with a new member",
    "Removing a member and rotating the space key",
    "Sub-agents get an ad-hoc space",
    "Shredding a person's key",
    "How the SDK hides keys",
    "Keys never reach logs, errors or telemetry",
    "Group key distribution: options compared",
    "Decisions",
    "Agreement with GENESIS.md and RFC 0001",
  ];
  for (const heading of required) {
    assert.ok(found.includes(heading), `missing section: ## ${heading}`);
  }
});

test("RFC 0003 defines the four kinds of key", () => {
  const text = read();
  for (const key of ["member key", "account key", "space key", "person key"]) {
    assert.match(text, new RegExp(`### .*${key}`, "i"), `no section: ${key}`);
  }
});

test("RFC 0003 says a removed member keeps its copy", () => {
  assert.match(read(), /removed member keeps/i);
});
