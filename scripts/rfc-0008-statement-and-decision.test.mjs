// Holds the owner's answers on NOC-187 and NOC-230 (Carsten, 2026-11-11) as
// RFC 0008, RFC 0007 and RFC 0009 record them: the account key statement
// entry is control entry type 7, which any member may write and which a
// concurrent add with a superseded account key loses to; the erasure
// decision is type 8, which finalizes a provisional erasure of a space with
// one account or takes it back; the relay refuses a finalize that comes less
// than 7 days after the shred with `too_early`; and the SDK reports
// provisional erasures with their notice. The byte-exact examples in RFC
// 0008 must equal the vectors that crates/ncdb-core/tests/vectors/membership
// /generate.py writes with independent libraries. Whether the wording is
// right is the review of the PR.
import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { read, root, section } from "./lib/rfc-text.mjs";

const RFC_0007 = "docs/rfc/0007-relay.md";
const RFC_0008 = "docs/rfc/0008-membership.md";
const RFC_0009 = "docs/rfc/0009-sdk-api.md";
const VECTORS = "crates/ncdb-core/tests/vectors/membership";

const hex = (name) => read(`${VECTORS}/${name}.hex`).replace(/\s/g, "");

test("the control bodies table names entry types 7 and 8 with the rights their authors need", () => {
  const bodies = section(read(RFC_0008), "the-control-bodies");
  assert.match(
    bodies,
    /^\| 7 \| Account key statement entry \| reader \(Carsten, 2026-11-11, NOC-187\) \|$/m,
  );
  assert.match(
    bodies,
    /^\| 8 \| Erasure decision \| admin, of the provisional shred's performer's account \(Carsten, 2026-11-11, NOC-230\) \|$/m,
  );
});

test("RFC 0008 gives the account key statement entry, its rules and its example bytes", () => {
  const entry = section(read(RFC_0008), "account-key-statement-entry");
  assert.match(entry, /account key statement entry \(entry type 7\)/);
  assert.match(entry, /8 {2}statement/);
  assert.match(entry, /Any member may carry it, a reader too/);
  assert.match(
    entry,
    /higher than the number of every statement for that recovery key/,
  );
  assert.match(
    entry,
    /Only members of the same account writing it was not taken/,
  );
  const body = hex("statement-entry.body");
  assert.equal(body.length, 360, "the body is 180 bytes");
  assert.ok(
    entry.replace(/\s/g, "").includes(body),
    "the example is not the vector's bytes",
  );
});

test("RFC 0008 gives the erasure decision and its example bytes, both ways", () => {
  const entry = section(read(RFC_0008), "erasure-decision");
  assert.match(entry, /erasure decision \(entry type 8\)/);
  assert.match(entry, /9 {2}final/);
  assert.match(entry, /Any admin of the space was not taken/);
  const text = entry.replace(/\s/g, "");
  for (const name of [
    "erasure-decision-finalize.body",
    "erasure-decision-take-back.body",
  ]) {
    const body = hex(name);
    assert.equal(body.length, 152, `${name} is 76 bytes`);
    assert.ok(text.includes(body), `the example is not the bytes of ${name}`);
  }
});

test("the vectors of both entries are committed beside their generator", () => {
  const generator = read(`${VECTORS}/generate.py`);
  for (const name of [
    "statement-2",
    "statement-entry.body",
    "erasure-decision-finalize.body",
    "erasure-decision-take-back.body",
  ]) {
    assert.match(
      generator,
      new RegExp(`write\\("${name.replace(".", "\\.")}"`),
    );
    assert.ok(
      existsSync(join(root, VECTORS, `${name}.hex`)),
      `${name}.hex is missing`,
    );
  }
});

test("the fold supersedes an older account key, and a finalize wins over a take-back", () => {
  const fn = section(read(RFC_0008), "the-function");
  assert.match(fn, /An account key K is \*\*superseded\*\*/);
  assert.match(
    fn,
    /An add that did not see an account key statement entry is rejected/,
  );
  assert.match(
    fn,
    /a finalize wins over every concurrent take-back \(Carsten, 2026-11-11, NOC-230\)/,
  );
});

test("RFC 0008 gives the provisional erasure: final is a change in the log, the relay's gate, a second account", () => {
  const provisional = section(
    read(RFC_0008),
    "a-provisional-erasure-in-a-space-with-one-account",
  );
  assert.match(provisional, /Final is a change in the log, not a timer/);
  assert.match(provisional, /`too_early`/);
  assert.match(provisional, /A second account joins within the 7 days/);
  assert.match(provisional, /An app must show it/);
  assert.match(
    provisional,
    /Each device counting 7 days and deleting the key on its own/,
  );
});

test("the harness gains M-C13 and M-C14", () => {
  const harness = section(
    read(RFC_0008),
    "how-the-convergence-harness-checks-it",
  );
  assert.match(
    harness,
    /^\| M-C13 \| A finalize wins over every concurrent take-back\. \|/m,
  );
  assert.match(
    harness,
    /^\| M-C14 \| A newer account key statement wins over a concurrent add\. \|/m,
  );
});

test("RFC 0007 lists `too_early` and the relay's check of the 7 days", () => {
  const text = read(RFC_0007);
  assert.match(
    text,
    /^\| `too_early` \| An erasure decision that finalizes a provisional shred/m,
  );
  assert.match(
    text,
    /already keeps as "accepted at" \(Carsten, 2026-11-11, NOC-230\)/,
  );
});

test("RFC 0009 names the provisional erasures and their notice", () => {
  const text = read(RFC_0009);
  assert.match(text, /^\| `provisionalErasures` \|/m);
  assert.match(text, /^\| `ProvisionalErasure` \|/m);
  assert.match(text, /^\| `erasurePending` \|/m);
});
