// Holds RFC 0005 as a fresh text (NOC-165). The owner wants every RFC to read
// as if it had just been written (Carsten, 2026-11-10): each choice stands in
// the section it belongs to, with a sentence on why and the options not taken,
// and there is no list of decisions, rejected options or open questions, no
// reference such as "D13" or "decision S8", and no History of the drafts. The
// tests below pin the content of every former decision D1 to D23 and every
// former rejected option where it now stands. Whether the wording is right is
// the review of the PR.
import assert from "node:assert/strict";
import { readdirSync } from "node:fs";
import { join, normalize } from "node:path";
import { test } from "node:test";
import { headings, read, root, section, slug } from "./lib/rfc-text.mjs";

const RFC = "docs/rfc/0005-schema-and-model-lock.md";
const DISCUSSION = "https://github.com/nocentraldb/nocentraldb/discussions/144";
const PUBLISHED =
  "2026-11-10 · First published (RFC 0001 to RFC 0010 rewritten as fresh texts, NOC-165)";

const part = (anchor) => section(read(RFC), anchor);

const links = (text, target) =>
  [...text.matchAll(/\]\(([^)\s]+)\)/g)].some((m) => m[1].endsWith(target));

const line = (text, start) => {
  const found = text.split("\n").find((l) => l.startsWith(start));
  assert.ok(found, `no line starts with "${start}"`);
  return found;
};

const holds = (text, patterns, where) => {
  for (const pattern of patterns) {
    assert.match(text, pattern, `${where} does not say ${pattern}`);
  }
};

const documents = () => [
  "README.md",
  "CONTRIBUTING.md",
  ...readdirSync(join(root, "docs"), { recursive: true })
    .filter((file) => file.endsWith(".md"))
    .map((file) => normalize(join("docs", file))),
];

test("RFC 0005 has no list of decisions, rejected options or open questions", () => {
  const found = headings(read(RFC)).map(slug);
  for (const gone of ["decisions", "rejected-options", "open-questions"]) {
    assert.ok(!found.includes(gone), `the section #${gone} is left`);
  }
  assert.ok(
    !found.some((anchor) => /^d\d+-/.test(anchor)),
    "a decision heading such as D1 is left",
  );
});

test("RFC 0005 names no decision by its id, its own or another RFC's", () => {
  const text = read(RFC).replace(part("history"), "");
  for (const pattern of [
    /\bD\d+\b/,
    /\bS\d+\b/,
    /\bM\d+\b/,
    /\bdecisions? [A-Z]?\d/i,
    /Accepted \(Carsten/,
    /\bfor the owner\b/,
  ]) {
    const hit = text.match(pattern);
    assert.ok(!hit, `RFC 0005 still names "${hit?.[0]}"`);
  }
});

test("no document links a removed anchor of RFC 0005", () => {
  const removed =
    /0005-schema-and-model-lock\.md#(d\d+-|decisions|rejected-options|open-questions)/;
  const found = documents().filter((path) => removed.test(read(path)));
  assert.deepEqual(found, []);
});

test("the status, the discussion and History", () => {
  assert.equal(
    line(read(RFC), "**Status.**"),
    "**Status.** Decided by the owner (Carsten, 2026-11-10).",
  );
  const feedback = line(read(RFC), "**How to give feedback.**");
  assert.ok(links(feedback, DISCUSSION), "the discussion #144 is not linked");
  assert.ok(links(feedback, "README.md#giving-feedback"));
  const lines = part("history")
    .split("\n")
    .filter((l) => l.trim());
  assert.deepEqual(lines, [`- ${PUBLISHED}`]);
});

// Each former decision stands in its section. The comment names the decision
// as the old text numbered it, so the PR's mapping table can be checked
// against this file.

test("the style of the schema, and the styles not taken", () => {
  // D1; rejected: Zod or Valibot, a language of its own, Drizzle, decorators
  holds(
    part("the-schema-in-typescript"),
    [/compiler setting/, /Zod/, /Prisma/, /Drizzle/, /decorators/],
    "The schema in TypeScript",
  );
});

test("names: no renames, and no numeric field ids", () => {
  // D8; rejected: numeric field ids
  holds(
    part("names"),
    [/alias/, /additive/, /[Nn]umeric field ids/, /registry/],
    "Names",
  );
});

test("no required fields and no defaults, and the options not taken", () => {
  // D6
  holds(
    part("no-required-fields-and-no-defaults"),
    [
      /[Dd]efaults, recorded in the lock/,
      /[Rr]equired fields that the SDK enforces/,
    ],
    "No required fields and no defaults",
  );
});

test("modifiers: deprecation for good, on fields and collections", () => {
  // D7, D17; rejected: removing a field after it was deprecated
  holds(
    part("modifiers"),
    [
      /cannot be undone/,
      /no version guarantees|There is no version after which/,
      /spells the same thing a second way/,
      /new objects could keep appearing/,
    ],
    "Modifiers",
  );
});

test("references: how they are declared, indexed, read and followed, with the options not taken", () => {
  // D18 to D22
  holds(
    part("references-into-another-space"),
    [
      /One collection only/i,
      /`nc\.ref\(\)` to any object/,
      /[Tt]wo columns/,
      /object id only/,
      /two members with the same copy of the space would read different objects/,
      /reads a second space/,
      /reverse map across spaces/,
      /`nc\.refs\("collection"\)`/,
    ],
    "References into another space",
  );
});

test("an int in TypeScript is a number, and the types not taken", () => {
  // D16
  holds(
    part("types-for-the-app"),
    [/2\^53/, /`3n`/, /`number \| bigint`/],
    "Types for the app",
  );
});

test("the lock: JSON in one form, with history, and the formats not taken", () => {
  // D3, D4; rejected: the SDKs read the TypeScript, a lock without history,
  // YAML or CBOR, semantic versions
  holds(
    part("the-structure-of-modellockjson"),
    [
      /YAML/,
      /CBOR/,
      /generated code has no history/,
      /lock without history/i,
      /[Ss]emantic versions?/,
      /content hash only/i,
      /needs model version 7/,
    ],
    "The structure of model.lock.json",
  );
});

test("the additive-change rule: options and targets may be added, and rule versions wait for migrations", () => {
  // D5, D12
  holds(
    part("the-additive-change-rule"),
    [
      /add a status/,
      /[Oo]nly new collections, new fields and descriptions/,
      /stated share of members/,
      /migrations RFC before v1/,
    ],
    "The additive-change rule",
  );
});

test("the check on the device keeps the newest lock, and the options not taken", () => {
  // D11, D23
  holds(
    part("two-checks-at-build-and-on-the-device"),
    [
      /[Tt]rust(ing)? the build check alone/,
      /last lock that opened the database/,
      /both the newest and the last lock/,
    ],
    "Two checks: at build and on the device",
  );
});

test("data from a newer model: the model version and the model id in changes", () => {
  // D10, D13, and the reading of principle 2
  holds(
    part("data-from-a-newer-model"),
    [
      /only that changes are parked/,
      /space's `model\.id` only/,
      /`model\.id` in each change only/,
      /document the risk/,
      /every client that understands all the changes it holds/,
    ],
    "Data from a newer model",
  );
});

test("personal fields: basic values, one person across spaces, and free text", () => {
  // D9, D14, D15
  holds(
    part("personal-fields"),
    [
      /email address/,
      /128 random bits/,
      /[Pp]er space only/,
      /derived from an external identifier/,
      /scans free text/,
      /leaves every app to find the risk/,
    ],
    "Personal fields",
  );
});

test("the reading of GENESIS.md stands as the design's reading, and the public names say why", () => {
  // D2, and the accepted reading of "a model old clients cannot read"
  holds(
    part("agreement-with-genesismd"),
    [/a model whose data old clients cannot keep/],
    "Agreement with GENESIS.md",
  );
  holds(
    part("public-names"),
    [/first thing a developer types/],
    "Public names",
  );
});
