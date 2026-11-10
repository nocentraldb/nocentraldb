// Holds RFC 0004 as one complete text (NOC-100). The owner wants every RFC to
// state the current design directly (Carsten, 2026-11-10): the rules of its
// four amendments stand where they belong, no amendment section, mark or
// changelog is left, "Decisions" keeps each decision in short form, and a
// dated "History" names the pull request of each change. This test replaces
// rfc-0004-amendment-4.test.mjs, the amendment checks of rfc-0004.test.mjs
// and RFC 0004's Amendment 2 check in decided-rfcs-alignment;
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

/** The decision under "Decisions" whose heading starts with `prefix`, such as "9.". */
const decision = (prefix) => {
  const decisions = part("decisions");
  const heading = decisions
    .split("\n")
    .find((l) => l.startsWith(`### ${prefix}`));
  assert.ok(heading, `no decision "${prefix}"`);
  return section(decisions, slug(heading.slice(4)));
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

test("no document links an amendment of RFC 0004 or its old decisions headings", () => {
  const found = documents().filter((path) =>
    /0004-merge-rules-basic-types\.md#(amendment-|decisions-for-the-owner)/.test(
      read(path),
    ),
  );
  assert.deepEqual(found, []);
  assert.doesNotMatch(read(RFC), /\(#decisions-for-the-owner/);
});

test("the status says the RFC is decided and points to Decisions and History", () => {
  const status = line(read(RFC), "**Status.**");
  assert.match(status, /^\*\*Status\.\*\* Decided\./);
  assert.match(status, /#26/);
  assert.ok(links(status, "#decisions"));
  assert.ok(links(status, "#history"));
});

test("decisions 1 to 20 each say what was chosen, by the owner and when, why, and what was not taken", () => {
  for (let n = 1; n <= 20; n += 1) {
    const text = decision(`${n}.`);
    const date = n <= 7 ? "2026-11-07" : n <= 14 ? "2026-11-08" : "2026-11-10";
    assert.match(
      text,
      new RegExp(`^Decided \\(Carsten, ${date}\\): `, "m"),
      `decision ${n}: no owner and date`,
    );
    assert.match(text, /^Why: /m, `decision ${n}: no reason`);
    assert.match(text, /^Not taken:$/m, `decision ${n}: no options not taken`);
    assert.match(text, /^- /m, `decision ${n}: lists no option not taken`);
    assert.doesNotMatch(
      text,
      /Recommended/,
      `decision ${n} is not in short form`,
    );
  }
});

test("the owner's decisions without a number are recorded: a reference into another space, and a create with arguments", () => {
  const ref = decision("A reference into another space");
  assert.match(ref, /^Decided \(Carsten, 2026-11-09\): /m);
  assert.match(ref, /NOC-29/);
  assert.ok(links(ref, "#references"));
  const notTaken = ref.split(/^Not taken:$/m)[1] ?? "";
  assert.ok((notTaken.match(/^- /gm) ?? []).length >= 2);
  const create = decision("A create that carries arguments");
  assert.match(create, /^Decided \(Carsten, 2026-11-09\): /m);
  assert.match(create, /"malformed arguments"/);
  assert.ok(
    links(create, "2026-11-09-NOC-39-a-create-with-arguments-is-malformed.md"),
    "no link to NOC-39's decision",
  );
});

test("\"Decisions this RFC follows\" names RFC 0002's nesting limit and RFC 0005's decisions, each linked", () => {
  const lines = part("decisions-this-rfc-follows")
    .split("\n")
    .filter((l) => l.startsWith("- **"));
  for (const [id, target] of [
    ["NOC-24", "0002-change-and-log-format.md#the-nesting-limit"],
    [
      "D12",
      "0005-schema-and-model-lock.md#d12-moving-a-field-to-a-new-rule-version",
    ],
    [
      "D18",
      "0005-schema-and-model-lock.md#d18-how-the-schema-declares-a-reference",
    ],
  ]) {
    const found = lines.find((l) => new RegExp(`\\b${id}\\b`).test(l));
    assert.ok(found, `no decision followed names ${id}`);
    assert.match(
      found,
      /\(Carsten, 2026-11-\d{2}\)/,
      `${id}: no owner and date`,
    );
    assert.ok(links(found, target), `${id}: no link to ${target}`);
  }
});

test('"History" lists each change by date, oldest first, with its pull request', () => {
  const lines = part("history")
    .split("\n")
    .filter((l) => l.startsWith("- **"));
  const dates = lines.map(
    (l) => l.match(/^- \*\*(\d{4}-\d{2}-\d{2})\*\*/)?.[1],
  );
  assert.ok(dates.every(Boolean), "a History line has no date");
  assert.deepEqual([...dates].sort(), dates, "History is not oldest first");
  for (const l of lines) {
    assert.match(
      l,
      /\]\(https:\/\/github\.com\/nocentraldb\/nocentraldb\/pull\/\d+\)/,
      `no pull request: ${l.slice(0, 60)}`,
    );
  }
  for (const ticket of [
    "NOC-8",
    "NOC-37",
    "NOC-35",
    "NOC-75",
    "NOC-63",
    "NOC-100",
  ]) {
    assert.ok(
      lines.some((l) => new RegExp(`\\b${ticket}\\b`).test(l)),
      `History does not name ${ticket}`,
    );
  }
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
