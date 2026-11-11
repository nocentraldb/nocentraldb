// Holds the owner's answer on NOC-189 (Carsten, 2026-11-10, A) as RFC 0006
// and RFC 0009 record it (NOC-195): the database remembers each account
// whose fingerprint a person confirmed in the table `confirmed_accounts` of
// its SQLite file, as #165 builds it; the table is no part of the view, so a
// rebuild keeps it, and the view format version stays 2. RFC 0009's "compare
// once" names the table. Whether the wording is right is the review of the PR.
import assert from "node:assert/strict";
import { test } from "node:test";
import { read, section } from "./lib/rfc-text.mjs";

const RFC_0006 = "docs/rfc/0006-local-view.md";
const RFC_0009 = "docs/rfc/0009-sdk-api.md";

const links = (text, target) =>
  [...text.matchAll(/\]\(([^)\s]+)\)/g)].some((m) => m[1].endsWith(target));

/** The first line of `text` that starts with `start`, which must exist. */
const line = (text, start) => {
  const found = text.split("\n").find((l) => l.startsWith(start));
  assert.ok(found, `no line starts with "${start}"`);
  return found;
};

test("RFC 0006 lists confirmed_accounts among the file's tables", () => {
  const where = section(read(RFC_0006), "where-the-view-lives");
  assert.match(where, /^confirmed_accounts {2,}/m);
});

test("RFC 0006 gives the table's layout, and says it is no part of the view", () => {
  const where = section(read(RFC_0006), "where-the-view-lives");
  const table = line(where, "- **`confirmed_accounts`**");
  for (const pattern of [
    /`account_key BLOB PRIMARY KEY NOT NULL`/,
    /32-byte Ed25519 public key/,
    /`confirmed_at INTEGER NOT NULL`/,
    /milliseconds since the Unix epoch/,
    /`STRICT`/,
    /first confirmation/,
    /no part of the view/,
    /`CREATE TABLE IF NOT EXISTS`/,
    /view format version stays 2/,
    /[Rr]aising the view format version to 3 was not taken/,
    /downgrade/,
    /a refused add records nothing/,
    /own account/,
  ]) {
    assert.match(
      table,
      pattern,
      `the confirmed_accounts bullet does not say ${pattern}`,
    );
  }
});

test("RFC 0006's rebuild keeps confirmed_accounts", () => {
  const how = section(read(RFC_0006), "how");
  assert.match(how, /`confirmed_accounts`/);
});

test("RFC 0009's compare once names the table in RFC 0006", () => {
  const members = section(read(RFC_0009), "invitations-and-members");
  const bullet = line(members, "- **An account new to this device**");
  assert.match(bullet, /`confirmed_accounts`/);
  assert.ok(links(bullet, "0006-local-view.md#where-the-view-lives"));
});
