// Asserts the structure of RFC 0010 (NOC-107), accounts, as the owner decided
// it (NOC-115) and asked it to read (Carsten, 2026-11-10, on #115): as one
// design, with each decision worked into the section it belongs to. The file
// exists and the RFC index lists it; its header and status say it is
// decided, by the owner, with the date; it links to GENESIS.md and to every
// decided RFC it builds on, and every relative link and its anchor resolve;
// it has a section for every part the ticket names; it has no list of
// decisions, no list of rejected options, no open question and no "Qn"
// reference, and no recommendation or draft wording; the options not taken
// still stand in the text; the decision log records the owner's decision on
// the RFC as a whole; and, as every RFC now reads as freshly written
// (NOC-165), the status says in one sentence that the owner decided it, and
// "History" holds the one line of its first publication. The test checks the
// shape of the design document, not whether the design is right; that is the
// owner's review of the pull request.
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
  assert.match(
    status,
    /^\*\*Status\.\*\* Decided\. [^.]*\(Carsten, 2026-11-10\)\.$/,
  );
  assert.doesNotMatch(status, /NOC-\d+|#\d+/, "the status tags a ticket or PR");
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
  // NOC-165: RFC 0010 has its own discussion now.
  assert.ok(
    feedback.includes(
      "(https://github.com/nocentraldb/nocentraldb/discussions/154)",
    ),
    "the line links RFC 0010's discussion",
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
    "agreement with genesis.md and the decided rfcs",
    "history",
    "how this rfc is tested",
  ]) {
    assert.ok(
      found.includes(wanted),
      `${RFC} has no section headed "${wanted}"`,
    );
  }
});

test("RFC 0010 reads as one design: no list of decisions, rejected options or questions", () => {
  const titles = headingLines(read()).map((h) => h.trim().toLowerCase());
  for (const gone of ["decisions", "rejected options", "open questions"]) {
    assert.ok(!titles.includes(gone), `${RFC} still has a "${gone}" section`);
  }
  assert.doesNotMatch(prose(read()), /\bQ\d+\b/, "a Qn reference is left");
});

test("the options not taken still stand in the text", () => {
  const text = prose(read()).split("\n## History\n")[0];
  for (const [option, pattern] of [
    ["the account key as the identity", /account key (?:were|as) the identity/],
    ["24 words of the seed", /twenty-four words|24 words/i],
    ["base32 characters", /base32/],
    ["a recovery member", /recovery member/],
    [
      "a key backup at the relay",
      /backup of the space keys|space keys at the relay/,
    ],
    ["no shared record", /no shared record/i],
    ["a relay mailbox", /mailbox/],
    ["a PAKE", /PAKE|password-authenticated/],
    ["any channel for a device", /any channel/],
    ["no automatic adds", /space by space/],
    ["other admins' SDKs admitting on their own", /without any admin deciding/],
    ["a keystore entry per device", /shar\w* a keystore entry|share one copy/],
    ["re-adding linked members", /re-add/],
    [
      "a certificate-replacing action",
      /replaces a member's certificate|replace a certificate/,
    ],
    ["a separate link call", /db\.account\.link/],
    ["an SDK screen", /its own screen/],
    ["the password manager", /password manager/],
    ["hex or words for the fingerprint", /hexadecimal/],
    ["an account at the relay", /account at the relay/],
    ["a passkey", /passkey/],
    ["admin-only devices", /admin devices/],
    ["a short code compared", /short code/],
    ["social recovery", /social recovery/],
  ]) {
    assert.match(text, pattern, `${RFC} no longer names ${option}`);
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

test("History holds one line: the first publication as a fresh text", () => {
  const lines = section(read(), "History")
    .split("\n")
    .filter((l) => l.startsWith("- "));
  assert.deepEqual(lines, [
    "- 2026-11-10 · First published (RFC 0001 to RFC 0010 rewritten as fresh texts, NOC-165)",
  ]);
  // NOC-165: the intro names no decision of another RFC by its id.
  const rfc = read();
  assert.doesNotMatch(rfc, /decision S15/);
  assert.doesNotMatch(rfc, /decisions? (16 and )?17/);
});
