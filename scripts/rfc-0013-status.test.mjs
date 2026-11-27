// RFC 0013 after the owner's answers on NOC-538 (Carsten, 2026-11-27) that
// step 4c builds into the SDK: the binding reports `body.status`
// ("shown", "notFullyShown" with a reason, "readOnly") and `onStatus`
// (Q2), and `space.richText` answers `notSupported` until a release
// carries rich text (Q5). The text states both and records Q2.
import assert from "node:assert/strict";
import { test } from "node:test";
import { decisionEntries } from "./lib/rfc-record.mjs";
import { read, section } from "./lib/rfc-text.mjs";

const RFC = "docs/rfc/0013-rich-text.md";
const binding = () => section(read(RFC), "an-editor-the-binding");

test("the binding reports its status through body.status and onStatus", () => {
  const text = binding();
  assert.match(text, /`body\.status`/);
  assert.match(text, /`onStatus`/);
  for (const status of ["shown", "notFullyShown", "readOnly"]) {
    assert.match(text, new RegExp(`\`"?${status}"?\``));
  }
});

test("a build without rich text answers notSupported", () => {
  assert.match(binding(), /`notSupported`/);
});

test("Decisions records the status (NOC-538, Q2)", () => {
  const entries = decisionEntries(read(RFC)).filter((e) =>
    e.includes("2026-11-27-NOC-538-a-rich-text-binding-reports"),
  );
  assert.equal(entries.length, 1);
});
