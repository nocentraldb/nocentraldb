// Asserts that the browser getting-started guide,
// docs/getting-started-browser.md, says where a database lives (NOC-327;
// the owner, 2026-11-15), as RFC 0009 ("Open and close") and RFC 0006 ("The
// browser", "Durability of a browser copy") do, and that its list of what
// the browser SDK does not do yet names only what is still missing.
// scripts/doc-snippets.test.mjs type-checks its snippets and checks that
// they show each call.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const guide = readFileSync("docs/getting-started-browser.md", "utf8");

/** The section under `heading`, without its heading line, to the next heading of its level or above. */
const section = (heading) => {
  const start = guide.search(new RegExp(`^#{2,3} ${heading}$`, "m"));
  assert.ok(start >= 0, `the guide has no section ${heading}`);
  const level = guide.slice(start).match(/^#+/)[0].length;
  const rest = guide.slice(guide.indexOf("\n", start) + 1);
  const end = rest.search(new RegExp(`^#{1,${level}} `, "m"));
  return end < 0 ? rest : rest.slice(0, end);
};

test("the database lives in the origin's OPFS, under the name the app gives, where the browser decides", () => {
  const text = section("Where the database lives");
  assert.match(text, /OPFS/);
  assert.match(text, /`name`/);
  assert.match(text, /browser decides/);
});

test("the keys are in IndexedDB, sealed under a non-extractable WebCrypto key", () => {
  const text = section("Where the database lives");
  assert.match(text, /IndexedDB/);
  assert.match(text, /non-extractable WebCrypto key/);
});

test("the browser may evict an origin's data, and persistentStorage and db.durability address it", () => {
  const text = section("Where the database lives");
  assert.match(text, /evict/);
  assert.match(text, /`persistentStorage`/);
  assert.match(text, /`db\.durability`/);
});

test("the section cites RFC 0009 and RFC 0006", () => {
  const text = section("Where the database lives");
  assert.match(text, /\(rfc\/0009-sdk-api\.md#open-and-close\)/);
  assert.match(text, /\(rfc\/0006-local-view\.md#the-browser\)/);
  assert.match(
    text,
    /\(rfc\/0006-local-view\.md#durability-of-a-browser-copy\)/,
  );
});

test("the list of what the browser SDK does not do yet names none of the calls it has", () => {
  const text = section("What the browser SDK does not do yet");
  for (const name of [
    "db.view",
    "db.durability",
    "spaces.subscribe",
    "leave()",
    "erasures",
    "lockOutOfDate",
  ]) {
    assert.ok(!text.includes(name), `the list still names ${name}`);
  }
  assert.match(text, /space\.inspect/);
});
