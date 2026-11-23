// NOC-379: four gaps that NOC-362's review (#389) found, where the decision
// log goes beyond an RFC without contradicting it. Each RFC now states what
// the listed decisions decided, in the passage where it belongs, with an
// entry under "Decisions" and a History line:
// - RFC 0010, the account space: NOC-266, NOC-310 and NOC-311;
// - RFC 0009's error table: NOC-169, NOC-177 and NOC-267;
// - RFC 0005: NOC-57 and NOC-251;
// - RFC 0011: NOC-318's plugin fields in ad-hoc copies.
import assert from "node:assert/strict";
import { test } from "node:test";
import { decisionEntries, historyLines } from "./lib/rfc-record.mjs";
import { read, section } from "./lib/rfc-text.mjs";

const rfc = (n) =>
  read(
    {
      "0005": "docs/rfc/0005-schema-and-model-lock.md",
      "0009": "docs/rfc/0009-sdk-api.md",
      "0010": "docs/rfc/0010-accounts.md",
      "0011": "docs/rfc/0011-plugin-field-types.md",
    }[n],
  );

/** Asserts that the RFC's Decisions name each ticket and History names NOC-379. */
const recorded = (n, tickets) => {
  const entries = decisionEntries(rfc(n)).join("\n");
  for (const ticket of tickets) {
    assert.match(
      entries,
      new RegExp(`\\b${ticket}\\b`),
      `RFC ${n}: no Decisions entry names ${ticket}`,
    );
  }
  assert.ok(
    historyLines(rfc(n)).some((l) => /\bNOC-379\b/.test(l)),
    `RFC ${n}: no History line for NOC-379`,
  );
};

test("RFC 0010: a device creates the account space with its first app space, and a paired one creates none", () => {
  const text = section(rfc("0010"), "the-account-space");
  assert.match(text, /with the first app space it creates/);
  assert.match(text, /not when (the database|it) opens/);
  assert.match(text, /joins an account[^.]*creates none/);
});

test("RFC 0010: an account-space record's id derives from what it records, and each collection has one index", () => {
  const text = section(rfc("0010"), "the-account-space");
  assert.match(text, /first 16 bytes of SHA-256/);
  assert.match(text, /"nocentraldb\/account-member"/);
  assert.match(text, /"nocentraldb\/account-membership"/);
  for (const index of ["byMemberId", "bySpace", "byAccount", "byNumber"]) {
    assert.match(text, new RegExp(`\`${index}\``), index);
  }
});

test("RFC 0010: the view names the account collections with a `$` in front", () => {
  const text = section(rfc("0010"), "the-account-space");
  for (const name of [
    "$members",
    "$memberships",
    "$links",
    "$statements",
    "$wrappedKeys",
  ]) {
    assert.ok(text.includes(`\`${name}\``), name);
  }
});

test("RFC 0010: the account space is the one whose creator is the current account", () => {
  const text = section(rfc("0010"), "the-account-space");
  assert.match(text, /genesis creator[^.]*current account/);
  assert.match(text, /by fingerprint/);
});

test("RFC 0010: a linked or paired database creates no account space before the account's arrives", () => {
  const text = section(rfc("0010"), "the-account-space");
  assert.match(text, /keeps a link statement[^.]*creates no account space/);
  assert.match(text, /`joined`/);
  assert.match(text, /recovered device writes no/);
  recorded("0010", ["NOC-266", "NOC-310", "NOC-311"]);
});

test("RFC 0009's error table names the codes of NOC-169, NOC-177 and NOC-267", () => {
  const text = section(rfc("0009"), "errors");
  assert.match(text, /`members\.remove`[^|]*itself[^|]*`space\.leave/);
  assert.match(text, /not a current member of the space/);
  assert.match(text, /last admin[^|]*lowers its own role/);
  assert.match(text, /waiting to be paired[^|]*creates no space/);
  assert.match(text, /recovery code[^|]*database that exists/);
  assert.match(text, /may or may not have run/);
  assert.match(text, /lock differs from the owning tab's/);
  recorded("0009", ["NOC-169", "NOC-177", "NOC-267"]);
});

test("RFC 0005: every sealed change of a space of another model parks, whatever its key 64 says", () => {
  const text = section(rfc("0005"), "data-from-a-newer-model");
  assert.match(
    text,
    /genesis names another `model\.id`[^.]*every sealed change[^.]*parks?/,
  );
  assert.match(text, /control changes[^.]*still fold/i);
});

test("RFC 0005: a plugin field's config that appears or disappears is a changed config", () => {
  const text = section(rfc("0005"), "refused");
  assert.match(text, /appears or disappears/);
  recorded("0005", ["NOC-57", "NOC-251"]);
});

test("RFC 0011: an ad-hoc copy leaves a plugin field unset and counts it", () => {
  const text = section(rfc("0011"), "what-this-does-not-do");
  assert.match(text, /ad-hoc/);
  assert.match(text, /unset/);
  assert.match(text, /`plugin_fields_not_copied`/);
  recorded("0011", ["NOC-318"]);
});
