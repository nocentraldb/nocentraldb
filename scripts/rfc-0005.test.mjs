// Asserts RFC 0005 (NOC-11), the schema language and model.lock.json: the
// file exists, links to RFC 0001's "The design in one page", every relative
// link resolves, it has the sections its ticket asks for, every schema
// function and CLI command its examples use is listed for the owner under
// "Public names", and every JSON example parses. The RFC is a design
// document; this keeps the gate meaningful for it.
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, normalize } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const RFC = "docs/rfc/0005-schema-and-model-lock.md";
const RFC_0001 = "docs/rfc/0001-nocentraldb.md";

const read = (path = RFC) => readFileSync(join(root, path), "utf8");

/** Relative Markdown links as [file, anchor]; external and in-page skipped. */
const relativeLinks = (markdown) =>
  [...markdown.matchAll(/\]\(([^)\s]+)\)/g)]
    .map((match) => match[1])
    .filter((href) => !/^[a-z]+:/i.test(href) && !href.startsWith("#"))
    .map((href) => href.split("#"));

/** GitHub's heading slug: lower case, punctuation dropped, spaces to dashes. */
const slug = (heading) =>
  heading
    .trim()
    .toLowerCase()
    .replace(/[^\p{L}\p{N} _-]/gu, "")
    .replace(/ /g, "-");

const headingLines = (markdown) =>
  markdown
    .split("\n")
    .filter((line) => /^#{1,6} /.test(line))
    .map((line) => line.replace(/^#+ /, ""));

/** The body of one level-2 section, up to the next level-2 heading. */
const section = (markdown, title) => {
  const start = markdown.search(new RegExp(`^## ${title}`, "im"));
  if (start < 0) return "";
  const rest = markdown.slice(start + 1);
  const end = rest.search(/^## /m);
  return end < 0 ? rest : rest.slice(0, end);
};

const codeBlocks = (markdown, language) =>
  [...markdown.matchAll(/^```(\w*)\n([\s\S]*?)^```$/gm)]
    .filter((match) => match[1] === language)
    .map((match) => match[2]);

test("RFC 0005 exists", () => {
  assert.ok(existsSync(join(root, RFC)), `${RFC} is missing`);
});

test("RFC 0005 links to RFC 0001's design in one page", () => {
  const targets = relativeLinks(read()).map(([file, anchor]) => [
    normalize(join(dirname(RFC), file)),
    anchor,
  ]);
  assert.ok(
    targets.some(
      ([file, anchor]) =>
        file === normalize(RFC_0001) && anchor === "the-design-in-one-page",
    ),
    `${RFC} has no link to ${RFC_0001}#the-design-in-one-page`,
  );
});

test("every relative link in RFC 0005 resolves, anchors included", () => {
  for (const [file, anchor] of relativeLinks(read())) {
    const target = normalize(join(dirname(RFC), file));
    assert.ok(
      existsSync(join(root, target)),
      `${RFC} links to missing ${file}`,
    );
    if (anchor && target.endsWith(".md")) {
      const slugs = headingLines(read(target)).map(slug);
      assert.ok(
        slugs.includes(anchor),
        `${RFC} links to ${file}#${anchor}, which has no such heading`,
      );
    }
  }
});

test("RFC 0005 has the sections its ticket asks for", () => {
  const found = headingLines(read()).map((heading) =>
    heading.replace(/`/g, "").trim().toLowerCase(),
  );
  for (const wanted of [
    "the schema in typescript",
    "the structure of model.lock.json",
    "indexes",
    "the additive-change rule",
    "data from a newer model",
    "personal fields",
    "where this differs from rfc 0001",
    "public names",
    "rejected options",
    "decisions for the owner",
  ]) {
    assert.ok(
      found.some((heading) => heading.startsWith(wanted)),
      `${RFC} has no section starting with "${wanted}"`,
    );
  }
});

test("every name the examples use is listed under Public names", () => {
  const markdown = read();
  const listed = section(markdown, "Public names");
  assert.ok(listed, `${RFC} has no "## Public names" section`);
  // Each name as it must appear, in backticks, in the Public names section:
  // `nc.string`, a modifier as `.personal(`, a command as `ncdb model build`.
  const used = new Set();
  for (const block of codeBlocks(markdown, "ts")) {
    for (const match of block.matchAll(/\bnc\.([A-Za-z]+)/g)) {
      used.add(`nc.${match[1]}`);
    }
    for (const match of block.matchAll(/\)\s*\.([a-z][A-Za-z]*)\(/g)) {
      used.add(`.${match[1]}(`);
    }
  }
  for (const block of codeBlocks(markdown, "sh")) {
    for (const match of block.matchAll(/\bncdb model ([a-z]+)/g)) {
      used.add(`ncdb model ${match[1]}`);
    }
  }
  assert.ok(used.size > 0, `${RFC} has no TypeScript examples`);
  for (const name of used) {
    assert.ok(
      listed.includes(`\`${name}`),
      `${RFC} uses ${name} in an example but does not list it under "Public names"`,
    );
  }
});

test("every JSON example in RFC 0005 parses", () => {
  const blocks = codeBlocks(read(), "json");
  assert.ok(blocks.length > 0, `${RFC} has no JSON example`);
  for (const block of blocks) {
    assert.doesNotThrow(() => JSON.parse(block), `invalid JSON:\n${block}`);
  }
});
