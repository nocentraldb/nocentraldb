// Asserts that RFC 0009 records the Rust erase calls the owner decided on
// NOC-326 (Carsten, 2026-11-15, all recommendations), as NOC-325 builds
// them in crates/nocentraldb: `Db::erasure_request`, `Account::sign_off`
// and `Db::erase`, their texts, results and notice, and
// `Erasures::take_back` with the notice `TakenBack`. They stand in "The
// Rust SDK" and "Public names"; the intro says the work-order RFC may wrap
// them.
import assert from "node:assert/strict";
import { test } from "node:test";
import { read, section } from "./lib/rfc-text.mjs";

const RFC = "docs/rfc/0009-sdk-api.md";

const rustBullet = (start) =>
  section(read(RFC), "the-rust-sdk")
    .split("\n")
    .find((l) => l.startsWith(start));

const rustRows = () => {
  const names = section(read(RFC), "public-names");
  return names
    .slice(names.indexOf("**Rust** (`nocentraldb`)"))
    .split("\n")
    .filter((l) => l.startsWith("| `"));
};

test("the intro says the Rust SDK erases with Db::erase until the work-order RFC", () => {
  const intro = read(RFC)
    .split("\n")
    .find((l) => l.startsWith("- **Work orders and erasure.**"));
  assert.match(intro, /`Db::erase`/);
  assert.match(intro, /NOC-326/);
});

test("The Rust SDK has the erase calls, their texts and their results", () => {
  const bullet = rustBullet("- **Erasing a person.**");
  assert.ok(bullet, "The Rust SDK has no bullet on erasing a person");
  for (const pattern of [
    /`db\.erasure_request\(&object\)` returns an `ErasureRequest`/,
    /`ncdb-erase-1:`/,
    /`db\.account\(\)\.sign_off\(&request\)` returns `Signed \{ sign_offs: SignOffs, not_signed: Vec<SpaceId> \}`/,
    /`ncdb-sign-off-1:`/,
    /`db\.erase\(&request, &sign_offs\)` returns `Erased \{ shreds: Vec<ErasedIn>, skipped: Vec<\(SpaceId, Skipped\)>, notice: Notice \}`/,
    /`ErasedIn \{ space: SpaceId, shred: ChangeId, provisional: bool \}`/,
    /`Skipped::\{NoSignOff, NotWriter, NoKey\}`/,
    /`ErrorCode::InvalidValue`[^.]*nothing is written/,
    /`NoticeCode::Erased`/,
    /`space\.erasures\(\)\.take_back\(shred\)`/,
    /`NoticeCode::TakenBack`/,
    /Carsten, 2026-11-15, NOC-326/,
  ]) {
    assert.match(bullet, pattern, `the bullet does not say ${pattern}`);
  }
});

test("The Rust SDK gives the texts of Erased and TakenBack", () => {
  const bullet = rustBullet("- **Erasing a person.**");
  assert.match(
    bullet,
    /"The person's personal fields are erased in these spaces\. Devices that never come back online, copies made outside the database and unmarked free text keep them; other devices and agents of this account erase only when they run the erasure too\."/,
  );
  assert.match(
    bullet,
    /"The erasure is taken back in this space, and the person's fields read again; a finalize another device of the account wrote before it saw this take-back still wins, and the person stays erased\."/,
  );
});

test("Public names lists the erase calls and their types", () => {
  const rows = rustRows().join("\n");
  for (const name of [
    "`Db::erasure_request`",
    "`Db::erase`",
    "`Account::sign_off`",
    "`ErasureRequest`",
    "`SignOffs`",
    "`Signed`",
    "`Erased`",
    "`ErasedIn`",
    "`Skipped`",
    "`Erasures::take_back`",
  ]) {
    assert.ok(rows.includes(name), `Public names lacks ${name}`);
  }
});

test("the notice codes include takenBack", () => {
  const names = section(read(RFC), "public-names");
  const row = names.split("\n").find((l) => l.includes("`memberAdded`"));
  assert.match(row, /`takenBack`/);
});
