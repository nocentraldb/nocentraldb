// Holds the owner's answer on NOC-203 (Carsten, 2026-11-11, every
// recommendation) as RFC 0009 records it (NOC-169): a public, opaque
// `MemberId`; `Members::list` returns `MemberInfo { id, account, linked_to,
// kind, role, is_self }` (NOC-307 A1 added `linked_to`) for the current members, with `MemberKind`; `Members::remove`
// and `Members::set_role` take a `MemberId` and return a `Notice`,
// `set_role` with `NoticeCode::RoleSet` (TypeScript `roleSet`). Each stands
// where it belongs, with the options not taken. Whether the wording is right
// is the review of the PR.
import assert from "node:assert/strict";
import { test } from "node:test";
import { read, section } from "./lib/rfc-text.mjs";

const RFC = "docs/rfc/0009-sdk-api.md";

/** The text of RFC 0009's section `anchor`, which must exist. */
const part = (anchor) => section(read(RFC), anchor);

/** The bullet of `text` that starts with `start`, which must exist. */
const bullet = (text, start) => {
  const found = text.split("\n").find((l) => l.startsWith(start));
  assert.ok(found, `no bullet starts with "${start}"`);
  return found;
};

/** Whether a table row of `text` starts with the cell `cell`. */
const row = (text, cell) =>
  text.split("\n").some((l) => l.startsWith(`| ${cell}`));

test("the TypeScript part shows setRole's notice and lists current members only", () => {
  const members = part("invitations-and-members");
  assert.match(members, /lowered\.notice; \/\/ \{ code: "roleSet"/);
  assert.match(
    members,
    /A member whose role was lowered keeps its copy of everything it received before now\./,
  );
  const list = bullet(members, "- **`members.list()`**");
  assert.match(list, /each current member's id/);
  assert.match(list, /A removed member is not listed/);
  assert.match(list, /NOC-203/);
  const calls = bullet(members, "- **`members.setRole(memberId, role)`**");
  assert.match(calls, /`roleSet` for `setRole`/);
  assert.match(calls, /Returning nothing from `setRole` was not taken/);
  assert.match(bullet(members, "- **Notices.**"), /`roleSet`/);
});

test("the Rust SDK names MemberId, MemberInfo, MemberKind, list, remove and set_role", () => {
  const rust = bullet(part("the-rust-sdk"), "- **Invitations and members.**");
  for (const pattern of [
    /`Members::list\(\) -> Result<Vec<MemberInfo>, Error>`/,
    /`MemberInfo \{ id: MemberId, account: Fingerprint, linked_to: Option<Fingerprint>, kind: MemberKind, role: Option<Role>, is_self: bool \}`/,
    /`MemberInfo` is `#\[non_exhaustive\]`, so that a later field is no breaking change \(Carsten, 2026-11-13, NOC-307 A1\)/,
    /`MemberKind` is `Device`, `Agent` or `SubAgent`, `#\[non_exhaustive\]`/,
    /`MemberId` is public and opaque: `Copy`, `Eq`, `Ord`, `Hash` and `Debug`, with no text form yet/,
    /`Members::remove\(MemberId\) -> Result<Notice, Error>` returns `NoticeCode::MemberRemoved`/,
    /`Members::set_role\(MemberId, Role\) -> Result<Notice, Error>` returns `NoticeCode::RoleSet`/,
    /`&Invitation` as the argument of `remove` and `set_role` was not taken/,
    /Carsten, 2026-11-11, NOC-203/,
  ]) {
    assert.match(
      rust,
      pattern,
      `the Rust members bullet does not say ${pattern}`,
    );
  }
});

test("Public names lists the member calls and roleSet", () => {
  const names = part("public-names");
  assert.ok(row(names, "`Members::remove`, `Members::set_role`"));
  assert.ok(
    row(names, "`Members::list`, `MemberInfo`, `MemberId`, `MemberKind`"),
  );
  assert.ok(
    names
      .split("\n")
      .some((l) => l.startsWith("| `memberAdded`") && l.includes("`roleSet`")),
    "the notice codes row has no roleSet",
  );
});
