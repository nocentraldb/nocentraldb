// Holds the owner's answer on NOC-246 (Carsten, 2026-11-12: 1A, 2A, 3A with
// the proposed names) as RFC 0009 records it: the erasure audit is a list
// call per space, `space.erasures.list()` and `space.erasures().list()`,
// shaped like `members.list()`. Each erasure gives its shred, its state
// (final, provisional, taken back), its performer and the account that
// signed it off, and every member that could have held the key, whether it
// is still a member and what it acknowledged (deleted, seen, missing). The
// options not taken stand beside it. Whether the wording is right is the
// review of the PR.
import assert from "node:assert/strict";
import { test } from "node:test";
import { read, section } from "./lib/rfc-text.mjs";

const RFC = "docs/rfc/0009-sdk-api.md";

const part = (anchor) => section(read(RFC), anchor);

/** The rows of a Markdown table in `text` whose first cell includes `name`. */
const row = (text, name) =>
  text
    .split("\n")
    .find((l) => l.startsWith("|") && (l.split("|")[1] ?? "").includes(name));

test("Spaces and members has Erasures, the audit as a list call per space", () => {
  const erasures = part("erasures");
  assert.ok(erasures, "no section Erasures");
  for (const pattern of [
    /project\.erasures\.list\(\)/,
    /`shred`/,
    /`"final"`/,
    /`"provisional"`/,
    /`"takenBack"`/,
    /`performer`/,
    /`signOff`/,
    /`current`/,
    /`"deleted"`/,
    /`"seen"`/,
    /`"missing"`/,
    /causal past/,
    /a reader too|readers? too/,
    /Carsten, 2026-11-12, NOC-246/,
  ]) {
    assert.match(erasures, pattern, `Erasures does not say ${pattern}`);
  }
});

test("Erasures names the options not taken", () => {
  const erasures = part("erasures");
  for (const pattern of [
    /`space\.status`/,
    /across spaces|database-wide/,
    /current members only/i,
    /every member the space ever had/i,
    /object ids/,
  ]) {
    assert.match(erasures, pattern, `no option not taken: ${pattern}`);
  }
});

test("The Rust SDK names the erasure audit and its types", () => {
  const rust = part("the-rust-sdk")
    .split("\n")
    .find((l) => l.startsWith("- **Erasures.**"));
  assert.ok(rust, "The Rust SDK has no bullet on erasures");
  for (const pattern of [
    /`space\.erasures\(\)` returns `Erasures`/,
    /`list\(\)` returns `Vec<Erasure>`/,
    /`Erasure \{ shred: ChangeId, state: ErasureState, performer: MemberId, performer_account: Fingerprint, sign_off: Fingerprint, members: Vec<ErasureMember> \}`/,
    /`ErasureState::\{Final, Provisional, TakenBack\}`/,
    /`ErasureMember \{ id: MemberId, account: Fingerprint, current: bool, acknowledged: Acknowledged \}`/,
    /`Acknowledged::\{Deleted, Seen, Missing\}`/,
    /`#\[non_exhaustive\]`/,
  ]) {
    assert.match(rust, pattern, `the Rust bullet does not say ${pattern}`);
  }
});

test("Public names lists the erasure audit in both SDKs", () => {
  const names = part("public-names");
  for (const name of [
    "`space.erasures.list`",
    '`"final"`, `"provisional"`, `"takenBack"`',
    '`"deleted"`, `"seen"`, `"missing"`',
    "`Space::erasures`",
    "`Erasure`",
    "`ErasureState`",
    "`ErasureMember`",
    "`Acknowledged`",
  ]) {
    assert.ok(row(names, name), `Public names has no row for ${name}`);
  }
});
