// Holds RFC 0004 as a fresh text (NOC-165). The owner wants every RFC to read
// as if it had just been written (Carsten, 2026-11-10): each choice stands in
// the section it belongs to, with a sentence on why and the options not
// taken, and there is no list of decisions, no list of rejected options, no
// reference such as "decision 7" or "D12", and no History of the drafts. The
// tests below pin the content of every former decision and rejected option
// where it now stands, and the rules of the field operations and references;
// rfc-0004-examples.test.mjs keeps checking the bytes of E1 to E17. Whether
// the wording is right is the review of the PR.
import assert from "node:assert/strict";
import { readdirSync } from "node:fs";
import { join, normalize } from "node:path";
import { test } from "node:test";
import {
  assertLinksResolve,
  headings,
  read,
  root,
  section,
  slug,
} from "./lib/rfc-text.mjs";

const RFC = "docs/rfc/0004-merge-rules-basic-types.md";

/** The Markdown link targets in `text`. */
const targets = (text) =>
  [...text.matchAll(/\]\(([^)\s]+)\)/g)].map((m) => m[1]);
const links = (text, target) => targets(text).some((t) => t.endsWith(target));

/** The text of RFC 0004's section `anchor`, which must exist. */
const part = (anchor) => {
  const text = section(read(RFC), anchor);
  assert.ok(text, `RFC 0004 has no section #${anchor}`);
  return text;
};

/** The first line of `text` that starts with `start`. */
const line = (text, start) => {
  const found = text.split("\n").find((l) => l.startsWith(start));
  assert.ok(found, `no line starts with "${start}"`);
  return found;
};

const DISCUSSION = "https://github.com/nocentraldb/nocentraldb/discussions/127";
const PUBLISHED =
  "2026-11-10 · First published (RFC 0001 to RFC 0010 rewritten as fresh texts, NOC-165)";

/** The RFC without its History and its byte-exact examples. */
const prose = () =>
  read(RFC)
    .replace(part("history"), "")
    .replace(part("byte-exact-examples"), "");

/** Asserts that `text` matches every pattern in `patterns`. */
const holds = (text, patterns, where) => {
  for (const pattern of patterns) {
    assert.match(text, pattern, `${where} does not say ${pattern}`);
  }
};

/** Every Markdown file a reader can follow a link from. */
const documents = () => [
  "README.md",
  "CONTRIBUTING.md",
  ...readdirSync(join(root, "docs"), { recursive: true })
    .filter((file) => file.endsWith(".md"))
    .map((file) => normalize(join("docs", file))),
];

test("RFC 0004 has no amendment section, mark or changelog", () => {
  const rfc = read(RFC);
  assert.doesNotMatch(rfc, /^## Amendment/m, "an amendment section is left");
  assert.doesNotMatch(rfc, /\(Amendment \d+/, "an amendment mark is left");
  assert.doesNotMatch(rfc, /\*\*Changelog\.\*\*/, "the changelog is left");
  assert.doesNotMatch(rfc, /amendment/i, "the text still speaks of amendments");
});

test("RFC 0004 has no list of decisions or rejected options", () => {
  const found = headings(read(RFC)).map(slug);
  for (const gone of [
    "decisions",
    "decisions-this-rfc-follows",
    "rejected-options",
    "for-how-field-operations-travel",
    "for-references",
  ]) {
    assert.ok(!found.includes(gone), `the section #${gone} is left`);
  }
  assert.ok(
    !found.some((anchor) => /^\d+-/.test(anchor)),
    "a numbered decision heading is left",
  );
});

test("RFC 0004 names no decision by its id, its own or another RFC's", () => {
  const text = prose();
  for (const pattern of [
    /\bdecisions? \d/i,
    /\bD\d+\b/,
    /\bM\d+\b/,
    /\bconflict point/i,
    /\bConflicts point/,
    /\boption [13]\b/,
  ]) {
    const hit = text.match(pattern);
    assert.ok(!hit, `RFC 0004 still names "${hit?.[0]}"`);
  }
});

test("no document links a removed anchor of RFC 0004", () => {
  const removed =
    /0004-merge-rules-basic-types\.md#(amendment-|decisions|\d+-|rejected-options|for-how-field-operations-travel|for-references|a-reference-into-another-space|a-create-that-carries-arguments)/;
  const found = documents().filter((path) => removed.test(read(path)));
  assert.deepEqual(found, []);
});

test("the status says in one sentence that the owner decided it, and when", () => {
  const status = read(RFC)
    .split("\n")
    .find((l) => l.startsWith("**Status.**"));
  assert.equal(
    status,
    "**Status.** Decided by the owner (Carsten, 2026-11-10).",
  );
});

test("the feedback line links the RFC's discussion", () => {
  const feedback = line(read(RFC), "**How to give feedback.**");
  assert.ok(links(feedback, DISCUSSION), "the discussion #127 is not linked");
  assert.ok(links(feedback, "README.md#giving-feedback"));
  assert.doesNotMatch(feedback, /no discussion of its own/);
});

test("History has one line: the first publication", () => {
  const lines = part("history")
    .split("\n")
    .filter((l) => l.trim());
  assert.deepEqual(lines, [`- ${PUBLISHED}`]);
});

// Each former decision and rejected option stands in its section. The comment
// names the decision as the old text numbered it, so the PR's mapping table
// can be checked against this file.

test("basic values: the last writer wins and losing writes stay, and the options not taken", () => {
  // 1; rejected: plain last writer wins, a multi-value register, wall-clock
  // time alone, arrival order, "arrived late" in the state
  holds(
    part("basic-values"),
    [
      /principle 3/,
      /[Pp]lain last writer wins/,
      /multi-value register/,
      /[Ww]all-clock time alone/,
      /arrival order/,
      /"arrived late"/,
    ],
    "Basic values",
  );
});

test("the tie-break says why the author comes before the operation id", () => {
  // 2
  holds(part("tie-break"), [/operation id alone/, /random/], "Tie-break");
  assert.doesNotMatch(part("tie-break"), /owner decision/);
});

test("sets: add wins, and the set rules not taken", () => {
  // 3; rejected: remove wins, two-phase set, last-writer-wins element set
  holds(
    part("concurrent-addition-and-removal"),
    [/[Rr]emove wins/, /[Tt]wo-phase set/, /[Ll]ast-writer-wins element set/],
    "Concurrent addition and removal",
  );
});

test("counters are a different field type, and why", () => {
  // rejected: counters for numbers
  holds(
    part("value-rules-per-kind"),
    [/counter/, /its own type/],
    "Value rules",
  );
});

test("deletion is a converged boolean that can be undone, and final deletion is not taken", () => {
  // 4
  holds(
    part("creating-and-deleting-an-object"),
    [
      /can be undone/,
      /edits to a deleted object are kept/i,
      /final/,
      /lost silently/,
    ],
    "Creating and deleting an object",
  );
  assert.ok(
    links(
      part("a-link-whose-target-was-deleted"),
      "#creating-and-deleting-an-object",
    ),
  );
});

test("a link to a target not received stays, and the options not taken", () => {
  // 5; rejected: dropping such links, refusing a link to an unknown target
  holds(
    part("a-link-to-an-object-the-member-has-not-received"),
    [
      /[Hh]iding such links/,
      /waiting for data/,
      /[Dd]ropping/,
      /referential integrity/,
    ],
    "A link to an object the member has not received",
  );
});

test("no field type assumes causal delivery, and what it costs", () => {
  // 6; rejected: requiring causal delivery
  holds(
    part("what-this-rfc-needs-from-rfc-0002"),
    [/grow until compaction/, /reorders/],
    "What this RFC needs from RFC 0002",
  );
});

test("the names the app reads are working names, and why", () => {
  // 7
  holds(
    part("late-changes-stay-identifiable"),
    [/working names/, /API review/, /where an app meets them/],
    "Late changes stay identifiable",
  );
});

test("the arguments sit in key 5 as a map, and the layouts not taken", () => {
  // 8
  holds(
    part("where-field-operations-sit-in-a-change"),
    [/keys 6 and 7/, /positional array/],
    "Where field operations sit in a change",
  );
});

test("one table of operation numbers, and the numberings not taken", () => {
  // 9
  holds(
    part("operation-numbers"),
    [
      /[Nn]umbers per type/,
      /as text/,
      /parked, not misread|is parked, instead of being misread/,
    ],
    "Operation numbers",
  );
});

test("operation ids, their lists and the net effect of a transaction, with the forms not taken", () => {
  // 10, 11
  holds(
    part("the-arguments-of-each-operation"),
    [
      /one byte string/,
      /prefix of the change id/,
      /empty `replaces` left out|Leaving an empty `replaces` out/,
      /[Rr]ejecting an unsorted or repeating list/,
      /bare position/,
    ],
    "The arguments of each operation",
  );
});

test("malformed arguments are parked, and the options not taken", () => {
  // 12
  holds(
    part("operations-a-member-does-not-understand"),
    [/leniently/, /whole change/],
    "Operations a member does not understand",
  );
});

test("a new operation is a new rule version, and the option not taken", () => {
  // 13
  holds(
    part("how-a-field-type-adds-operations"),
    [/under the same rule version/],
    "How a field type adds operations",
  );
});

test("deletion travels as operation 4, a create with arguments is malformed, with the forms not taken", () => {
  // 14, a create that carries arguments
  holds(
    part("creating-and-deleting-an-object"),
    [
      /`\$deleted`/,
      /key 3 absent/,
      /[Ss]eparate delete and restore/,
      /[Ii]gnoring key 5/,
    ],
    "Creating and deleting an object",
  );
});

test("a reference: its type, its encoding and its states, with the options not taken", () => {
  // 15 to 20, and a reference into another space
  holds(
    part("references"),
    [
      /`ncdb\/link` with a new rule version/,
      /new operation number/,
      /plain value/,
      /[Cc]ross-space links/,
      /`nc\.link`/,
    ],
    "References",
  );
  holds(
    part("the-value-and-its-encoding"),
    [/32-byte byte string/, /[Aa] map/, /`undefined`/, /`invalidValue`/],
    "The value and its encoding",
  );
  holds(
    part("how-a-reference-resolves"),
    [/fourth state/, /current member/, /stored state/],
    "How a reference resolves",
  );
});

test("plugins are Rust compiled into the core, and why", () => {
  // rejected: plugins loaded at run time
  holds(
    part("the-plugin-contract"),
    [/loaded at run time/, /compiled into the core/],
    "The plugin contract",
  );
});

test("the limits stand as limits, without the bookkeeping of the draft", () => {
  // the former conflicts 1 to 5
  const text = part("agreement-with-genesismd-and-rfc-0001");
  holds(
    text,
    [/highest stamp/, /do not add up/, /not arrived/, /grow/],
    "Agreement",
  );
  assert.doesNotMatch(text, /Resolved/);
  assert.doesNotMatch(text, /for the owner and pm/);
});

test("how field operations travel in a change has its sections and defines set, add, remove and delete", () => {
  const travel = part("how-field-operations-travel-in-a-change");
  const found = headings(`## x\n${travel}`).map((h) => h.toLowerCase());
  for (const wanted of [
    "where field operations sit in a change",
    "operation numbers",
    "the arguments of each operation",
    "creating and deleting an object",
    "operations a member does not understand",
    "how a field type adds operations",
    "byte-exact examples",
    "what the relay sees",
  ]) {
    assert.ok(found.includes(wanted), `no section "${wanted}"`);
  }
  for (const operation of ["set", "add", "remove", "delete"]) {
    assert.match(travel, new RegExp(`\\*\\*${operation}\\*\\*`));
  }
  assert.ok(
    links(
      line(read(RFC), "**Out of scope.**"),
      "#how-field-operations-travel-in-a-change",
    ),
  );
});

test("a create with key 5 is parked as malformed, and the depth links RFC 0002's profile", () => {
  assert.match(
    line(
      part("creating-and-deleting-an-object"),
      "- **A create with key 5 present is malformed.**",
    ),
    /"malformed arguments"/,
  );
  const depth = line(part("the-arguments-of-each-operation"), "**Depth.**");
  assert.ok(links(depth, "0002-change-and-log-format.md#the-cbor-profile"));
  assert.doesNotMatch(read(RFC), /pending the owner in NOC-24/);
});

test("a reference has its own section: state, three states, encoding, what is seen and what it does not do", () => {
  const refs = part("references");
  const found = headings(`## x\n${refs}`).map((h) => h.toLowerCase());
  for (const wanted of [
    "state and operations",
    "how a reference resolves",
    "the value and its encoding",
    "what the relay and the members see",
    "what this does not do",
  ]) {
    assert.ok(found.includes(wanted), `no section "${wanted}"`);
  }
  const states = part("how-a-reference-resolves");
  for (const state of ["**ok**", "**not accessible**", "**missing**"]) {
    assert.ok(states.includes(`| ${state}`), `no row for ${state}`);
  }
  assert.match(part("the-value-and-its-encoding"), /\[space id, object id\]/);
  assert.match(
    line(part("summary"), "| Reference"),
    /space id and an object id/,
  );
  assert.match(line(part("operation-numbers"), "| 1 "), /reference/);
  assert.ok(
    links(
      part("a-link-to-an-object-the-member-has-not-received"),
      "#references",
    ),
  );
});

test("every relative link in RFC 0004 resolves, anchors included", () => {
  assertLinksResolve(RFC);
});
