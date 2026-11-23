// NOC-453: RFC 0009 follows RFC 0008 ("Roles and rights") and the shipped
// code (NOC-169, #202): any member may lower its own role with
// `members.setRole`; only an admin may change another member's role or
// remove another member (owner, 2026-11-23, NOC-453 A).
import assert from "node:assert/strict";
import { test } from "node:test";
import { decisionEntries, historyLines } from "./lib/rfc-record.mjs";
import { read, section } from "./lib/rfc-text.mjs";

const RFC = "docs/rfc/0009-sdk-api.md";

test("RFC 0009 lets any member lower its own role, as RFC 0008 does", () => {
  const text = section(read(RFC), "invitations-and-members");
  assert.doesNotMatch(text, /Only an admin may call them/);
  assert.match(text, /[Aa]ny member may lower its own role/);
  assert.match(
    text,
    /[Oo]nly an admin may (raise a role, )?change another member's role or remove another member/,
  );
});

test("RFC 0009 records NOC-453 under Decisions and in History", () => {
  const rfc = read(RFC);
  assert.ok(
    decisionEntries(rfc).some((e) => e.includes("NOC-453")),
    "no Decisions entry for NOC-453",
  );
  assert.ok(
    historyLines(rfc).some((l) => /\bNOC-453\b/.test(l)),
    "no History line for NOC-453",
  );
});
