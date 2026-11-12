// Asserts RFC 0011, "Plugin field types" (NOC-235), as the owner decided its
// design on NOC-232 (Carsten, 2026-11-12: the seven points as recommended).
// The file exists and the RFC index lists it; its status says the owner
// decided it, with the date; it says how to give feedback; it links
// GENESIS.md and the RFCs it builds on, and every link and anchor resolves;
// it has a section for every part NOC-235 names; it reads as one design, with
// no list of decisions, rejected options or open questions and no
// recommendation or draft wording; each of the seven decisions stands in its
// section with the owner's name, date and ticket, and with the options not
// taken; RFC 0004, RFC 0005 and RFC 0009 point to it and take its names; the
// decision log records the decision; and "History" holds one line. The test
// checks the shape of the design document and the decisions it carries, not
// whether the design is right: that is the owner's review of the pull
// request.
import assert from "node:assert/strict";
import { existsSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import {
  assertLinksResolve,
  headings,
  read,
  relativeLinks,
  root,
  section,
} from "./lib/rfc-text.mjs";

const RFC = "docs/rfc/0011-plugin-field-types.md";
const INDEX = "docs/rfc/README.md";
const BY_OWNER = "(Carsten, 2026-11-12, NOC-232)";

const rfc = () => read(RFC);

/** The text of a Markdown file outside fenced code blocks. */
const prose = (markdown) => markdown.replace(/^```[\s\S]*?^```$/gm, "");

/** The first line of `markdown` that starts with `start`. */
const line = (markdown, start) =>
  markdown.split("\n").find((l) => l.startsWith(start)) ?? "";

/** Asserts that `text` holds every pattern, naming the section on failure. */
const holds = (text, patterns, where) => {
  for (const pattern of patterns) {
    assert.match(text, pattern, `"${where}" does not hold ${pattern}`);
  }
};

/** Asserts that the section holds the owner's decision, by name, date and ticket. */
const decided = (text, where) =>
  assert.ok(text.includes(BY_OWNER), `"${where}" does not name ${BY_OWNER}`);

test("RFC 0011 exists and the RFC index lists it", () => {
  assert.ok(existsSync(join(root, RFC)), `${RFC} is missing`);
  assert.match(
    read(INDEX),
    /^\| \[0011\]\(0011-plugin-field-types\.md\) \| .*[Pp]lugin field types.* \|$/m,
  );
});

test("RFC 0011 says it is decided, by the owner, with the date", () => {
  const text = rfc();
  assert.equal(text.split("\n")[0], "# RFC 0011: Plugin field types");
  assert.doesNotMatch(text.split("\n")[2], /draft/i);
  const status = line(text, "**Status.**");
  assert.match(
    status,
    /^\*\*Status\.\*\* Decided\. [^\n]*\(Carsten, 2026-11-12\)\.$/,
  );
  assert.doesNotMatch(status, /NOC-\d+|#\d+/, "the status tags a ticket or PR");
});

test("RFC 0011 says how to give feedback", () => {
  const feedback = line(rfc(), "**How to give feedback.**");
  assert.match(feedback, /\(README\.md#giving-feedback\)/);
  assert.ok(
    feedback.includes(
      "(https://github.com/nocentraldb/nocentraldb/discussions/categories/rfc-feedback)",
    ),
    "the line links the RFC feedback category",
  );
});

for (const target of [
  "0001-nocentraldb.md",
  "0002-change-and-log-format.md",
  "0004-merge-rules-basic-types.md",
  "0005-schema-and-model-lock.md",
  "0006-local-view.md",
  "0009-sdk-api.md",
  "../../factory/GENESIS.md",
]) {
  test(`RFC 0011 links ${target}`, () => {
    assert.ok(
      relativeLinks(rfc()).some((l) => l.split("#")[0] === target),
      `${RFC} has no link to ${target}`,
    );
  });
}

test("every relative link in RFC 0011 resolves, anchors included", () => {
  assertLinksResolve(RFC);
});

test("RFC 0011 has a section for every part NOC-235 names", () => {
  const found = headings(rfc()).map((h) => h.trim().toLowerCase());
  for (const wanted of [
    "summary",
    "terms",
    "where things stand today",
    "the field type a plugin implements",
    "the data a plugin reads and writes",
    "a plugin and the registry",
    "handing plugins to the core",
    "typescript",
    "a type that is not registered",
    "checking a plugin: ncdb-harness",
    "what a plugin depends on: ncdb-plugin",
    "the view keeps no column for a plugin field",
    "what a plugin can reach",
    "what this does not do",
    "agreement with genesis.md and the decided rfcs",
    "changes to the decided rfcs",
    "history",
    "how this rfc is tested",
  ]) {
    assert.ok(found.includes(wanted), `${RFC} has no section "${wanted}"`);
  }
});

test("RFC 0011 reads as one design: no list of decisions, options or questions", () => {
  const titles = headings(rfc()).map((h) => h.trim().toLowerCase());
  for (const gone of ["decisions", "rejected options", "open questions"]) {
    assert.ok(!titles.includes(gone), `${RFC} has a "${gone}" section`);
  }
  const text = prose(rfc()).split("\n## History\n")[0];
  assert.doesNotMatch(text, /\brecommend/i, "a recommendation is left");
  assert.doesNotMatch(text, /\bdraft\b/i, "draft wording is left");
  assert.doesNotMatch(text, /\b(?:Decision|Q)\s?\d\b/, "a question's number");
});

test("1: a typed FieldType trait the core wraps, with decode_state and write", () => {
  const text = section(rfc(), "the-field-type-a-plugin-implements");
  decided(text, "The field type a plugin implements");
  holds(
    text,
    [
      /`FieldType`/,
      /`decode_state`/,
      /`write`/,
      /`configure`/,
      /round-trip/,
      /below 64/,
      /`UnknownOperation`/,
      /4 to 63/,
      // not taken: a trait over bytes, and closed enums per app
      /over bytes/,
      /by hand/,
      /macro|feature flags?/,
    ],
    "The field type a plugin implements",
  );
});

test("the data a plugin reads and writes is one type, RFC 0002's CBOR subset", () => {
  const text = section(rfc(), "the-data-a-plugin-reads-and-writes");
  holds(
    text,
    [
      /`Data`/,
      /null/,
      /bytes/,
      /map/,
      /0002-change-and-log-format\.md#the-cbor-profile/,
    ],
    "The data a plugin reads and writes",
  );
});

test("2: one crate is one namespace; the registry is keyed by type id and rule", () => {
  const text = section(rfc(), "a-plugin-and-the-registry");
  decided(text, "A plugin and the registry");
  holds(
    text,
    [
      /`plugin\(\)`/,
      /`Plugin`/,
      /type id and rule/,
      /two plugins for one namespace/,
      /`ncdb`/,
      /every rule version it (?:ever )?shipped/,
      // not taken: one type at a time, and registration at link time
      /`register::<Counter>\(\)`/,
      /`inventory`|`linkme`/,
    ],
    "A plugin and the registry",
  );
});

test("3: OpenOptions::plugin, callable more than once", () => {
  const text = section(rfc(), "handing-plugins-to-the-core");
  decided(text, "Handing plugins to the core");
  holds(
    text,
    [
      /`OpenOptions::plugin`/,
      /more than once/,
      /`View::open`/,
      // not taken: one set built beforehand
      /`OpenOptions::plugins/,
    ],
    "Handing plugins to the core",
  );
});

test("4: no plugins in TypeScript in v1, ncdb core build next", () => {
  const text = section(rfc(), "typescript");
  decided(text, "TypeScript");
  holds(
    text,
    [
      /`UnknownType`/,
      /`ncdb core build`/,
      /`nc\.open\(\{ core \}\)`/,
      /Rust toolchain/,
      // not taken for now: wasm modules every core loads at run time
      /wasm modules?/,
      /wasmi|interpreter/,
      /RFC 0004/,
    ],
    "TypeScript",
  );
});

test("5: a type that is not registered opens, parks as UnknownType, refuses writes", () => {
  const text = section(rfc(), "a-type-that-is-not-registered");
  decided(text, "A type that is not registered");
  holds(
    text,
    [
      /`ParkReason::UnknownType`/,
      /reads as unset/,
      /`unknownType`/,
      /rule version (?:that )?differs|different rule version/,
      /`config`/,
      // not taken: refusing to open, and a debug-only refusal
      /`modelRefused`/,
      /debug builds?/,
    ],
    "A type that is not registered",
  );
});

test("6: the harness is a public crate, ncdb-harness", () => {
  const text = section(rfc(), "checking-a-plugin-ncdb-harness");
  decided(text, "Checking a plugin: ncdb-harness");
  holds(
    text,
    [
      /`ncdb-harness`/,
      /dev-dependency/,
      /wasm32/,
      /cannot prove/,
      // not taken: a CLI command, and a check on every registration
      /`ncdb plugin check`/,
      /every (?:registration|open)/,
    ],
    "Checking a plugin: ncdb-harness",
  );
});

test("7: what a plugin depends on is ncdb-plugin, not ncdb-core", () => {
  const text = section(rfc(), "what-a-plugin-depends-on-ncdb-plugin");
  decided(text, "What a plugin depends on: ncdb-plugin");
  holds(
    text,
    [
      /`ncdb-plugin`/,
      /`Origin`/,
      /`OperationId`/,
      /`MalformedArguments`/,
      // not taken: a public module inside ncdb-core
      /public module/,
    ],
    "What a plugin depends on: ncdb-plugin",
  );
});

test("no column for a plugin field in v1, and what a plugin can reach", () => {
  holds(
    section(rfc(), "the-view-keeps-no-column-for-a-plugin-field"),
    [/fs_<collection>/, /cannot be indexed/],
    "The view keeps no column for a plugin field",
  );
  holds(
    section(rfc(), "what-a-plugin-can-reach"),
    [/`\[redacted\]`/, /trusted like any (?:other )?dependency/, /relay/],
    "What a plugin can reach",
  );
});

test("RFC 0004 and RFC 0005 point to RFC 0011 where they waited for a plugin RFC", () => {
  const contract = section(
    read("docs/rfc/0004-merge-rules-basic-types.md"),
    "the-plugin-contract",
  );
  assert.doesNotMatch(contract, /until a plugin RFC/);
  assert.match(contract, /\(0011-plugin-field-types\.md(?:#[^)]*)?\)/);
  const lock = read("docs/rfc/0005-schema-and-model-lock.md");
  const notCovered = section(lock, "what-this-rfc-does-not-cover");
  assert.doesNotMatch(notCovered, /belongs to the plugin RFC/);
  assert.match(notCovered, /\(0011-plugin-field-types\.md(?:#[^)]*)?\)/);
  assert.match(
    section(lock, "refused"),
    /^\| Change a plugin field's `config` \|.*NOC-232/m,
  );
});

test("RFC 0009 takes ParkReason::UnknownType, unknownType and OpenOptions::plugin", () => {
  const sdk = read("docs/rfc/0009-sdk-api.md");
  const rust = section(sdk, "the-rust-sdk");
  assert.match(
    rust,
    /RFC 0005's `UnknownCollection`, `UnknownField` and `UnknownType`/,
  );
  assert.doesNotMatch(rust, /which no core reason produces/);
  assert.match(section(sdk, "errors"), /^\| `unknownType` \|/m);
  const names = section(sdk, "public-names");
  assert.match(
    names,
    /^\| `OpenOptions::plugin` \|.*0011-plugin-field-types\.md/m,
  );
  assert.match(names, /`unknownType`/);
  for (const text of [rust, section(sdk, "errors")]) {
    assert.ok(text.includes(BY_OWNER), "RFC 0009 does not name the decision");
  }
});

test("the decision log records the owner's decision on RFC 0011", () => {
  const entry = readdirSync(join(root, "factory/decisions"))
    .filter((f) => /^2026-11-12-NOC-232-/.test(f))
    .map((f) => read(`factory/decisions/${f}`))
    .find((t) => /^# [^\n]*· The owner decides RFC 0011, /m.test(t));
  assert.ok(entry, "no NOC-232 decision file names RFC 0011 as decided");
  assert.match(entry, /^- Decided by: Carsten, 2026-11-12, on NOC-232/m);
  assert.match(entry, /^- Topics: .*rfc-0011/m);
});

test("History holds one line: the first publication", () => {
  const lines = section(rfc(), "history")
    .split("\n")
    .filter((l) => l.startsWith("- "));
  assert.equal(lines.length, 1);
  assert.match(lines[0], /^- 2026-11-12 · First published .*NOC-235/);
});
