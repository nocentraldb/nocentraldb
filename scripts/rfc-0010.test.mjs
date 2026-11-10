// Asserts the structure of RFC 0010 (NOC-107), the draft of accounts: the file
// exists and the RFC index lists it; it says plainly that it is a draft and
// that nothing in it is decided; it links to GENESIS.md and to every decided
// RFC it builds on, and every relative link and its anchor resolve; it has a
// section for every part the ticket names (the account key and where it
// lives, the recovery code, adding a device, linking per-database accounts,
// and what a member certificate means for RFC 0008); every open question has
// options, exactly one recommended option and a recommendation; and its
// "Decisions" section records no decision. The test checks the shape of the
// design document, not whether the design is right; that is the owner's
// review of the pull request.
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
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
});

test("RFC 0010 says it is a draft, and that nothing in it is decided", () => {
  const rfc = read();
  assert.match(rfc.split("\n")[0], /^# RFC 0010: Accounts/);
  assert.match(rfc.split("\n")[2], /DRAFT/, "the header line names the draft");
  const status = line(rfc, "**Status.**");
  assert.match(status, /^\*\*Status\.\*\* Draft\b/);
  assert.match(status, /owner/, "the status names who decides");
  assert.match(status, /nothing .*decided|decides nothing/i);
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

/** Each open question, from its "**Qn." line to the next one. */
const questions = () =>
  [
    ...section(read(), "Open questions").matchAll(
      /^\*\*(Q\d+)\.[\s\S]*?(?=^\*\*Q\d+\.|(?![\s\S]))/gm,
    ),
  ].map((match) => ({ id: match[1], text: match[0] }));

test("every open question has options, one recommended option and a recommendation", () => {
  const found = questions();
  assert.ok(found.length >= 8, `${RFC} lists ${found.length} open questions`);
  for (const [i, { id }] of found.entries()) {
    assert.equal(id, `Q${i + 1}`, "the questions are numbered Q1, Q2, ...");
  }
  for (const { id, text } of found) {
    assert.match(text, /^- A\. /m, `${id} has no option A`);
    assert.match(text, /^- B\. /m, `${id} has no option B`);
    assert.equal(
      text.match(/\*\(recommended\)\*/g)?.length,
      1,
      `${id} needs exactly one recommended option`,
    );
    assert.match(text, /^Recommendation: /m, `${id} gives no recommendation`);
    assert.doesNotMatch(text, /^Decided/m, `${id} is decided in a draft`);
  }
});

test("every open question is cited in the text above it", () => {
  const body = prose(read()).split("\n## Open questions\n")[0];
  for (const { id } of questions()) {
    assert.ok(
      body.includes(`[${id}](#open-questions)`),
      `${id} is never cited where its question arises`,
    );
  }
});

test("the Decisions section records no decision yet", () => {
  const decisions = section(read(), "Decisions");
  assert.match(decisions, /^None yet\./m);
  assert.doesNotMatch(decisions, /Decided \(/);
});

test("the History section has a dated line that names the ticket", () => {
  assert.match(
    section(read(), "History"),
    /^- \*\*\d{4}-\d{2}-\d{2}\*\* · NOC-107 · /m,
  );
});
