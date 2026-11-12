// Holds the owner's answers on NOC-231 (Carsten, 2026-11-12, A for all
// five) as RFC 0002 and RFC 0006 record them (NOC-175, the database on
// OPFS): where a database lives on OPFS, which names it may have, what an
// open does when another tab holds the pool, who asks for persistent
// storage, and the log's `written_at`, which the durability report's
// oldest unconfirmed time reads. Whether the wording is right is the
// review of the PR.
import assert from "node:assert/strict";
import { test } from "node:test";
import { read, section } from "./lib/rfc-text.mjs";

const RFC_0002 = "docs/rfc/0002-change-and-log-format.md";
const RFC_0006 = "docs/rfc/0006-local-view.md";
const SOURCE = /Carsten, 2026-11-12, NOC-231/;

test("RFC 0002's log holds written_at, set for the member's own changes", () => {
  const log = section(read(RFC_0002), "on-a-member");
  assert.match(log, /written at\s+integer/);
  assert.match(log, /milliseconds since the Unix epoch/);
  assert.match(log, /null for every change it received/i);
  assert.match(log, SOURCE);
});

test("RFC 0006 reads oldestUnconfirmedAt from written_at", () => {
  const reports = section(read(RFC_0006), "what-the-sdk-reports");
  assert.match(reports, /`written_at`/);
  assert.match(reports, SOURCE);
});

test("RFC 0006 gives a database's place on OPFS and the names it may have", () => {
  const browser = section(read(RFC_0006), "the-browser");
  for (const pattern of [
    /`\.opfs-sahpool`/,
    /`\/<name>\/nocentraldb\.sqlite`/,
    /two slots/,
    /1 to 64/,
    /`a`–`z`, `0`–`9`, `-` and `_`/,
    /`default`/,
    /`invalidValue`/,
    SOURCE,
  ]) {
    assert.match(browser, pattern, `"The browser" does not say ${pattern}`);
  }
});

test("RFC 0006 says an open whose pool another tab holds fails with storageFailed", () => {
  const browser = section(read(RFC_0006), "the-browser");
  assert.match(browser, /another tab holds/);
  assert.match(browser, /`storageFailed`/);
});

test("RFC 0006 says the page asks for persistent storage, since a worker cannot", () => {
  const reports = section(read(RFC_0006), "what-the-sdk-reports");
  assert.match(reports, /`persist\(\)`/);
  assert.match(reports, /dedicated worker/);
  assert.match(reports, /`persisted\(\)`/);
  assert.match(reports, SOURCE);
});
