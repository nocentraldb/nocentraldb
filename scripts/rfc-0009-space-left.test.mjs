// Holds the owner's answer on NOC-219 (Carsten, 2026-11-11, option A) as RFC
// 0009 records it (NOC-216): the text of the notice `space.leave()` returns,
// `spaceLeft`, which says what leaving does not do. It stands where the call
// is described, with its source. Whether the wording is right is the review
// of the PR.
import assert from "node:assert/strict";
import { test } from "node:test";
import { read, section } from "./lib/rfc-text.mjs";

const RFC = "docs/rfc/0009-sdk-api.md";
const TEXT =
  "This member keeps its copy of everything it received before now. Until an admin rotates the space key, the others write under a key it still holds.";

/** The line of `text` that starts with `start`, which must exist. */
const line = (text, start) => {
  const found = text.split("\n").find((l) => l.startsWith(start));
  assert.ok(found, `no line starts with "${start}"`);
  return found;
};

test("the spaces example shows the notice of leave with its code and text", () => {
  const spaces = section(read(RFC), "spaces");
  assert.ok(
    spaces.includes(`left.notice; // { code: "spaceLeft", text: "${TEXT}" }`),
    "the example does not show leave's notice",
  );
});

test("space.leave() names its notice spaceLeft, with the owner's text and its source", () => {
  const leave = line(section(read(RFC), "spaces"), "- **`space.leave()`**");
  assert.match(leave, /`spaceLeft`/);
  assert.ok(leave.includes(TEXT), "the leave bullet does not give the text");
  assert.match(leave, /Carsten, 2026-11-11, NOC-219/);
});
