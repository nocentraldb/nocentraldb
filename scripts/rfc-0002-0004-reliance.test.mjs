// RFC 0004 states what its merge rules rely on after the rules themselves,
// and RFC 0002 states each guarantee in the section it belongs to, with no
// section of answers to another RFC (Carsten, 2026-11-20, NOC-362).
import assert from "node:assert/strict";
import { test } from "node:test";
import { headings, read, section } from "./lib/rfc-text.mjs";

const RFC_0002 = "docs/rfc/0002-change-and-log-format.md";
const RFC_0004 = "docs/rfc/0004-merge-rules-basic-types.md";

test("RFC 0004 says what its merge rules rely on, after References", () => {
  const found = headings(read(RFC_0004));
  const at = found.indexOf("What the merge rules rely on");
  assert.ok(at > 0, "no section What the merge rules rely on");
  assert.equal(found[at - 1], "What this does not do");
  assert.equal(found[at + 1], "The plugin contract");
  const part = section(read(RFC_0004), "what-the-merge-rules-rely-on");
  assert.match(part, /\| #\s+\| Property\s+\| Why this RFC needs it\s+\|\n/);
  assert.match(part, /guarantees all five\./);
});

test("RFC 0002 has no section of answers to RFC 0003 and RFC 0004", () => {
  const text = read(RFC_0002);
  assert.ok(
    !headings(text).includes("What RFC 0003 and RFC 0004 need from this RFC"),
  );
  assert.match(
    text,
    /0004-merge-rules-basic-types\.md#what-the-merge-rules-rely-on/,
  );
  assert.match(
    section(text, "the-cbor-profile"),
    /a set may hold both 0\.0 and −0\.0/,
  );
  assert.match(
    section(text, "the-clock"),
    /stamps far in the future are not refused/,
  );
});
