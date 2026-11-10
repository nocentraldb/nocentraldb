// Asserts that RFC 0009 records the Rust names the owner decided on NOC-135
// (Carsten, 2026-11-10, option A for both parts), as #136 and #139 build
// them in crates/nocentraldb: `spaces().list()` with `SpaceInfo`, `Role`
// and `SpaceModel`, and `db.invitation()` with `Invitation`, `Fingerprint`,
// `space.members()`, `Members`, `Members::add` and `AddMember`. They stand
// in "The Rust SDK" and "Public names". `Members::add` returns a notice, as
// the owner decided on NOC-154; rfc-0009-fresh.test.mjs holds the rest of
// NOC-154 and the options not taken.
import assert from "node:assert/strict";
import { test } from "node:test";
import { assertLinksResolve, read, section } from "./lib/rfc-text.mjs";

const RFC = "docs/rfc/0009-sdk-api.md";

/** The bullet of "The Rust SDK" that starts with `start`. */
const rustBullet = (start) =>
  section(read(RFC), "the-rust-sdk")
    .split("\n")
    .find((l) => l.startsWith(start));

/** The rows of the Rust table under "Public names". */
const rustRows = () => {
  const names = section(read(RFC), "public-names");
  return names
    .slice(names.indexOf("**Rust** (`nocentraldb`)"))
    .split("\n")
    .filter((l) => l.startsWith("| `"));
};

test("The Rust SDK lists spaces().list() with SpaceInfo, Role and SpaceModel", () => {
  const bullet = rustBullet("- **Spaces and their list");
  assert.ok(bullet, "The Rust SDK has no bullet on the list of spaces");
  assert.match(bullet, /`db\.spaces\(\)\.list\(\)` returns `Vec<SpaceInfo>`/);
  assert.match(
    bullet,
    /`SpaceInfo \{ id: SpaceId, role: Option<Role>, model: SpaceModel \}`/,
  );
  assert.match(bullet, /`None`[^.]*removed[^.]*no membership[^.]*folded/);
  assert.match(bullet, /`Role::\{Reader, Writer, Admin\}`/);
  assert.match(bullet, /`SpaceModel::\{Ours, Another\}`/);
});

test("The Rust SDK lists db.invitation() and members().add with Invitation, Fingerprint, Members and AddMember", () => {
  const bullet = rustBullet("- **Invitations and members");
  assert.ok(bullet, "The Rust SDK has no bullet on invitations and members");
  assert.match(bullet, /`db\.invitation\(\)` returns an `Invitation`/);
  assert.match(
    bullet,
    /member id, its two public keys, its certificate and its account's fingerprint/,
  );
  assert.match(bullet, /`Fingerprint`/);
  assert.match(bullet, /`space\.members\(\)` returns `Members`/);
  assert.match(
    bullet,
    /`Members::add\(&Invitation, AddMember\) -> Result<Notice, Error>`/,
  );
  assert.match(
    bullet,
    /`AddMember \{ role: Role, fingerprint: Option<Fingerprint> \}`/,
  );
  assert.match(bullet, /`ErrorCode::UnknownAccount`/);
});

test("Public names lists every Rust name of NOC-135, each with the RFC it builds on", () => {
  const rows = rustRows();
  for (const name of [
    "`Spaces::list`",
    "`SpaceInfo`",
    "`SpaceModel`",
    "`Role`",
    "`Db::invitation`",
    "`Invitation`",
    "`Fingerprint`",
    "`Space::members`",
    "`Members`",
    "`Members::add`",
    "`AddMember`",
  ]) {
    const row = rows.find((r) =>
      r
        .split("|")[1]
        .split(", ")
        .map((s) => s.trim())
        .includes(name),
    );
    assert.ok(row, `Public names has no Rust row for ${name}`);
    assert.match(row, /\]\(\d{4}-[^)]+\.md#[^)]+\)/, `${name} links no RFC`);
  }
});

test("every relative link in RFC 0009 resolves, anchors included", () => {
  assertLinksResolve(RFC);
});
