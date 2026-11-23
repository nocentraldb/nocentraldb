// Asserts that the memory budget of one change, as the owner decided it
// (Carsten, 2026-11-23, NOC-451, question 1 A; NOC-479), stands where a
// developer plans against it: docs/bulk-import.md and RFC 0007's "Large
// changes" state the same sentence, the RFC records it under Decisions with
// its decision file and in History, and the guide says that a phone was not
// measured and that the guard runs only for releases.
import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { decisionEntries, historyLines } from "./lib/rfc-record.mjs";
import { read, root, section } from "./lib/rfc-text.mjs";

const BUDGET =
  "A change at the 16 MiB limit fits in 256 MiB of core memory, natively and in a browser tab's WebAssembly memory.";
const DECISION =
  "2026-11-23-NOC-479-a-change-at-the-16-mib-limit-fits-in-256-mib-of-core-memory.md";
const GUIDE = "docs/bulk-import.md";
const RFC = "docs/rfc/0007-relay.md";

test("the decision file records the budget", () => {
  assert.ok(existsSync(join(root, "factory/decisions", DECISION)), DECISION);
  const text = read(`factory/decisions/${DECISION}`);
  assert.match(text, /- Decided by: Carsten/);
  assert.ok(text.includes("256 MiB"));
});

test("the bulk import guide states the budget, its guard, and that a phone was not measured", () => {
  const near = section(read(GUIDE), "a-change-near-the-limit");
  assert.ok(near.includes(BUDGET), `${GUIDE} states: ${BUDGET}`);
  assert.match(
    near,
    /A phone was not measured\. The budget rests on native and Chromium\./,
  );
  assert.match(near, /release-workflow|release\.yml/);
  assert.match(near, /memory_budget\.rs/);
  assert.match(near, /memory-budget\/browser\.test\.mjs/);
  assert.match(near, /beta and stable/);
});

test("RFC 0007's limits state the budget once, in Large changes", () => {
  const text = read(RFC);
  assert.ok(section(text, "large-changes").includes(BUDGET));
  assert.equal(text.split(BUDGET).length, 2, "the sentence stands once");
});

test("RFC 0007 records the budget under Decisions and in History", () => {
  const text = read(RFC);
  const entry = decisionEntries(text).find((e) => e.includes(DECISION));
  assert.ok(entry, "a Decisions entry links the decision file");
  assert.match(entry, /^- \*\*2026-11-23 · /);
  assert.doesNotMatch(
    entry,
    /\b(Carsten|pm)\b/,
    "the decision log names who decided",
  );
  assert.ok(
    historyLines(text).some(
      (l) => l.startsWith("- 2026-11-23 · ") && l.includes("NOC-479"),
    ),
    "History has a line for NOC-479",
  );
});
