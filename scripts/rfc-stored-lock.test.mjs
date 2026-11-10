// Asserts that RFC 0005 and RFC 0006 record the lock check on the device as
// the owner decided it in NOC-155 (Carsten, 2026-11-10) and #137 built it
// (NOC-162): the device keeps the newest lock it opened the database with,
// not simply the last one, and the view keeps that lock in `meta` under the
// key `lock`, as canonical JSON, with the view format still at 2. Where the
// RFCs name a value the code also holds (the key, the format version), the
// test reads it from the code, so the text cannot drift from it.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const RFC_0005 = "docs/rfc/0005-schema-and-model-lock.md";
const RFC_0006 = "docs/rfc/0006-local-view.md";
const STORED_LOCK = "crates/ncdb-core/src/view/stored_lock.rs";
const VIEW = "crates/ncdb-core/src/view/mod.rs";

const read = (path) => readFileSync(join(root, path), "utf8");

/** GitHub's heading slug: lower case, punctuation dropped, spaces to dashes. */
const slug = (heading) =>
  heading
    .trim()
    .toLowerCase()
    .replace(/[^\p{L}\p{N} _-]/gu, "")
    .replace(/ /g, "-");

/** The text under the heading whose anchor is `anchor`, up to the next heading of its level or higher. */
const section = (path, anchor) => {
  const lines = read(path).split("\n");
  const start = lines.findIndex(
    (l) => /^#{1,6} /.test(l) && slug(l.replace(/^#+ /, "")) === anchor,
  );
  assert.ok(start >= 0, `${path} has no heading #${anchor}`);
  const level = lines[start].match(/^#+/)[0].length;
  const rest = lines.slice(start + 1);
  const end = rest.findIndex((l) => {
    const m = l.match(/^(#+) /);
    return m && m[1].length <= level;
  });
  return (end < 0 ? rest : rest.slice(0, end)).join("\n");
};

/** The key the core stores the lock under in `meta`, as the code names it. */
const lockKey = () => {
  const m = read(STORED_LOCK).match(/const KEY: &str = "([^"]+)";/);
  assert.ok(m, `${STORED_LOCK} names no key`);
  return m[1];
};

/** The view's format version, as the code names it. */
const viewFormat = () => {
  const m = read(VIEW).match(/pub const FORMAT: u64 = (\d+);/);
  assert.ok(m, `${VIEW} names no format version`);
  return m[1];
};

const TWO_CHECKS = "two-checks-at-build-and-on-the-device";

test("RFC 0005: the device keeps the newest lock it opened the database with, not the last one", () => {
  const text = section(RFC_0005, TWO_CHECKS);
  assert.match(text, /newest lock/);
  assert.doesNotMatch(text, /last opened/);
  assert.match(
    text,
    /additive successor of the stored one[^.]*takes its place/,
  );
});

test("RFC 0005: a rolled-back app opens as an old client and leaves the stored lock, so a fork from the older lock is refused, and says why", () => {
  const text = section(RFC_0005, TWO_CHECKS);
  const rollback = text
    .split("\n")
    .find((l) => l.startsWith("- ") && l.includes("rolled back"));
  assert.ok(rollback, "no bullet on a rolled-back app");
  assert.match(rollback, /old client/);
  assert.match(rollback, /does not replace the stored lock/);
  assert.match(rollback, /forks from the older one[^.]*refused/);
  assert.match(
    rollback,
    /other rules/,
    "the reason: data folded by other rules",
  );
  assert.match(rollback, /principle 2/);
  assert.match(text, /even after[^.]*rolled back on purpose/, "the limit");
  assert.match(text, /\(0006-local-view\.md#where-the-view-lives\)/);
});

test("RFC 0005: no passage says the SDK checks against the lock it last opened", () => {
  const rfc = read(RFC_0005);
  assert.doesNotMatch(rfc, /last opened/);
});

test("RFC 0006: meta holds the lock under the code's key, as canonical JSON, and the view format stays the code's", () => {
  const text = section(RFC_0006, "where-the-view-lives");
  const key = lockKey();
  assert.match(text, /^meta {2,}[^\n]*\block\b/m, "the table of tables");
  const meta = text.split("\n").find((l) => l.startsWith("- **`meta`**"));
  assert.ok(meta, "no bullet on meta");
  assert.ok(meta.includes(`the key \`${key}\``), `the key \`${key}\``);
  assert.match(meta, /canonical JSON/);
  assert.match(meta, /0005-schema-and-model-lock\.md#canonical-form/);
  assert.match(meta, /newest lock/);
  assert.ok(
    meta.includes(`format version stays ${viewFormat()}`),
    `the format version ${viewFormat()}`,
  );
});

test("RFC 0006: a database without a stored lock opens only with an unchanged view model, and keeps that lock", () => {
  const meta = section(RFC_0006, "where-the-view-lives")
    .split("\n")
    .find((l) => l.startsWith("- **`meta`**"));
  assert.match(meta, /holds no lock/);
  assert.match(meta, /#134/);
  assert.match(meta, /model hash/);
  assert.match(meta, /keeps that lock from then on/);
  assert.match(meta, /`modelRefused`/);
});
