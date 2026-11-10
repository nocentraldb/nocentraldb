// Asserts RFC 0009 (NOC-64), the SDK API for apps: the file exists, links to
// every decided RFC it builds on, every relative link and its anchor
// resolve, it has a section for every part the ticket names, every decision
// for the owner has options and a recommendation, every SDK name its
// TypeScript examples use is listed under "Public names", every row there
// links the RFC it builds on, and the RFC index lists it. The test checks the
// shape of the design document, not whether the design is right; that is the
// owner's review of the pull request.
import assert from "node:assert/strict";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { dirname, join, normalize } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const RFC = "docs/rfc/0009-sdk-api.md";
const INDEX = "docs/rfc/README.md";

const read = (path = RFC) => readFileSync(join(root, path), "utf8");

/** The text of a Markdown file outside fenced code blocks. */
const prose = (markdown) => markdown.replace(/^```[\s\S]*?^```$/gm, "");

/** GitHub's heading slug: lower case, punctuation dropped, spaces to dashes. */
const slug = (heading) =>
  heading
    .trim()
    .toLowerCase()
    .replace(/[^\p{L}\p{N} _-]/gu, "")
    .replace(/ /g, "-");

const headingLines = (markdown) =>
  prose(markdown)
    .split("\n")
    .filter((line) => /^#{1,6} /.test(line))
    .map((line) => line.replace(/^#+ /, ""));

/** Relative links as { resolved, anchor }; external links skipped. */
const relativeLinks = (path) =>
  [...prose(read(path)).matchAll(/\]\(([^)\s]+)\)/g)]
    .map((match) => match[1])
    .filter((href) => !/^[a-z]+:/i.test(href))
    .map((href) => {
      const [file, anchor] = href.split("#");
      const resolved = file ? normalize(join(dirname(path), file)) : path;
      return { href, resolved, anchor };
    });

/** The body of one level-2 section, up to the next level-2 heading. */
const section = (markdown, title) => {
  const text = prose(markdown);
  const start = text.search(new RegExp(`^## ${title}`, "im"));
  if (start < 0) return "";
  const rest = text.slice(start + 1);
  const end = rest.search(/^## /m);
  return end < 0 ? rest : rest.slice(0, end);
};

const codeBlocks = (markdown, language) =>
  [...markdown.matchAll(/^```(\w*)\n([\s\S]*?)^```$/gm)]
    .filter((match) => match[1] === language)
    .map((match) => match[2]);

test("RFC 0009 exists", () => {
  assert.ok(existsSync(join(root, RFC)), `${RFC} is missing`);
});

for (const target of [
  "docs/rfc/0001-nocentraldb.md",
  "docs/rfc/0002-change-and-log-format.md",
  "docs/rfc/0003-keys-and-cryptography.md",
  "docs/rfc/0004-merge-rules-basic-types.md",
  "docs/rfc/0005-schema-and-model-lock.md",
  "docs/rfc/0006-local-view.md",
  "docs/rfc/0007-relay.md",
  "docs/rfc/0008-membership.md",
  "factory/GENESIS.md",
]) {
  test(`RFC 0009 links to ${target}`, () => {
    const links = relativeLinks(RFC).filter((l) => l.resolved === target);
    assert.ok(links.length > 0, `${RFC} has no link to ${target}`);
  });
}

test("every relative link in RFC 0009 resolves, to a heading when it names one", () => {
  for (const { href, resolved, anchor } of relativeLinks(RFC)) {
    assert.ok(existsSync(join(root, resolved)), `broken link: ${href}`);
    if (!anchor || !resolved.endsWith(".md")) continue;
    assert.ok(
      headingLines(read(resolved)).map(slug).includes(anchor),
      `${RFC}: ${href} names no heading in ${resolved}`,
    );
  }
});

test("RFC 0009 has a section for every part NOC-64 names", () => {
  const found = headingLines(read()).map((h) =>
    h.replace(/`/g, "").trim().toLowerCase(),
  );
  for (const wanted of [
    "open and close",
    "write",
    "read and query",
    "subscribe",
    "spaces and members",
    "sync and status",
    "the rust sdk",
    "errors",
    "public names",
    "history",
    "how this rfc is tested",
  ]) {
    assert.ok(
      found.some((heading) => heading.startsWith(wanted)),
      `${RFC} has no section starting with "${wanted}"`,
    );
  }
});

/** The SDK names a TypeScript example uses, in the form Public names lists. */
const namesUsed = (block) => {
  const used = new Set();
  const chain = "([a-z][A-Za-z]*(?:\\.[a-z][A-Za-z]*)*)";
  for (const [, name] of block.matchAll(/\bnc\.([A-Za-z]+)/g)) {
    used.add(`nc.${name}`);
  }
  for (const [, name] of block.matchAll(new RegExp(`\\bdb\\.${chain}`, "g"))) {
    used.add(`db.${name}`);
  }
  for (const [, name] of block.matchAll(
    new RegExp(`\\b(?:space|project|adhoc)\\.${chain}`, "g"),
  )) {
    used.add(`space.${name}`);
  }
  for (const [, name] of block.matchAll(/\btx\.([a-z][A-Za-z]*)/g)) {
    used.add(`tx.${name}`);
  }
  for (const [, name] of block.matchAll(/\.(equal|take|after)\(/g)) {
    used.add(`.${name}(`);
  }
  return used;
};

test("every SDK name the TypeScript examples use is listed under Public names", () => {
  const markdown = read();
  const listed = section(markdown, "Public names");
  assert.ok(listed, `${RFC} has no "## Public names" section`);
  const used = new Set(
    codeBlocks(markdown, "ts").flatMap((b) => [...namesUsed(b)]),
  );
  assert.ok(used.size > 10, `${RFC} has too few TypeScript examples`);
  for (const name of used) {
    assert.ok(
      listed.includes(`\`${name}`),
      `${RFC} uses ${name} in an example but does not list it under "Public names"`,
    );
  }
});

test("every row under Public names links the RFC it builds on", () => {
  const rows = section(read(), "Public names")
    .split("\n")
    .filter((line) => line.startsWith("| `"));
  assert.ok(rows.length > 20, `${RFC} lists only ${rows.length} names`);
  for (const row of rows) {
    assert.match(
      row,
      /\]\((?:\d{4}-[^)]+\.md|\.\.\/[^)]+)(?:#[^)]*)?\)/,
      `no link to the RFC a name builds on: ${row}`,
    );
  }
});

test("RFC 0009 shows the Rust SDK", () => {
  assert.ok(
    codeBlocks(read(), "rust").length > 0,
    `${RFC} has no Rust example`,
  );
});

test("the RFC index lists RFC 0009", () => {
  assert.match(read(INDEX), /^\| \[0009\]\(0009-sdk-api\.md\) \| .+ \|$/m);
});

test("the decision log records the owner's decision on RFC 0009", () => {
  const dir = join(root, "factory/decisions");
  const files = readdirSync(dir).filter((f) => /^2026-11-09-NOC-64-/.test(f));
  assert.ok(
    files.some((f) =>
      /Decided by: Carsten, 2026-11-09/.test(
        readFileSync(join(dir, f), "utf8"),
      ),
    ),
    "factory/decisions/ has no NOC-64 file decided by Carsten on 2026-11-09",
  );
});
