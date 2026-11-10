// Asserts that RFC 0003, RFC 0008 and RFC 0009 record the owner's answers of
// 2026-11-10 that came after NOC-114, each as recommended on its ticket:
// NOC-116 A, the Rust names of the sync report in RFC 0009; NOC-117 A, the
// bytes a writer signs for a person-key record in RFC 0003; NOC-110 point 4 A,
// the bytes of the associated data of an encrypted previous key in RFC 0003;
// and NOC-110 points 1 to 3, the current generation and who may hold a
// generation, in RFC 0008. Each stands where it belongs. RFC 0003, RFC 0008
// and RFC 0009 are fresh texts (NOC-165), whose reasons and options not taken
// rfc-0003-fresh, rfc-0008-fresh and rfc-0009-fresh hold.
import assert from "node:assert/strict";
import { test } from "node:test";
import { read, section } from "./lib/rfc-text.mjs";

const RFC_0003 = "docs/rfc/0003-keys-and-cryptography.md";
const RFC_0008 = "docs/rfc/0008-membership.md";
const RFC_0009 = "docs/rfc/0009-sdk-api.md";

/** The rows of every Markdown table in `text` whose first cell matches `pattern`. */
const rowsStarting = (text, pattern) =>
  text
    .split("\n")
    .filter((l) => l.startsWith("|"))
    .filter((l) => pattern.test(l.split("|")[1] ?? ""));

// NOC-116 A

test("RFC 0009's Rust SDK names SyncReport, SyncStatus and Connection", () => {
  const rust = section(read(RFC_0009), "the-rust-sdk");
  assert.match(rust, /`db\.sync\(\)` returns a `SyncReport`/);
  assert.match(
    rust,
    /`SyncStatus` with `connection`, `unconfirmed`, `refused`, `epoch_changed` and `error`/,
  );
  assert.match(
    rust,
    /`Connection::Connected`, `Connecting`, `Offline`, `Paused` or `None`/,
  );
  assert.match(rust, /shadow Rust's `Sync` marker trait/);
});

test("RFC 0009's Sync and status points to the Rust names", () => {
  const sync = section(read(RFC_0009), "sync-and-status");
  assert.match(sync, /`SyncReport`/);
  assert.ok(sync.includes("(#the-rust-sdk)"), "no link to The Rust SDK");
});

test("RFC 0009's Public names lists Db::sync, SyncReport, SyncStatus and Connection", () => {
  const names = section(read(RFC_0009), "public-names");
  for (const name of ["Db::sync", "SyncReport", "SyncStatus", "Connection"]) {
    const pattern = new RegExp(`\`${name.replace(/:/g, "\\:")}[\`(]`);
    assert.ok(rowsStarting(names, pattern).length > 0, `no row for ${name}`);
  }
});

// NOC-117 A

const RECORD_BYTES =
  /SHA-256\("nocentraldb\/person-key-record" \|\| space id \(16 bytes\) \|\| person-key id \(16 bytes\) \|\| generation \(8 bytes, big-endian\) \|\| writer member id \(32 bytes\) \|\| wrapped bytes\)/;

test("RFC 0003 gives the bytes a writer signs for a person-key record", () => {
  const record = section(read(RFC_0003), "person-key-record");
  assert.match(record, RECORD_BYTES);
  assert.match(record, /Ed25519/);
  const person = section(read(RFC_0003), "the-person-key");
  assert.match(person, /\(#person-key-record\)/);
  assert.doesNotMatch(person, /open question/i);
});

// NOC-110 point 4 A

test("RFC 0003's key-chain subkey gives the bytes of the associated data", () => {
  const subkey = section(read(RFC_0003), "key-chain-subkey");
  assert.match(
    subkey,
    /associated data = space id \(16 bytes\) \|\| new generation id \(8 bytes, big-endian\) \|\|\s+previous generation id \(8 bytes, big-endian\)/,
  );
  assert.match(subkey, /no label/);
  assert.doesNotMatch(subkey, /open-questions/);
});

// NOC-110 points 1 to 3

test("RFC 0008's current generation says which heads two entries or a later rejection leave", () => {
  const steps = section(
    read(RFC_0008),
    "two-removals-that-rotate-the-key-at-once",
  );
  assert.match(steps, /two key chain entries name is never a head/);
  assert.match(steps, /rejected later.*is still a head and can be current/);
});

test("RFC 0008 says who may hold a generation, whatever the later verdict", () => {
  const steps = section(
    read(RFC_0008),
    "two-removals-that-rotate-the-key-at-once",
  );
  assert.match(steps, /The creator holds the first generation/);
  assert.match(steps, /valid when written, whatever its final verdict/);
  assert.match(steps, /never valid when written/);
  assert.match(steps, /author of an envelope is not counted for it/);
});
