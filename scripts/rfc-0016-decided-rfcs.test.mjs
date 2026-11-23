// RFC 0016's move takes operation 6, since RFC 0013's rich-text update holds 5,
// and the decided RFCs it builds on describe the move (NOC-474). These tests
// pin each place the ticket names.
import assert from "node:assert/strict";
import { test } from "node:test";
import { decisionEntries } from "./lib/rfc-record.mjs";
import { read, section } from "./lib/rfc-text.mjs";

const rfc0016 = () => read("docs/rfc/0016-moving-an-object.md");

test("RFC 0016's move is operation 6, and its Decisions record the correction", () => {
  const formats = section(rfc0016(), "formats");
  assert.match(formats, /\*\*Move, operation 6\*\*/);
  assert.match(formats, /4: 6, 5: \{1: destination, 2: replaces\}/);
  assert.match(
    formats,
    /numbers 0, 4 and 6 belong to the object, and 5 is rich text/,
  );
  assert.doesNotMatch(rfc0016(), /operation 5\b/);
  const entry = decisionEntries(rfc0016()).find((e) => /operation 6/.test(e));
  assert.ok(entry, "no Decisions entry records operation 6");
  assert.match(
    entry,
    /0, 4 and 6 belong to the object, and 5 is the rich-text update/,
  );
});

test("RFC 0016 computes later edits from the log and keeps space.inspect out of them", () => {
  const part = section(rfc0016(), "later-edits");
  assert.doesNotMatch(part, /space\.inspect/);
  assert.match(part, /computes the later edits from the log when the app asks/);
  assert.match(
    rfc0016(),
    /`space\.moves\(\)` with `later_edits\(\)`, `carry_over\(&edits\)` and `conflicts\(\)`/,
  );
});

test("RFC 0016 and RFC 0009 refuse a set plugin field for now, deciding nothing", () => {
  for (const file of [
    "docs/rfc/0016-moving-an-object.md",
    "docs/rfc/0009-sdk-api.md",
  ]) {
    const text = read(file);
    assert.match(
      text,
      /plugin field (that )?holds a value[^.]*refused for now|for now, an object whose plugin field holds a value/,
      file,
    );
    assert.match(text, /`error\.movePluginLater`/, file);
    assert.match(text, /NOC-489/, file);
  }
});

test("RFC 0004 lists 5 as the rich-text update, 6 as the move, and reserves 7 to 63", () => {
  const rfc = read("docs/rfc/0004-merge-rules-basic-types.md");
  const table = section(rfc, "operation-numbers");
  assert.match(
    table,
    /^\| 5 +\| \*\*rich-text update\*\* +\| rich text \(`ncdb\/richText`\)/m,
  );
  assert.match(table, /^\| 6 +\| \*\*move\*\* /m);
  assert.match(table, /^\| 7 to 63 +\| reserved /m);
  assert.match(table, /\*\*Numbers 0, 4 and 6 belong to the object\*\*/);
  assert.match(
    read("docs/rfc/0013-rich-text.md"),
    /gives number 5 to this RFC's rich-text update and reserves 7 to 63/,
  );
  assert.match(section(rfc, "resolution"), /\| \*\*moved\*\* /);
  assert.match(
    section(rfc, "how-a-reference-resolves"),
    /follows the trace into the space it names[^.]*up to 8 in a row/,
  );
});

test("RFC 0005 has $movedFrom, RFC 0003 points to RFC 0016's notice, RFC 0002 lists the move", () => {
  assert.match(
    read("docs/rfc/0005-schema-and-model-lock.md"),
    /`\$movedFrom` is set in the new space's change/,
  );
  const rfc0003 = read("docs/rfc/0003-keys-and-cryptography.md");
  assert.match(
    rfc0003,
    /returns the notice `objectMoved`, which \[RFC 0016\]\(0016-moving-an-object\.md#the-sdk-call-and-its-notice\)/,
  );
  assert.doesNotMatch(
    rfc0003,
    /notices of moving data between spaces and of invitations come later, with/,
  );
  const rfc0002 = read("docs/rfc/0002-change-and-log-format.md");
  assert.match(rfc0002, /4 is delete; and 6 is move/);
  assert.match(rfc0002, /Number 5 is rich text's update/);
});

test("RFC 0009 names the move's calls, error, notice and link status", () => {
  const rfc = read("docs/rfc/0009-sdk-api.md");
  const part = section(rfc, "moving-an-object");
  for (const name of [
    "`space.move(objects, destination)`",
    "`space.moves`",
    "`laterEdits()`",
    "`carryOver(edits)`",
    "`conflicts()`",
    "`objectMoved`",
    "`moved`",
  ]) {
    assert.ok(part.includes(name), `Moving an object does not name ${name}`);
  }
  assert.doesNotMatch(part, /space\.inspect/);
  assert.match(rfc, /`"wrongCollection"` or `"moved"`/);
  assert.match(rfc, /^\| `moved` \| writes \|/m);
  assert.match(
    rfc,
    /`space\.moves\(\)` returns `Moves`, with `later_edits\(\)`, `carry_over\(&edits\)` and `conflicts\(\)`/,
  );
});
