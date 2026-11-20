// Asserts that RFC 0007 records the owner's answer on NOC-383 (option A,
// Carsten, 2026-11-20): the error that ends a removed member's subscription,
// or refuses its pull or subscribe, carries the entry of the control change
// that removed it, at key 5, and nothing else of the space; the member folds
// it, as RFC 0008 ("A member removed while offline who keeps writing", "On
// the laptop itself") promises. The Decisions entry links its decision file.
import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { test } from "node:test";
import { read, section } from "./lib/rfc-text.mjs";

const RFC = "docs/rfc/0007-relay.md";

test("Subscribe says the relay ends a removed member's subscription with its removal", () => {
  const text = section(read(RFC), "subscribe-follow-a-space-live");
  assert.match(
    text,
    /`removed_member`, which carries the control change that removes it/,
  );
  assert.match(text, /nothing else of the space/);
});

test("The errors says which error carries a removal, and what the member does with it", () => {
  const text = section(read(RFC), "the-errors");
  assert.match(text, /5: removal/);
  assert.doesNotMatch(text, /An error never carries a frame/);
  const rows = text
    .split("\n")
    .filter((l) => /^\| `(not_member|removed_member)` \|/.test(l));
  assert.equal(rows.length, 2, "rows for not_member and removed_member");
  for (const row of rows) {
    assert.match(row, /fold/i, `the member folds its removal: ${row}`);
  }
});

test("Decisions records NOC-383 with its decision file, and History a dated line", () => {
  const decisions = section(read(RFC), "decisions");
  const entry = decisions.split("\n").find((l) => l.includes("NOC-383"));
  assert.ok(entry, "no Decisions entry for NOC-383");
  assert.match(entry, /Carsten, 2026-11-20/);
  const file = entry.match(/\]\(\.\.\/\.\.\/(factory\/decisions\/[^)]+)\)/);
  assert.ok(file, `the entry links no decision file: ${entry}`);
  assert.ok(existsSync(file[1]), `${file[1]} is missing`);
  const history = section(read(RFC), "history");
  assert.match(history, /^- 2026-11-20 · .*NOC-383/m);
});
