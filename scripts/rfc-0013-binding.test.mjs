// RFC 0013, rich text, after the owner's answers on NOC-467 (Carsten,
// 2026-11-22), which follow the browser spike of the TipTap binding
// (NOC-455). The RFC is decided, so the answers are written into its text,
// recorded under "Decisions" and dated in "History" (rfc-record.mjs):
// 1. the example rebuilds the editor from its `onContentError` option, since
//    TipTap's default throws before any `editor.on("contentError")` listener;
// 2. the pause holds a write until `compositionend`, for at most 2 s;
// 3. a rebuild during a composition waits for `compositionend`, then
//    restores the focus and the selection; the owner's answer on NOC-556
//    (2026-11-27) replaced the wait by a pull deferred while composing
//    (rfc-0013-composition-pull.test.mjs), so its entry names NOC-556;
// 4. one UndoManager is kept across editors, so a rebuild keeps the undo
//    history;
// 5. the guard reloads once, then the field is read-only with a status; the
//    binding refuses an editor without `enableContentCheck`.
// The test checks that the text states each of these and no longer states
// what they replace; whether the design is right is the owner's review.
import assert from "node:assert/strict";
import { test } from "node:test";
import { assertRecord, decisionEntries } from "./lib/rfc-record.mjs";
import { read, section } from "./lib/rfc-text.mjs";

const RFC = "docs/rfc/0013-rich-text.md";
const PUBLISHED = "2026-11-22 · First published (NOC-411, #422)";
const binding = () => section(read(RFC), "an-editor-the-binding");

test("History and Decisions record the change after publication", () => {
  const rfc = read(RFC);
  assertRecord(rfc, PUBLISHED);
  const entries = decisionEntries(rfc).filter((e) => e.includes("NOC-467"));
  assert.equal(entries.length, 3, entries.join("\n"));
  for (const entry of entries) {
    assert.match(entry, /^- \*\*2026-11-22 · /);
    assert.doesNotMatch(
      entry,
      /\bCarsten\b/,
      "the decision log says who decided",
    );
    assert.match(entry, /factory\/decisions\/2026-11-22-NOC-467-/);
  }
});

test("ncEditor rebuilds from the editor's onContentError option and hands each editor to onEditor", () => {
  const text = binding();
  // NOC-538 Q4: the app gets each new editor through onEditor.
  assert.match(text, /ncEditor\(body, \{/);
  assert.match(text, /onEditor: \(next\) =>/);
  assert.doesNotMatch(text, /body\.rebuild\(editor\)/);
  // F20: the rebuild goes in the option, never in a listener.
  assert.match(text, /The rebuild goes in the editor's `onContentError` option/);
  assert.doesNotMatch(text, /editor\.on\("contentError"/);
});

test("the pause holds a write while a composition is open, for at most 2 s", () => {
  const text = binding();
  assert.match(text, /compositionend/);
  assert.match(text, /composition/i);
});

test("a rebuild waits for the composition and keeps focus, selection and undo", () => {
  const text = binding();
  assert.doesNotMatch(text, /costs the selection and the undo history/);
  assert.match(text, /UndoManager/);
  assert.match(text, /focus/);
});

test("the guard reloads once, and an editor without the content check is refused", () => {
  const text = binding();
  assert.match(text, /reloads (its `Y\.Doc` )?once/);
  assert.match(text, /read-only/);
  assert.match(text, /without `enableContentCheck`/);
});
