// Asserts that RFC 0007 records the owner's answer on NOC-143 (option A,
// Carsten, 2026-11-10), as NOC-152 writes it into the text: after the relay
// accepts a control change that makes a member current in a space, it pushes
// `added {space}` to that member's open connections, best-effort like live
// entries; the member pulls and subscribes as for a space named in `welcome`,
// and a lost message is recovered at the next reconnect. On the wire the
// message follows decision R12: type at key 1, no request id, the space at
// key 3. The AWS backend finds a member's connections through a secondary
// index on member id in `connections`. The decision stands under "Decisions"
// as R13, and a dated History line names NOC-143 and NOC-152.
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
  assert.match(
    passage,
    /\[R13\]\(#r13-telling-a-connected-member-it-was-added\)/,
  );
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

test("decision R13 records the owner's choice on NOC-143, with the options not taken", () => {
  const r13 = section(read(RFC), "r13-telling-a-connected-member-it-was-added");
  assert.match(r13, /^Decided \(Carsten, 2026-11-10\), on NOC-143: /m);
  assert.match(r13, /^Why: /m);
  assert.match(r13, /^Not taken:\n\n- /m);
  assert.match(r13, /`spaces` request/);
  assert.match(r13, /next reconnect/);
});

test("RFC 0007's status names R13", () => {
  const status = read(RFC)
    .split("\n")
    .find((l) => l.startsWith("**Status.**"));
  assert.match(status, /R13/);
});

test("History has a line for NOC-152 that names NOC-143", () => {
  const entry = section(read(RFC), "history")
    .split("\n")
    .find((l) => l.startsWith("- ") && l.includes(" · NOC-152 · "));
  assert.ok(entry, "History has no line for NOC-152");
  assert.match(entry, /^- \*\*2026-11-10\*\* · NOC-152 · /);
  assert.match(entry, /NOC-143/);
});
