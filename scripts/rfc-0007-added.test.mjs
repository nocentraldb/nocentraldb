// Asserts that RFC 0007 records the owner's answer on NOC-143 (option A,
// Carsten, 2026-11-10), as NOC-152 writes it into the text: after the relay
// accepts a control change that makes a member current in a space, it pushes
// `added {space}` to that member's open connections, best-effort like live
// entries; the member pulls and subscribes as for a space named in `welcome`,
// and a lost message is recovered at the next reconnect. On the wire the
// message follows the rule of "The keys of each message": type at key 1, no
// request id, the space at key 3. The AWS backend finds a member's
// connections through a secondary index on member id in `connections`. RFC
// 0007 is a fresh text (NOC-165): rfc-0007-fresh.test.mjs holds the options
// not taken in Subscribe.
import assert from "node:assert/strict";
import { test } from "node:test";
import { read, section } from "./lib/rfc-text.mjs";

const RFC = "docs/rfc/0007-relay.md";

/** The lines of `text` that start a Markdown table row. */
const tableRows = (text) => text.split("\n").filter((l) => l.startsWith("| "));

/** The trimmed cells of a table row, backticks dropped. */
const cells = (row) =>
  row
    .slice(1, -1)
    .split("|")
    .map((c) => c.trim().replaceAll("`", ""));

test("the keys table has `added`, relay → member, with no request id and the space at key 3", () => {
  const text = section(read(RFC), "the-keys-of-each-message");
  const row = tableRows(text)
    .map(cells)
    .find((r) => r[0] === "added");
  assert.ok(row, "the keys table has no row for added");
  assert.deepEqual(row, ["added", "relay → member", "3 space"]);
  const absent = text.split("\n").find((l) => l.startsWith("- **Key 2**"));
  assert.ok(absent, "the rule for key 2 is missing");
  assert.match(absent, /`added`/, "key 2's rule does not name added");
});

test("Subscribe says when the relay sends `added` and what the member does", () => {
  const text = section(read(RFC), "subscribe-follow-a-space-live");
  assert.match(text, /^relay +→ member +added +\{space\}/m);
  const passage = text
    .split("\n\n")
    .find((p) => !p.startsWith("```") && p.includes("`added`"));
  assert.ok(passage, "Subscribe does not explain added");
  assert.match(passage, /makes a member current/);
  assert.match(passage, /open connections/);
  assert.match(passage, /pulls/);
  assert.match(passage, /subscribes/);
  assert.match(passage, /`welcome`/);
  assert.match(passage, /best-effort/i);
  assert.match(passage, /next reconnect/);
});

test("Opening a connection points from `welcome` to `added`", () => {
  const text = section(read(RFC), "opening-a-connection");
  const bullet = text
    .split("\n")
    .find((l) => l.startsWith("- `welcome`") && l.includes("`added`"));
  assert.ok(bullet, "no welcome bullet names added");
});

test("the notifier sketch tells a member's open connections it was added", () => {
  const text = section(read(RFC), "the-backend-interface");
  const notifier = text.match(/pub trait Notifier \{([\s\S]*?)\n\}/);
  assert.ok(notifier, "the sketch has no Notifier");
  assert.match(
    notifier[1],
    /async fn added\(&self, member: MemberId, space: SpaceId\)\s*->\s*Result<\(\), StoreError>;/,
  );
});

test("the AWS backend looks up a member's connections through an index on member id", () => {
  const aws = section(read(RFC), "the-aws-backend");
  const row = tableRows(aws).find((l) => l.startsWith("| `connections` |"));
  assert.ok(row, "the AWS backend has no connections table");
  assert.match(row, /secondary index on member id/);
  const delivery = section(read(RFC), "delivery-to-subscribers");
  assert.match(delivery, /`added`/);
  assert.match(delivery, /index on member id/);
});

test("What it costs accounts for the index on member id and for `added`", () => {
  const costs = section(read(RFC), "what-it-costs");
  assert.match(costs, /index on member id/);
  assert.match(costs, /`added`/);
});
