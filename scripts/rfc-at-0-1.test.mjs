// The RFCs say what release 0.1 builds, as the code holds it (NOC-362).
// These tests pin the facts that went stale after the RFCs were published:
// the AWS relay backend is built and deployed, the relay stores on SQLite
// locally or on DynamoDB with S3 at 0.1 (Postgres and adapters come later),
// and the owning tab of a browser database is RFC 0006's design for 0.1,
// with its Safari limit, which RFC 0012 replaces after 0.1.
import assert from "node:assert/strict";
import { test } from "node:test";
import { read, section } from "./lib/rfc-text.mjs";

const RELAY = "docs/rfc/0007-relay.md";
const VIEW = "docs/rfc/0006-local-view.md";
const DIAGRAM = "docs/rfc/0001-relay-in-the-middle.svg";

/** The visible labels of an SVG, one per <text> element. */
const labels = (svg) =>
  [...svg.matchAll(/<text[^>]*>([^<]*)<\/text>/g)].map((m) => m[1]);

test("RFC 0007 says the AWS backend is built and deployed", () => {
  const text = read(RELAY);
  assert.doesNotMatch(text, /AWS backend is not built/);
  assert.match(text, /AWS backend is built in `crates\/ncdb-relay-aws`/);
  assert.match(text, /\]\(\.\.\/relay-deploy\.md\)/);
  assert.match(text, /The Postgres backend is not built/);
});

test("RFC 0007 cites the decision file, not the retired DECISIONS.md", () => {
  assert.doesNotMatch(read(RELAY), /factory\/DECISIONS\.md/);
});

test("the relay diagram names only 0.1's storage as built", () => {
  const found = labels(read(DIAGRAM));
  assert.ok(found.includes("SQLite locally, or"), found.join(" | "));
  assert.ok(found.includes("DynamoDB with S3 on AWS"));
  const postgres = found.filter((l) => /Postgres/.test(l));
  assert.deepEqual(postgres, ["Postgres comes after 0.1"]);
  const adapter = found.filter((l) => /adapter/i.test(l));
  assert.deepEqual(adapter, ["An adapter, after 0.1"]);
});

test("RFC 0006's owning tab names Safari's limit and RFC 0012", () => {
  const browser = section(read(VIEW), "the-browser");
  assert.match(browser, /This is the design of 0\.1\. \[RFC 0012\]/);
  assert.match(browser, /`nocentraldb\/<name>\/worker`/);
  assert.match(browser, /`waitingForTab` on `db\.durability`/);
  assert.match(browser, /NOC-352/);
  assert.match(browser, /NOC-355/);
});
