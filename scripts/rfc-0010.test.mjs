// Asserts the structure of RFC 0010 (NOC-107), accounts, as the owner decided
// it (NOC-115): the file exists and the RFC index lists it; its header and
// status say it is decided, by the owner, with the date; it links to
// GENESIS.md and to every decided RFC it builds on, and every relative link
// and its anchor resolve; it has a section for every part the ticket names
// (the account key and where it lives, the recovery code, adding a device,
// linking per-database accounts, and what a member certificate means for
// RFC 0008); no question is left open and no recommendation or draft wording
// is left in the design; "Decisions" holds Q1 to Q12 in short form, each cited
// where it applies; the decision log records the owner's decision on the RFC
// as a whole; and "History" names both steps. The test checks the shape of
// the design document, not whether the design is right; that is the owner's
// review of the pull request.
import assert from "node:assert/strict";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { dirname, join, normalize } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const RFC = "docs/rfc/0010-accounts.md";
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

/** Relative links as { href, resolved, anchor }; external links skipped. */
const relativeLinks = (path) =>
  [...prose(read(path)).matchAll(/\]\(([^)\s]+)\)/g)]
    .map((match) => match[1])
    .filter((href) => !/^[a-z]+:/i.test(href))
    .map((href) => {
      const [file, anchor] = href.split("#");
      const resolved = file ? normalize(join(dirname(path), file)) : path;
      return { href, resolved, anchor };
    });

/** The body of the level-2 section `title`, up to the next level-2 heading. */
const section = (markdown, title) => {
  const text = prose(markdown);
  const [, rest] = text.split(`\n## ${title}\n`);
  assert.ok(rest !== undefined, `${RFC} has no "## ${title}" section`);
  return rest.split(/^## /m)[0];
};

/** The first line of `markdown` that starts with `start`. */
const line = (markdown, start) =>
  markdown.split("\n").find((l) => l.startsWith(start)) ?? "";

test("RFC 0010 exists and the RFC index lists it", () => {
  assert.ok(existsSync(join(root, RFC)), `${RFC} is missing`);
  assert.match(read(INDEX), /^\| \[0010\]\(0010-accounts\.md\) \| .+ \|$/m);
  const row = read(INDEX)
    .split("\n")
    .find((l) => l.startsWith("| [0010]"));
  assert.doesNotMatch(row, /draft/i, "the index still calls it a draft");
});

test("RFC 0010 says it is decided, by the owner, with the date", () => {
  const rfc = read();
  assert.match(rfc.split("\n")[0], /^# RFC 0010: Accounts/);
  assert.doesNotMatch(rfc.split("\n")[2], /draft/i, "the header names a draft");
  const status = line(rfc, "**Status.**");
  assert.match(status, /^\*\*Status\.\*\* Decided\. /);
  assert.match(status, /\(Carsten, 2026-11-10\)/);
  assert.match(status, /NOC-115/, "the status names where the owner decided");
});

test("the design reads as decided: no recommendation or draft wording outside History", () => {
  const text = prose(read()).split("\n## History\n")[0];
  assert.doesNotMatch(text, /\*\(recommended\)\*/);
  assert.doesNotMatch(text, /\brecommend/i, "a recommendation is left");
  assert.doesNotMatch(text, /\bdraft\b/i, "draft wording is left");
  assert.doesNotMatch(text, /#open-questions\)/, "a link to an open question");
});

test("RFC 0010 says how to give feedback", () => {
  const feedback = line(read(), "**How to give feedback.**");
  assert.ok(feedback, `${RFC} has no "How to give feedback" line`);
  assert.match(feedback, /\(README\.md#giving-feedback\)/);
  assert.ok(
    feedback.includes(
      "(https://github.com/nocentraldb/nocentraldb/discussions/categories/rfc-feedback)",
    ),
    "the line links the RFC feedback category",
  );
});

for (const target of [
  "docs/rfc/0001-nocentraldb.md",
  "docs/rfc/0002-change-and-log-format.md",
  "docs/rfc/0003-keys-and-cryptography.md",
  "docs/rfc/0006-local-view.md",
  "docs/rfc/0007-relay.md",
  "docs/rfc/0008-membership.md",
  "docs/rfc/0009-sdk-api.md",
  "factory/GENESIS.md",
]) {
  test(`RFC 0010 links to ${target}`, () => {
    const links = relativeLinks(RFC).filter((l) => l.resolved === target);
    assert.ok(links.length > 0, `${RFC} has no link to ${target}`);
  });
}

test("every relative link in RFC 0010 resolves, to a heading when it names one", () => {
  for (const { href, resolved, anchor } of relativeLinks(RFC)) {
    assert.ok(existsSync(join(root, resolved)), `broken link: ${href}`);
    if (!anchor || !resolved.endsWith(".md")) continue;
    assert.ok(
      headingLines(read(resolved)).map(slug).includes(anchor),
      `${RFC}: ${href} names no heading in ${resolved}`,
    );
  }
});

test("RFC 0010 has a section for every part NOC-107 names", () => {
  const found = headingLines(read()).map((h) => h.trim().toLowerCase());
  for (const wanted of [
    "summary",
    "terms",
    "the account key and where it lives",
    "what a member certificate means for rfc 0008",
    "the recovery code",
    "adding a device or an agent to an account",
    "linking accounts drawn per database",
    "losing a device",
    "what the relay learns",
    "what this does not do",
    "rejected options",
    "agreement with genesis.md and the decided rfcs",
    "open questions",
    "decisions",
    "history",
    "how this rfc is tested",
  ]) {
    assert.ok(
      found.includes(wanted),
      `${RFC} has no section headed "${wanted}"`,
    );
  }
});

test("no question is left open", () => {
  assert.match(section(read(), "Open questions"), /^None\./m);
});

/** The decisions under "## Decisions", one "### Qn. Title" heading each. */
const decisions = () =>
  [
    ...section(read(), "Decisions").matchAll(
      /^### (Q\d+)\. ([^\n]+)\n([\s\S]*?)(?=^### |(?![\s\S]))/gm,
    ),
  ].map((m) => ({ id: m[1], heading: `${m[1]}. ${m[2]}`, text: m[3] }));

test("Decisions holds Q1 to Q12 in short form, as the owner decided them", () => {
  const found = decisions();
  assert.deepEqual(
    found.map((d) => d.id),
    Array.from({ length: 12 }, (_, i) => `Q${i + 1}`),
  );
  for (const { id, text } of found) {
    assert.match(
      text,
      /^Decided \(Carsten, 2026-11-10\), on NOC-115: /m,
      `${id} names no decision by the owner`,
    );
    assert.match(text, /^Why: /m, `${id} says not why`);
    assert.match(text, /^Not taken:\n\n- /m, `${id} lists no option not taken`);
  }
});

test("every decision is cited by its own heading where it applies", () => {
  const body = prose(read()).split("\n## Decisions\n")[0];
  for (const { id, heading } of decisions()) {
    assert.ok(
      body.includes(`[${id}](#${slug(heading)})`),
      `${id} is never cited where it applies`,
    );
  }
});

test("the decision log records the owner's decision on RFC 0010", () => {
  const files = readdirSync(join(root, "factory/decisions")).filter((f) =>
    /^2026-11-10-NOC-107-/.test(f),
  );
  const entry = files
    .map((f) => read(`factory/decisions/${f}`))
    .find((t) => /^# [^\n]*· The owner decides RFC 0010, /m.test(t));
  assert.ok(entry, "no NOC-107 decision file names RFC 0010 as decided");
  assert.match(entry, /^- Decided by: Carsten, 2026-11-10, on NOC-115/m);
  assert.match(entry, /^- Topics: .*rfc-0010/m);
});

test("History names the draft and the owner's decision", () => {
  const history = section(read(), "History");
  assert.match(history, /^- \*\*\d{4}-\d{2}-\d{2}\*\* · NOC-107 · Draft 0\.1/m);
  assert.match(history, /^- \*\*2026-11-10\*\* · NOC-115 · .*Carsten/m);
});
