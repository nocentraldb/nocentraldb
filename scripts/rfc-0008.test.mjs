// Asserts RFC 0008, membership, roles and the first change of a space
// (NOC-23): the file exists, links to RFC 0001 and RFC 0002 with relative
// links that resolve (to a heading, when a link names one), and has a section
// for every topic the ticket asks it to define. The test checks the shape of
// the design document, not whether the design is right; that is the owner's
// review of the pull request.
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, normalize } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (path) => readFileSync(join(root, path), "utf8");

const RFC_0001 = "docs/rfc/0001-nocentraldb.md";
const RFC_0002 = "docs/rfc/0002-change-and-log-format.md";
const RFC = "docs/rfc/0008-membership.md";

/** GitHub's heading anchor: lower case, punctuation dropped, spaces to dashes. */
const slug = (heading) =>
  heading
    .trim()
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s_-]/gu, "")
    .replace(/\s/g, "-");
const anchors = (markdown) =>
  new Set(
    markdown
      .split("\n")
      .filter((line) => /^#{1,6} /.test(line))
      .map((line) => slug(line.replace(/^#+ /, ""))),
  );

/** The text of a Markdown file outside fenced code blocks. */
const prose = (path) => read(path).replace(/```[\s\S]*?```/g, "");

/** The relative links of a Markdown file outside code blocks, resolved. */
const relativeLinks = (path) =>
  [...prose(path).matchAll(/\]\(([^)\s]+)\)/g)]
    .map(([, target]) => target)
    .filter((target) => !/^[a-z]+:/.test(target))
    .map((target) => {
      const [file, anchor] = target.split("#");
      const resolved = file ? normalize(join(dirname(path), file)) : path;
      return { target, resolved, anchor };
    });

/** The level-2 headings of the RFC, in order. */
const headings = (path) =>
  [...prose(path).matchAll(/^## (.+)$/gm)].map((match) => match[1].trim());

test("RFC 0008 exists", () => {
  assert.ok(existsSync(join(root, RFC)), `${RFC} is missing`);
});

for (const target of [RFC_0001, RFC_0002]) {
  test(`RFC 0008 links to ${target}`, () => {
    const links = relativeLinks(RFC).filter((l) => l.resolved === target);
    assert.ok(links.length > 0, `${RFC} has no link to ${target}`);
  });
}

test("every relative link in RFC 0008 resolves, to a heading when it names one", () => {
  for (const { target, resolved, anchor } of relativeLinks(RFC)) {
    assert.ok(existsSync(join(root, resolved)), `broken link: ${target}`);
    if (!anchor || !resolved.endsWith(".md")) continue;
    assert.ok(
      anchors(read(resolved)).has(anchor),
      `${RFC}: ${target} names no heading in ${resolved}`,
    );
  }
});

test("RFC 0008 has a section for every topic of NOC-23", () => {
  const found = headings(RFC);
  const required = [
    "The first change of a space",
    "Roles and rights",
    "The control bodies",
    "Folding the membership state",
    "How the convergence harness checks it",
    "How the relay applies the rules",
    "Groups",
    "Rejected options",
    "Decisions for the owner",
  ];
  for (const heading of required) {
    assert.ok(found.includes(heading), `missing section: ## ${heading}`);
  }
});

test("RFC 0008 defines the body of every control change RFC 0003 names", () => {
  const text = prose(RFC);
  for (const body of [
    "membership entry",
    "key envelope",
    "key chain entry",
    "person-key entry",
    "shred entry",
    "acknowledgement",
  ]) {
    assert.match(
      text,
      new RegExp(`^### .*${body}`, "im"),
      `no section: ${body}`,
    );
  }
});

test("RFC 0008 folds the three concurrency cases the ticket names", () => {
  const text = prose(RFC);
  for (const heading of [
    /^### .*concurrent add.* remove/im,
    /^### .*two removals/im,
    /^### .*removed while offline/im,
  ]) {
    assert.match(text, heading, `no section matching ${heading}`);
  }
});
