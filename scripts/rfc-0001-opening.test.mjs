// RFC 0001 opens with what the product is for, as the landing page's hero
// does (Carsten, 2026-11-19, NOC-362): its first heading and paragraph, and
// the last sentence of "The problem", in the owner's words.
import assert from "node:assert/strict";
import { test } from "node:test";
import { read, section } from "./lib/rfc-text.mjs";

const RFC = "docs/rfc/0001-nocentraldb.md";

test("RFC 0001 opens with what the product is for", () => {
  const text = read(RFC);
  assert.match(
    text,
    /\n## Reads like local files, syncs like shared docs\n\nNoCentralDB puts all the data right next to the people and agents who use it\. A relay, the server in the middle, keeps everyone in sync, yet it only ever sees encrypted data\.\n/,
  );
  assert.doesNotMatch(text, /For when a central database is the wrong place/);
});

test("The problem ends with the apps where the data belongs with its users", () => {
  const problem = section(read(RFC), "the-problem").trim();
  assert.ok(
    problem.endsWith(
      "This RFC is for the apps where the data belongs with the people and agents who use it.",
    ),
    problem.slice(-120),
  );
  assert.match(
    problem,
    /For many apps a central database is still the right choice\./,
  );
});
