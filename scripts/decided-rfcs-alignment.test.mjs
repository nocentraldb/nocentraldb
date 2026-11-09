// Asserts that the texts a user reads first follow the owner's decisions in
// RFC 0003, RFC 0005 and RFC 0006 (NOC-35): the FAQ says what a lost device
// and a browser copy give away and keep, RFC 0001 says which fields can be
// personal, and every list of erasure limits names the app's duty for free
// text and that removing a member is not erasure. Each claim links its RFC
// section; whether the wording is right is the owner's review of the PR.
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, normalize } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (path) => readFileSync(join(root, path), "utf8");

const RFC_0001 = "docs/rfc/0001-nocentraldb.md";
const FAQ = "docs/faq.md";
const USE_CASES = "docs/use-cases.md";
const GENESIS = "factory/GENESIS.md";

const PLAINTEXT = "0006-local-view.md#what-the-view-stores-in-plaintext";
const RFC_0006_DECISIONS = "0006-local-view.md#decisions-for-the-owner";
const RFC_0006_BROWSER = "0006-local-view.md#the-browser";
const REMOVING =
  "0003-keys-and-cryptography.md#removing-a-member-and-rotating-the-space-key";
const ACCOUNT_KEY = "0003-keys-and-cryptography.md#the-account-key";
const PERSONAL = "0005-schema-and-model-lock.md#personal-fields";
const BUILD_CHECKS = "0005-schema-and-model-lock.md#what-the-build-checks";
const FREE_TEXT = "0005-schema-and-model-lock.md#personal-data-in-free-text";
const NOT_ERASURE = "0005-schema-and-model-lock.md#about-whom-not-by-whom";

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

/** The text under the heading `heading`, up to the next heading of the same or a higher level. */
const section = (markdown, heading) => {
  const lines = markdown.split("\n");
  const start = lines.findIndex((line) => line.replace(/^#+ /, "") === heading);
  assert.ok(start >= 0, `no heading "${heading}"`);
  const level = lines[start].match(/^#+/)[0].length;
  const rest = lines.slice(start + 1);
  const end = rest.findIndex((line) => {
    const m = line.match(/^(#+) /);
    return m && m[1].length <= level;
  });
  return (end < 0 ? rest : rest.slice(0, end)).join("\n");
};

/** The sentences of `text` that link a target ending with `target`. */
const sentencesLinking = (text, target) =>
  text
    .split(/(?<=[.!?])\s+(?=[A-Z[])/)
    .filter((s) =>
      [...s.matchAll(/\]\(([^)\s]+)\)/g)].some(([, t]) => t.endsWith(target)),
    );

const linkingSentence = (text, target, pattern, what) => {
  const found = sentencesLinking(text, target).find((s) => pattern.test(s));
  assert.ok(
    found,
    `${what}: no sentence linking ${target} that matches ${pattern}`,
  );
  return found;
};

const LOST_DEVICE = "What happens if I lose my device?";
const STORAGE = "Does every device have to store all the data?";
const GDPR = "Does destroying a key satisfy a GDPR erasure request?";

test("the FAQ says a lost device holds every space in plaintext, with RFC 0006's section", () => {
  const answer = section(read(FAQ), LOST_DEVICE);
  linkingSentence(answer, PLAINTEXT, /plaintext/i, "plaintext");
  linkingSentence(
    answer,
    PLAINTEXT,
    /unlocked.*(disk|storage)|(disk|storage).*unlocked/i,
    "unlocked device or its disk",
  );
});

test("the FAQ says v1 relies on the device's disk encryption, with RFC 0006's decision 6", () => {
  const answer = section(read(FAQ), LOST_DEVICE);
  linkingSentence(answer, RFC_0006_DECISIONS, /disk encryption/i, "decision 6");
  assert.match(answer, /decision 6/, "the FAQ does not name decision 6");
});

test("the FAQ says removing the member does not delete the lost device's copy, with RFC 0003's section", () => {
  const answer = section(read(FAQ), LOST_DEVICE);
  linkingSentence(answer, REMOVING, /does not delete/i, "removing a member");
});

test("the FAQ follows RFC 0003 on recovering the account after losing every device", () => {
  const answer = section(read(FAQ), LOST_DEVICE);
  assert.ok(
    !/passkey/i.test(answer),
    "RFC 0003 decided against a passkey in v1",
  );
  assert.ok(
    !/does not yet specify how a person regains access/.test(answer),
    "RFC 0003 specifies the recovery code",
  );
  linkingSentence(answer, ACCOUNT_KEY, /recovery code/i, "recovery code");
});

test("the FAQ says a browser may evict its copy, Safari after seven days, with RFC 0006", () => {
  const answer = section(read(FAQ), STORAGE);
  linkingSentence(
    answer,
    RFC_0006_BROWSER,
    /seven days/i,
    "Safari's seven-day rule",
  );
  linkingSentence(answer, "browser-durability.md", /./, "the durability page");
  linkingSentence(answer, "browser-storage.md", /./, "the measurements");
});

test("the FAQ says the SDK asks for persistent storage, with RFC 0006's decision 8", () => {
  const answer = section(read(FAQ), STORAGE);
  linkingSentence(
    answer,
    RFC_0006_DECISIONS,
    /persistent storage/i,
    "decision 8",
  );
  assert.match(answer, /decision 8/, "the FAQ does not name decision 8");
});

test("RFC 0001 says only basic values can be personal, and only when the field is added", () => {
  const line = read(RFC_0001)
    .split("\n")
    .find((l) => l.startsWith("- **Personal fields.**"));
  assert.ok(line, "RFC 0001 has no bullet Personal fields");
  assert.ok(
    !/Any field can be marked/i.test(line),
    "RFC 0001 still says any field",
  );
  const claim = linkingSentence(line, BUILD_CHECKS, /basic value/i, "D9");
  assert.match(
    claim,
    /when the field is added/,
    "D9: from the version that adds it",
  );
});

/** Each list of erasure limits, with the text that holds it. */
const erasureLists = () => [
  {
    what: "RFC 0001's team lead",
    text: section(
      read(RFC_0001),
      "The team lead who has to pass the data-protection review",
    ),
  },
  {
    what: "use case 1",
    text: section(read(USE_CASES), "1. Erasure that reaches every copy"),
  },
  { what: "the FAQ's GDPR answer", text: section(read(FAQ), GDPR) },
];

test("every list of erasure limits names the app's duty for free text, with RFC 0005's pattern", () => {
  for (const { what, text } of erasureLists()) {
    linkingSentence(text, FREE_TEXT, /reference/i, what);
  }
});

test("every list of erasure limits says removing a member is not erasure, with RFC 0005", () => {
  for (const { what, text } of erasureLists()) {
    linkingSentence(
      text,
      NOT_ERASURE,
      /removing a member is not (an )?erasure/i,
      what,
    );
  }
});

test("GENESIS.md's Shred and audit points to RFC 0005's pattern for free text", () => {
  const line = read(GENESIS)
    .split("\n")
    .find((l) => l.startsWith("- **Shred and audit.**"));
  assert.ok(line, "GENESIS.md has no bullet Shred and audit");
  assert.match(line, /unmarked free text/, "the limit is no longer named");
  linkingSentence(line, FREE_TEXT, /free text/i, "GENESIS.md");
});

test("RFC 0001 announces Amendment 4 in a dated NOC-35 changelog line, and its section decides nothing", () => {
  const rfc = read(RFC_0001);
  const top = rfc.split(/^## /m)[0];
  assert.match(
    top,
    /^- \*\*\d{4}-\d{2}-\d{2}\*\* · NOC-35 · \[Amendment 4: [^\]]+\]\(#amendment-4-[^)]+\)/m,
    "no changelog line for Amendment 4",
  );
  const heading = rfc.split("\n").find((l) => l.startsWith("## Amendment 4: "));
  assert.ok(heading, "no section Amendment 4");
  const body = section(rfc, heading.slice(3));
  assert.match(body, /decides nothing new/, "Amendment 4 must decide nothing");
  assert.ok(
    sentencesLinking(body, PERSONAL).length +
      sentencesLinking(body, FREE_TEXT).length >
      0,
  );
  assert.ok(
    (rfc.match(/\*\(Amendment 4\.\)\*/g) ?? []).length >= 3,
    "the changed sentences are not marked",
  );
});

test("every relative link in the changed documents resolves, anchors included", () => {
  for (const path of [
    FAQ,
    RFC_0001,
    USE_CASES,
    GENESIS,
    "docs/rfc/0002-change-and-log-format.md",
    "docs/rfc/0003-keys-and-cryptography.md",
    "docs/rfc/0006-local-view.md",
    "docs/rfc/0007-relay.md",
    "docs/rfc/0008-membership.md",
  ]) {
    const markdown = read(path).replace(/```[\s\S]*?```/g, "");
    for (const [, target] of markdown.matchAll(/\]\(([^)\s]+)\)/g)) {
      if (/^[a-z]+:/.test(target)) continue;
      const [file, anchor] = target.split("#");
      const resolved = file ? normalize(join(dirname(path), file)) : path;
      assert.ok(
        existsSync(join(root, resolved)),
        `${path}: ${target} has no file`,
      );
      if (anchor && resolved.endsWith(".md")) {
        assert.ok(
          anchors(read(resolved)).has(anchor),
          `${path}: ${target} names no heading`,
        );
      }
    }
  }
});
