// Holds two owner answers of 2026-11-12 as RFC 0009 records them. NOC-241 A:
// the space list is a stream, `db.spaces.subscribe(callback)` and
// `Spaces::subscribe() -> impl Stream<Item = Vec<SpaceInfo>>`, which gives the
// current list at once and each new list when a space is added, left or
// removed, or its role changes. NOC-255 A: a write of a personal field whose
// person has a person key this device does not hold yet is refused with
// `personKeyNotHeld` (`ErrorCode::PersonKeyNotHeld`), next to `noPerson`
// wherever the RFC lists errors. Whether the wording is right is the review
// of the PR.
import assert from "node:assert/strict";
import { test } from "node:test";
import { read, section } from "./lib/rfc-text.mjs";

const RFC = "docs/rfc/0009-sdk-api.md";

const part = (anchor) => section(read(RFC), anchor);

/** The row of a Markdown table in `text` whose first cell includes `name`. */
const row = (text, name) =>
  text
    .split("\n")
    .find((l) => l.startsWith("|") && (l.split("|")[1] ?? "").includes(name));

test("Spaces has db.spaces.subscribe, the space list as a stream", () => {
  const spaces = part("spaces");
  for (const pattern of [
    /db\.spaces\.subscribe\(/,
    /current list at once/,
    /added, left or removed, or (?:its|the member's) role changes/,
    /Carsten, 2026-11-12, NOC-241/,
  ])
    assert.match(spaces, pattern);
});

test("the Rust SDK names Spaces::subscribe as a stream of Vec<SpaceInfo>", () => {
  const rust = part("the-rust-sdk");
  const bullet = rust
    .split("\n")
    .find((l) => l.startsWith("- **Spaces and their list.**"));
  assert.ok(bullet, "no bullet Spaces and their list");
  assert.match(
    bullet,
    /`Spaces::subscribe\(\) -> impl Stream<Item = Vec<SpaceInfo>>`/,
  );
  assert.match(bullet, /Carsten, 2026-11-12, NOC-241/);
});

test("Public names list the stream in both languages", () => {
  const names = part("public-names");
  assert.ok(row(names, "`db.spaces.subscribe`"), "no TypeScript row");
  assert.ok(row(names, "`Spaces::subscribe`"), "no Rust row");
});

test("personKeyNotHeld stands next to noPerson wherever errors are listed", () => {
  const errors = part("errors");
  const errorRow = row(errors, "`personKeyNotHeld`");
  assert.ok(errorRow, "no row in Errors");
  for (const pattern of [
    /person key/,
    /does not hold yet/,
    /retr(?:y|ies)/,
    /sync/,
    /Carsten, 2026-11-12, NOC-255/,
  ])
    assert.match(errorRow, pattern);

  const names = part("public-names");
  const writes = row(names, "`noPerson`");
  assert.ok(writes, "no Public names row with noPerson");
  assert.match(writes.split("|")[1], /`personKeyNotHeld`/);

  const personal = part("personal-fields");
  assert.match(personal, /`personKeyNotHeld`/);

  const rust = part("the-rust-sdk");
  const bullet = rust
    .split("\n")
    .find((l) => l.startsWith("- **Personal fields.**"));
  assert.match(bullet, /`ErrorCode::PersonKeyNotHeld`/);
});
