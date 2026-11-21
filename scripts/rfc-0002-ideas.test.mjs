// RFC 0002 opens with a plain introduction to the ideas behind the format,
// for a reader who does not know cryptography (Carsten, 2026-11-20, NOC-362).
// It stands between "How to give feedback" and "What this RFC does not
// cover", and names each idea the sections below define precisely.
import assert from "node:assert/strict";
import { test } from "node:test";
import { headings, read, section } from "./lib/rfc-text.mjs";

const RFC = "docs/rfc/0002-change-and-log-format.md";

test("RFC 0002's first section is the ideas behind it", () => {
  const found = headings(read(RFC));
  assert.equal(found[1], "The ideas behind this RFC");
  assert.equal(found[2], "What this RFC does not cover");
});

test("the ideas section names each idea, and points to the sections below", () => {
  const ideas = section(read(RFC), "the-ideas-behind-this-rfc");
  assert.match(
    ideas,
    /^\nYou do not need to know cryptography to follow this RFC\./,
  );
  for (const idea of [
    "A log instead of a table.",
    "A change is one write.",
    "A signature says who wrote a change.",
    "Encryption hides what was written.",
    "A label and sealed contents.",
    "A hash is a fingerprint.",
    "Order without a trusted clock.",
    "The same bytes everywhere.",
  ]) {
    assert.ok(ideas.includes(`- **${idea}**`), `missing: ${idea}`);
  }
  assert.match(ideas, /The sections below define each of these precisely\.\n/);
});
