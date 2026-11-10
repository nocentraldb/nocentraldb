// Asserts that RFC 0009 records the Rust names the owner decided on NOC-135
// (Carsten, 2026-11-10, option A for both parts), as #136 and #139 build
// them in crates/nocentraldb: `spaces().list()` with `SpaceInfo`, `Role`
// and `SpaceModel`, and `db.invitation()` with `Invitation`, `Fingerprint`,
// `space.members()`, `Members`, `Members::add` and `AddMember`. They stand
// in "The Rust SDK" and "Public names", with decisions after S32 and a dated
// History line naming NOC-135 and NOC-150. The five points NOC-154 asks the
// owner stay open: the RFC names them as open and does not decide them
// (NOC-150).
import assert from "node:assert/strict";
import { test } from "node:test";
import { assertLinksResolve, read, section, slug } from "./lib/rfc-text.mjs";

const RFC = "docs/rfc/0009-sdk-api.md";

/** The decisions of RFC 0009 whose text names NOC-135. */
const noc135Decisions = () => {
  const decisions = section(read(RFC), "decisions");
  return decisions
    .split("\n")
    .filter((l) => l.startsWith("### "))
    .map((l) => l.slice(4))
    .map((heading) => ({ heading, text: section(decisions, slug(heading)) }))
    .filter(({ text }) =>
      /^Decided \(Carsten, 2026-11-10\),? on NOC-135/m.test(text),
    );
};

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
  const bullet = rustBullet("- **Spaces and their list**");
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
  const bullet = rustBullet("- **Invitations and members**");
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
    /`Members::add\(&Invitation, AddMember\) -> Result<\(\), Error>`/,
  );
  assert.match(
    bullet,
    /`AddMember \{ role: Role, fingerprint: Option<Fingerprint> \}`/,
  );
  assert.match(bullet, /`ErrorCode::UnknownAccount`/);
});

test("The Rust SDK names NOC-154's five points as open, not decided", () => {
  const bullet = rustBullet("- **Open for `members().add`**");
  assert.ok(bullet, "The Rust SDK does not name what is open for add");
  assert.match(bullet, /NOC-154/);
  for (const point of [
    /notice/,
    /error code/,
    /accounts/,
    /text form/,
    /agent/,
  ]) {
    assert.match(bullet, point);
  }
  const open = section(read(RFC), "open-questions");
  assert.match(open, /NOC-154/);
  for (const { heading, text } of noc135Decisions()) {
    assert.doesNotMatch(
      text,
      /`Notice`|NoticeCode|invalidCertificate|alreadyMember/,
      heading,
    );
  }
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

test("RFC 0009 records NOC-135's two parts as decisions after S32", () => {
  const found = noc135Decisions();
  assert.deepEqual(
    found.map(({ heading }) => heading.split(".")[0]),
    ["S33", "S34"],
  );
  const [list, add] = found.map(({ text }) => text);
  assert.match(list, /`SpaceInfo`/);
  assert.match(list, /#139/);
  assert.match(add, /`AddMember`/);
  assert.match(add, /#136/);
  for (const text of [list, add]) {
    assert.match(text, /^Why: /m);
    assert.match(text, /^Not taken:\n\n- /m);
  }
});

test("the Status line and a dated History line name NOC-135 and NOC-150", () => {
  const status = read(RFC)
    .split("\n")
    .find((l) => l.startsWith("**Status.**"));
  assert.match(status, /S33 and S34/);
  assert.match(status, /NOC-135/);
  const line = section(read(RFC), "history")
    .split("\n")
    .find((l) => l.startsWith("- **2026-11-10** · NOC-150 · "));
  assert.ok(line, "History has no dated line for NOC-150");
  assert.match(line, /NOC-135/);
  assert.match(line, /\(#\d+\)\.?$/);
});

test("every relative link in RFC 0009 resolves, anchors included", () => {
  assertLinksResolve(RFC);
});
