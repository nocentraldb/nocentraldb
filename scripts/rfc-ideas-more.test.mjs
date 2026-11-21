// RFC 0004, RFC 0006 and RFC 0008 open with a plain introduction to the
// ideas behind them, as RFC 0002 and RFC 0003 do (Carsten, 2026-11-20,
// NOC-362): the RFC's first section, naming each idea.
import assert from "node:assert/strict";
import { test } from "node:test";
import { headings, read, section } from "./lib/rfc-text.mjs";

const IDEAS = {
  "docs/rfc/0004-merge-rules-basic-types.md": [
    "Everyone edits their own copy.",
    "Every copy must agree without asking.",
    "Each field type brings its own rule.",
    "The last writer wins, and the loser stays readable.",
    "An addition wins over a removal it did not see.",
    "A link keeps what was written.",
    "Nothing disappears silently.",
  ],
  "docs/rfc/0006-local-view.md": [
    "The log is the truth; the view is computed from it.",
    "The fold.",
    "Reads never wait for the network.",
    "Queries only through declared indexes.",
    "Pages from a cursor.",
    "One code base, natively and in the browser.",
    "Plaintext on the device.",
  ],
  "docs/rfc/0008-membership.md": [
    "Membership is written in the log.",
    "Three roles.",
    "Rights are checked after the fact.",
    "Everyone reaches the same verdict.",
    "A removal draws a line, the cutoff.",
    "A verdict can change when a late change arrives.",
    "Revocations win.",
  ],
};

for (const [path, ideas] of Object.entries(IDEAS)) {
  test(`${path} opens with the ideas behind it`, () => {
    const text = read(path);
    assert.equal(headings(text)[1], "The ideas behind this RFC");
    const part = section(text, "the-ideas-behind-this-rfc");
    for (const idea of ideas) {
      assert.ok(part.includes(`- **${idea}**`), `missing: ${idea}`);
    }
    assert.match(part, /The sections below define each/);
  });
}
