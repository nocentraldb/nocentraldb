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

// The owner's review (Carsten, 2026-11-08): every decision is recorded where
// it is asked, the proposed reading of GENESIS.md is accepted, personal data
// in free text gets its own section, and a CRM example shows a person who is
// not a member.
const DECIDED = "Decided (Carsten, 2026-11-08)";

/** The text of each owner decision, from its "**Dn." line to the next one. */
const decisions = (markdown) => {
  const body = section(markdown, "Decisions for the owner");
  return [
    ...body.matchAll(/^\*\*(D\d+)\.[\s\S]*?(?=^\*\*D\d+\.|(?![\s\S]))/gm),
  ].map((match) => ({ id: match[1], text: match[0] }));
};

test("every owner decision in RFC 0005 records the owner's decision", () => {
  const found = decisions(read());
  assert.ok(found.length >= 14, `${RFC} lists ${found.length} decisions`);
  for (const { id, text } of found) {
    assert.ok(
      text.includes(DECIDED),
      `${RFC}: ${id} does not say "${DECIDED}"`,
    );
  }
});

test("the proposed reading of GENESIS.md in RFC 0005 is marked accepted", () => {
  const agreement = section(read(), "Agreement with GENESIS.md");
  const reading = agreement
    .split("\n")
    .find((line) => line.startsWith("A point for the owner"));
  assert.ok(reading, `${RFC} has no proposed reading of GENESIS.md`);
  assert.ok(
    reading.includes("Accepted (Carsten, 2026-11-08)"),
    `${RFC}: the proposed reading of GENESIS.md is not marked accepted`,
  );
});

test("RFC 0005 has a section on personal data in free text", () => {
  const found = headingLines(read()).map((heading) => heading.toLowerCase());
  assert.ok(
    found.some((heading) => heading.startsWith("personal data in free text")),
    `${RFC} has no section starting with "Personal data in free text"`,
  );
});

test("RFC 0005 shows a customer who is a person but not a member", () => {
  const examples = codeBlocks(read(), "ts").join("\n");
  assert.match(
    examples,
    /customers: nc\.collection\(\{[\s\S]*?\.personal\(\{ person: "self" \}\)/,
    `${RFC} has no "customers" collection with personal fields about the customer`,
  );
});

test("the task board's schema and its lock example name the same fields", () => {
  const markdown = read();
  const schema = codeBlocks(markdown, "ts")[0];
  const inSchema = new Set(
    [
      ...schema.matchAll(
        /^\s+(\w+): nc\.(?!index|reach|view|collection)\w+\(/gm,
      ),
    ].map((match) => match[1]),
  );
  inSchema.delete("body"); // rich text: left out of the lock example on purpose
  const lock = JSON.parse(codeBlocks(markdown, "json")[0]);
  const inLock = new Set(
    Object.values(lock.collections).flatMap((collection) =>
      Object.keys(collection.fields),
    ),
  );
  assert.deepEqual([...inSchema].sort(), [...inLock].sort());
});

// NOC-54: RFC 0002 Amendment 5 put the writer's model.id and model version
// in payload key 64, so two sentences of RFC 0005 said less than the design.
// RFC 0005 records the change as an amendment of its own, in the style of
// RFC 0001's amendments: a dated changelog line at the top, a section that
// decides nothing and links RFC 0002 Amendment 5, and the changed sentences
// marked.
const AMENDMENT = "amendment-1-following-rfc-0002-amendment-5";
const RFC_0002_AMENDMENT =
  "0002-change-and-log-format.md#amendment-5-the-writers-model-in-the-payload";

/** The link targets of one line of Markdown. */
const targets = (text) =>
  [...text.matchAll(/\]\(([^)\s]+)\)/g)].map((match) => match[1]);

/** The body of the level-2 section whose GitHub anchor is `anchor`. */
const sectionByAnchor = (markdown, anchor) =>
  markdown
    .split(/^(?=## )/m)
    .find((part) => slug(part.split("\n")[0].replace(/^## /, "")) === anchor);

test("RFC 0005 announces Amendment 1 in a dated NOC-54 changelog line that links RFC 0002 Amendment 5", () => {
  const top = read().split(/^## /m)[0];
  assert.match(top, /^\*\*Changelog\.\*\*$/m, "RFC 0005 has no changelog");
  const line = top
    .split("\n")
    .find((l) => /^- \*\*\d{4}-\d{2}-\d{2}\*\* · NOC-54 · /.test(l));
  assert.ok(line, "no dated changelog line for NOC-54 above the first section");
  assert.ok(
    targets(line).includes(`#${AMENDMENT}`),
    `the changelog line does not link #${AMENDMENT}`,
  );
  assert.ok(
    targets(line).includes(RFC_0002_AMENDMENT),
    "the changelog line does not link RFC 0002 Amendment 5",
  );
});

test("RFC 0005 has the section of Amendment 1: it decides nothing and links RFC 0002 Amendment 5", () => {
  const body = sectionByAnchor(read(), AMENDMENT);
  assert.ok(body, `no section #${AMENDMENT}`);
  assert.match(body, /NOC-54/);
  assert.match(body, /decides nothing|adds no decision/i);
  assert.ok(
    targets(body).includes(RFC_0002_AMENDMENT),
    "the amendment does not link RFC 0002 Amendment 5",
  );
});

test("RFC 0005 no longer says a change does not name its model, and marks what replaced it", () => {
  const rfc = read();
  assert.doesNotMatch(rfc, /Nothing in a change says which model wrote it/);
  assert.doesNotMatch(rfc, /Until it has one, the SDK falls back to B/);
  const newer = section(rfc, "Data from a newer model");
  const model = newer
    .split("\n")
    .find((l) => l.startsWith("- ") && /model\.id/.test(l));
  assert.match(
    model ?? "",
    /\*\(Amendment 1\.\)\*$/,
    "the bullet on model.id is not marked",
  );
  const d10 = section(rfc, "Decisions for the owner")
    .split(/^(?=\*\*D\d+\.)/m)
    .find((d) => d.startsWith("**D10."));
  const recommendation = d10
    .split("\n")
    .find((l) => l.startsWith("Recommendation:"));
  assert.match(recommendation, /payload key 64/);
  assert.match(
    recommendation,
    /\*\(Amendment 1\.\)\*/,
    "D10's changed sentence is not marked",
  );
});

test("point 6 of Data from a newer model counts changes of another model apart, marked", () => {
  const point = section(read(), "Data from a newer model")
    .split("\n")
    .find((l) => l.startsWith("6. "));
  assert.ok(point, "Data from a newer model has no point 6");
  assert.match(
    point,
    /another model/,
    'point 6 does not name the reason "another model"',
  );
  assert.match(
    point,
    /apart/,
    "point 6 does not count changes of another model apart",
  );
  assert.match(
    point,
    /\*\(Amendment 1\.\)\*$/,
    "point 6's change is not marked",
  );
  const body = sectionByAnchor(read(), AMENDMENT);
  assert.match(body, /point 6/, "the amendment does not cover point 6");
});
