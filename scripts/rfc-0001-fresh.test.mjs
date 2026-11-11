// Holds RFC 0001 as a fresh text (NOC-165). The owner wants every RFC to read
// as if it had just been written (Carsten, 2026-11-10): each choice stands in
// the section it belongs to, with a sentence on why and the options not taken,
// and there is no list of decisions, no reference such as "M10", "S7" or
// "decision 13", and no History of the drafts. RFC 0001 held one decision of
// its own, that groups leave the design, which now stands as a section of the
// design, and a list of the decisions of other RFCs it follows, which now
// stand, linked, where the text says them. The rewrite also writes in
// NOC-161: RFC 0001's prose says no more than the later RFCs decide, on rich
// text and streams, adapters, views and what the relay sees. And the FAQ says
// what the recovery code does since RFC 0010 (NOC-132). Whether the wording
// is right is the review of the PR.
import assert from "node:assert/strict";
import { readdirSync } from "node:fs";
import { join, normalize } from "node:path";
import { test } from "node:test";
import { headings, read, root, section, slug } from "./lib/rfc-text.mjs";

const RFC = "docs/rfc/0001-nocentraldb.md";
const FAQ = "docs/faq.md";
const DISCUSSION = "https://github.com/nocentraldb/nocentraldb/discussions/21";
const PUBLISHED =
  "2026-11-10 · First published (RFC 0001 to RFC 0010 rewritten as fresh texts, NOC-165)";

const part = (anchor, path = RFC) => section(read(path), anchor);

const links = (text, target) =>
  [...text.matchAll(/\]\(([^)\s]+)\)/g)].some((m) => m[1].endsWith(target));

const line = (text, start) => {
  const found = text.split("\n").find((l) => l.startsWith(start));
  assert.ok(found, `no line starts with "${start}"`);
  return found;
};

const documents = () => [
  "README.md",
  "CONTRIBUTING.md",
  ...readdirSync(join(root, "docs"), { recursive: true })
    .filter((file) => file.endsWith(".md"))
    .map((file) => normalize(join("docs", file))),
];

test("RFC 0001 has no list of decisions", () => {
  const found = headings(read(RFC)).map(slug);
  for (const gone of ["decisions", "decisions-this-rfc-follows"]) {
    assert.ok(!found.includes(gone), `the section #${gone} is left`);
  }
});

test("RFC 0001 names no decision by its id", () => {
  const text = read(RFC).replace(part("history"), "");
  for (const pattern of [
    /\bM\d+\b/,
    /\bD\d+\b/,
    /\bS(?!3\b)\d+\b/, // S3 is the AWS service
    /\bR\d+\b/,
    /\bdecisions? \d/i,
    /\bconflict \d/i,
    /\bDRAFT 0\.\d/i,
  ]) {
    const hit = text.match(pattern);
    assert.ok(!hit, `RFC 0001 still names "${hit?.[0]}"`);
  }
});

test("no document links a removed anchor of RFC 0001", () => {
  const removed =
    /0001-nocentraldb\.md#(decisions|decisions-this-rfc-follows)\)/;
  const found = documents().filter((path) => removed.test(read(path)));
  assert.deepEqual(found, []);
});

test("the status, the discussion and History", () => {
  assert.equal(
    line(read(RFC), "**Status.**"),
    "**Status.** Decided by the owner (Carsten, 2026-11-10).",
  );
  assert.ok(
    links(part("how-to-give-advice"), DISCUSSION),
    "the discussion #21 is not linked",
  );
  const lines = part("history")
    .split("\n")
    .filter((l) => l.trim());
  assert.deepEqual(lines, [`- ${PUBLISHED}`]);
});

test("groups leave the design, as a section of the design, with the options not taken", () => {
  const groups = part("groups-leave-the-design");
  assert.match(groups, /withdrawn by the owner \(Carsten, 2026-11-09\)/);
  assert.match(groups, /more problems than they solved/);
  assert.match(groups, /groups as members of many spaces/i);
  assert.match(groups, /lists of members that the SDK expands/);
  assert.ok(links(groups, "0008-membership.md#groups"));
  assert.doesNotMatch(groups, /^- \*\*(Chosen|Decided by)/m);
});

test("the rules of other RFCs it follows stand, linked, where the text says them", () => {
  const design = part("the-design-in-one-page");
  assert.ok(
    links(design, "0008-membership.md#how-the-relay-applies-the-rules"),
  );
  assert.ok(links(design, "0007-relay.md#what-the-relay-sees"));
  assert.ok(links(design, "0009-sdk-api.md#queries-and-indexes"));
  const lead = part("the-team-lead-who-has-to-pass-the-data-protection-review");
  assert.ok(links(lead, "0008-membership.md#a-shred-is-final-when-accepted"));
  assert.ok(
    links(lead, "0005-schema-and-model-lock.md#personal-data-in-free-text"),
  );
  assert.ok(
    links(
      part("field-types-we-ship"),
      "0005-schema-and-model-lock.md#personal-fields",
    ),
  );
  assert.ok(
    links(
      part("what-it-is-not"),
      "0006-local-view.md#what-developers-and-their-users-read",
    ),
  );
});

test("rich text and streams are reserved for later RFCs, not shipped", () => {
  // NOC-161
  const types = part("field-types-we-ship");
  assert.match(types, /`nc\.richText`/);
  assert.match(types, /`nc\.stream`/);
  assert.match(types, /later RFCs?/);
  assert.ok(
    links(types, "0005-schema-and-model-lock.md#what-this-rfc-does-not-cover"),
  );
  assert.doesNotMatch(types, /NoCentralDB ships the following field types/);
});

test("an adapter is a member of the space, not part of the relay", () => {
  // NOC-161
  const arch = part("architecture");
  assert.match(arch, /[Aa]dapters? (is|are) (a )?members?/);
  assert.ok(links(arch, "0007-relay.md#what-this-rfc-does-not-cover"));
});

test("reads go through declared indexes; computed fields and views are reserved", () => {
  // NOC-161
  const rfc = read(RFC);
  assert.doesNotMatch(rfc, /asks a local view/);
  assert.doesNotMatch(rfc, /from a local view/);
  assert.doesNotMatch(
    rfc,
    /You declare counts, blockers and rankings as computed fields and views/,
  );
  const design = part("the-design-in-one-page");
  assert.match(line(design, "- **Reads cost only their result.**"), /reserved/);
  assert.match(
    line(design, "- **Ordered pages.**"),
    /[Ee]very index keeps its order/,
  );
});

test("the relay sees content-free effects of a work order, as RFC 0008 lists them", () => {
  // NOC-161
  const work = line(part("the-design-in-one-page"), "- **Work orders.**");
  assert.ok(links(work, "0008-membership.md#shred-entry"));
  assert.doesNotMatch(read(RFC), /only metadata/);
});

test("the FAQ says the recovery code makes a new account key under the same account", () => {
  // NOC-132
  const faq = read(FAQ);
  assert.doesNotMatch(faq, /restores your account key/);
  const sentence = faq
    .split("\n")
    .find((l) => l.startsWith("If you lose every device,"));
  assert.ok(sentence, "the FAQ has no answer for losing every device");
  assert.match(sentence, /new account key under the same account/);
  assert.match(sentence, /not the spaces/);
  assert.ok(links(sentence, "0010-accounts.md#the-recovery-code"));
  assert.ok(links(sentence, "0003-keys-and-cryptography.md#the-account-key"));
});

test("the browser bullet has one pair of parentheses for its links", () => {
  const bullet = line(
    part("what-it-is-not"),
    "- **Not a durable store in every browser.**",
  );
  assert.doesNotMatch(bullet, /\)\) \(\[/);
});
