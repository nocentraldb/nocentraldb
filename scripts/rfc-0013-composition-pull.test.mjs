// RFC 0013 after the owner's answer on NOC-556 (Carsten, 2026-11-27),
// which follows the spike NOC-555: TipTap ends an IME composition itself
// when it meets a node its schema lacks, before a rebuild could wait for
// compositionend, so the binding defers its pull of what the core folded
// while a composition is open, until compositionend and for at most 2 s.
// The RFC states that, no longer states that the rebuild waits for
// compositionend, and records the change under Decisions and History.
import assert from "node:assert/strict";
import { test } from "node:test";
import { decisionEntries, historyLines } from "./lib/rfc-record.mjs";
import { read, section } from "./lib/rfc-text.mjs";

const RFC = "docs/rfc/0013-rich-text.md";
const DECISION =
  "factory/decisions/2026-11-27-NOC-556-the-rich-text-binding-defers-its-pull-while-an-ime-composition-is-open-for-at-most-2-s.md";

test("the binding defers its pull while a composition is open, for at most 2 s", () => {
  const text = section(read(RFC), "an-editor-the-binding");
  assert.match(text, /defers (its|the) pull/);
  assert.doesNotMatch(text, /rebuild waits while an IME composition is open/);
  assert.doesNotMatch(read(RFC), /A rebuild waits for `compositionend`/);
});

test("Decisions and History record the change, and the decision file supersedes NOC-467's", () => {
  const rfc = read(RFC);
  const entries = decisionEntries(rfc).filter((e) => e.includes("NOC-556"));
  assert.equal(entries.length, 1, entries.join("\n"));
  assert.ok(
    historyLines(rfc).some((l) => /^- 2026-11-27 · .*NOC-556/.test(l)),
    "a History line for NOC-556",
  );
  const decision = read(DECISION);
  assert.match(decision, /Decided by: Carsten, 2026-11-27/);
  assert.match(
    decision,
    /Supersedes: 2026-11-22-NOC-467-a-rich-text-editor-rebuild-waits-for-compositionend-and-restores-focus-and-selection\.md/,
  );
});
