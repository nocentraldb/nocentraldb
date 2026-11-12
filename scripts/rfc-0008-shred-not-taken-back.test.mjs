// Holds the owner's answer on NOC-242 (Carsten, 2026-11-12: B) as RFC 0008
// records it: of two accepted shreds of one person-key id, the first in the
// order that is not taken back takes effect. After a provisional erasure is
// taken back, a later shred of the same person key erases the person again,
// provisionally, with its own 7 days. The old rule, the first in the order
// takes effect and the second changes nothing, is stated nowhere any more.
// Whether the wording is right is the review of the PR.
import assert from "node:assert/strict";
import { test } from "node:test";
import { read, section } from "./lib/rfc-text.mjs";

const RFC = "docs/rfc/0008-membership.md";

const part = (anchor) => section(read(RFC), anchor);

const RULE = /the first in the order that is not taken back takes effect/;
const MARK = /Carsten, 2026-11-12, NOC-242/;

test("the function's rule for the whole set names the first shred not taken back", () => {
  const fn = part("the-function");
  assert.match(fn, RULE);
  assert.match(fn, MARK);
});

test("the function lists a take-back among the late arrivals that change the state", () => {
  const fn = part("the-function");
  const late = fn
    .split("\n")
    .filter((l) => l.startsWith("- ") && /take-back/.test(l));
  assert.ok(
    late.some((l) => /later shred/.test(l) && /takes effect/.test(l)),
    "no late arrival where a take-back lets a later shred take effect",
  );
});

test("a shred is final when accepted repeats the new rule", () => {
  const final = part("a-shred-is-final-when-accepted");
  assert.match(final, RULE);
  assert.doesNotMatch(final, /Both are accepted and destroy the same key/);
});

test("the provisional erasure says a later shred erases again with its own 7 days", () => {
  const provisional = part("a-provisional-erasure-in-a-space-with-one-account");
  assert.match(provisional, /later shred of the same person key/);
  assert.match(provisional, /its own 7 days/);
  assert.match(provisional, MARK);
});

test("a later shred is judged by the rules that hold when it is written", () => {
  const RULES = /by the rules that hold when it is written/;
  const WHILE = /while the space has one account/;
  const provisional = part("a-provisional-erasure-in-a-space-with-one-account");
  const takenBack = provisional
    .split("\n")
    .find((l) => l.startsWith("- **Taken back**"));
  assert.match(takenBack, RULES);
  assert.match(takenBack, WHILE);
  const late = part("the-function")
    .split("\n")
    .find((l) =>
      l.startsWith("- a take-back arrives of the shred that took effect"),
    );
  assert.match(late, RULES);
  assert.match(late, WHILE);
  assert.doesNotMatch(read(RFC), /since the space still has one account/);
});

test("RFC 0008 no longer states the old rule", () => {
  const text = read(RFC);
  assert.doesNotMatch(text, /the second changes nothing/);
  assert.doesNotMatch(text, /the first in the order takes effect/);
});
