// RFC 0003 opens with a plain introduction to the ideas behind its keys, for
// a reader who does not know cryptography (Carsten, 2026-11-20, NOC-362).
import assert from "node:assert/strict";
import { test } from "node:test";
import { headings, read, section } from "./lib/rfc-text.mjs";

const RFC = "docs/rfc/0003-keys-and-cryptography.md";

test("RFC 0003's first section is the ideas behind it", () => {
  assert.equal(headings(read(RFC))[1], "The ideas behind this RFC");
});

test("the ideas section names each idea, and points to the sections below", () => {
  const ideas = section(read(RFC), "the-ideas-behind-this-rfc");
  assert.match(
    ideas,
    /^\nYou do not need to know cryptography to follow this RFC\./,
  );
  for (const idea of [
    "A key pair: one half to keep, one half to share.",
    "One shared key for a space.",
    "Handing a key to a new member.",
    "Rotating instead of taking back.",
    "Devices and the person behind them.",
    "Deriving keys instead of storing more.",
    "Erasing by destroying a key.",
    "Proven building blocks only.",
  ]) {
    assert.ok(ideas.includes(`- **${idea}**`), `missing: ${idea}`);
  }
  assert.match(
    ideas,
    /The sections below define each key, who holds it and what it protects\./,
  );
});
