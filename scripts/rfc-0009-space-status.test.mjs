// Holds the owner's answer on NOC-193 (Carsten, 2026-11-10, every
// recommendation) as RFC 0009 records it (NOC-195): the Rust names of the
// per-space status. `space.status()` returns a `SpaceStatusReport` with
// `current()` and `subscribe()`; `current()` gives a `SpaceStatus` with the
// twelve values of "Sync and status" in snake case and their types; the
// reasons and the refused, waiting shreds are their own types; and the
// status names no member by its `MemberId`, which the owner made public for
// the member calls on NOC-203 (Carsten, 2026-11-11) while the status stays as
// NOC-193 decided. Like every choice in the fresh texts, it stands
// in the section it belongs to, with the options not taken. Whether the
// wording is right is the review of the PR.
import assert from "node:assert/strict";
import { test } from "node:test";
import { read, section } from "./lib/rfc-text.mjs";

const RFC = "docs/rfc/0009-sdk-api.md";

const part = (anchor) => section(read(RFC), anchor);

/** The bullet of "The Rust SDK" that starts with `start`. */
const bullet = (start) => {
  const found = part("the-rust-sdk")
    .split("\n")
    .find((l) => l.startsWith(start));
  assert.ok(found, `The Rust SDK has no bullet "${start}"`);
  return found;
};

/** The rows of a Markdown table in `text` whose first cell includes `name`. */
const row = (text, name) =>
  text
    .split("\n")
    .find((l) => l.startsWith("|") && (l.split("|")[1] ?? "").includes(name));

test("Sync and status points to the Rust names of the space status", () => {
  const sync = part("sync-and-status");
  assert.match(sync, /`SpaceStatusReport`/);
  assert.match(sync, /`SpaceStatus`/);
});

test("the Rust SDK names the space status report and its twelve values with their types", () => {
  const text = bullet("- **The space status.**");
  for (const pattern of [
    /`space\.status\(\)` returns a `SpaceStatusReport`/,
    /`current\(\)`/,
    /`subscribe\(\)`/,
    /`SpaceStatus`/,
    /`#\[non_exhaustive\]`/,
    /`role: Option<Role>`/,
    /`initial_sync_complete: bool`/,
    /`behind: u64`/,
    /`gaps: Vec<ChangeId>`/,
    /`parked: BTreeMap<ParkReason, u64>`/,
    /`rejected: BTreeMap<RejectReason, u64>`/,
    /`newest_model: Option<u32>`/,
    /`refused: Vec<Refused>`/,
    /`Refused \{ change: ChangeId, code: String \}`/,
    /`unknown_error`/,
    /`lost_grants: Vec<ChangeId>`/,
    /`equivocations: Vec<ChangeId>`/,
    /`exposed: bool`/,
    /`waiting_for_key: bool`/,
    /`shreds_waiting: Vec<ShredWaiting>`/,
    /`ShredWaiting \{ change: ChangeId, age: Duration, blocking: Vec<Fingerprint> \}`/,
    /another model/,
  ]) {
    assert.match(
      text,
      pattern,
      `the space status bullet does not say ${pattern}`,
    );
  }
});

test("the space status names no member by its MemberId, and the forms not taken are named", () => {
  const text = bullet("- **The space status.**");
  assert.match(text, /names no member by its `MemberId`/);
  assert.doesNotMatch(text, /no public `MemberId`/);
  for (const pattern of [
    /`StatusReport`/,
    /`SpaceRole`/,
    /text codes/,
    /`LostGrant`/,
  ]) {
    assert.match(text, pattern, `no option not taken: ${pattern}`);
  }
});

test("Public names lists the space status types", () => {
  const names = part("public-names");
  for (const name of [
    "SpaceStatusReport",
    "SpaceStatus",
    "ParkReason",
    "RejectReason",
    "Refused",
    "ShredWaiting",
  ]) {
    assert.ok(row(names, `\`${name}`), `Public names has no row for ${name}`);
  }
});
