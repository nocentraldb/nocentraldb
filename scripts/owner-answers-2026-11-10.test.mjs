// Asserts that RFC 0003, RFC 0008 and RFC 0009 record the owner's answers of
// 2026-11-10 that came after NOC-114, each as recommended on its ticket:
// NOC-116 A, the Rust names of the sync report in RFC 0009; NOC-117 A, the
// bytes a writer signs for a person-key record in RFC 0003; NOC-110 point 4 A,
// the bytes of the associated data of an encrypted previous key in RFC 0003;
// and NOC-110 points 1 to 3, the current generation and who may hold a
// generation, in RFC 0008. Each stands where it belongs, with a decision in
// short form and a dated History line, and leaves the open questions.
import assert from "node:assert/strict";
import { test } from "node:test";
import { read, section, slug } from "./lib/rfc-text.mjs";

const RFC_0003 = "docs/rfc/0003-keys-and-cryptography.md";
const RFC_0008 = "docs/rfc/0008-membership.md";
const RFC_0009 = "docs/rfc/0009-sdk-api.md";

/** The decision of `path` whose heading starts with `### <prefix>`. */
const decision = (path, prefix) => {
  const decisions = section(read(path), "decisions");
  const heading = decisions
    .split("\n")
    .find((l) => l.startsWith(`### ${prefix}`));
  assert.ok(heading, `${path} has no decision "${prefix}"`);
  return section(decisions, slug(heading.slice(4)));
};

/** The rows of every Markdown table in `text` whose first cell matches `pattern`. */
const rowsStarting = (text, pattern) =>
  text
    .split("\n")
    .filter((l) => l.startsWith("|"))
    .filter((l) => pattern.test(l.split("|")[1] ?? ""));

const historyLine = (path, ticket) =>
  section(read(path), "history")
    .split("\n")
    .find((l) => l.startsWith("- **2026-11-10** · ") && l.includes(ticket));

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
  assert.match(rust, /#s29-the-rust-names-of-the-sync-report/);
});

test("RFC 0009's Sync and status points to the Rust names", () => {
  const sync = section(read(RFC_0009), "sync-and-status");
  assert.match(sync, /`SyncReport`/);
  assert.match(sync, /#s29-the-rust-names-of-the-sync-report/);
});

test("RFC 0009's Public names lists Db::sync, SyncReport, SyncStatus and Connection", () => {
  const names = section(read(RFC_0009), "public-names");
  for (const name of ["Db::sync", "SyncReport", "SyncStatus", "Connection"]) {
    const pattern = new RegExp(`\`${name.replace(/:/g, "\\:")}[\`(]`);
    assert.ok(rowsStarting(names, pattern).length > 0, `no row for ${name}`);
  }
});

test("RFC 0009 records S29 on NOC-116, removes it from Open questions, and dates it", () => {
  const text = decision(RFC_0009, "S29. ");
  assert.match(text, /^Decided \(Carsten, 2026-11-10\), on NOC-116: /m);
  assert.match(text, /`SyncReport`/);
  assert.match(text, /^Not taken:\n\n- /m);
  const open = section(read(RFC_0009), "open-questions");
  assert.doesNotMatch(open, /NOC-116/);
  assert.match(open, /\*\*NOC-115, question 10\.\*\*/);
  const status = read(RFC_0009)
    .split("\n")
    .find((l) => l.startsWith("**Status.**"));
  assert.match(status, /S26 to S29/);
  assert.match(historyLine(RFC_0009, "NOC-116") ?? "", /\(#\d+\)\.?$/);
});

// NOC-117 A

const RECORD_BYTES =
  /SHA-256\("nocentraldb\/person-key-record" \|\| space id \(16 bytes\) \|\| person-key id \(16 bytes\) \|\| generation \(8 bytes, big-endian\) \|\| writer member id \(32 bytes\) \|\| wrapped bytes\)/;

test("RFC 0003 gives the bytes a writer signs for a person-key record", () => {
  const record = section(read(RFC_0003), "person-key-record");
  assert.match(record, RECORD_BYTES);
  assert.match(record, /Ed25519/);
  assert.match(record, /#21-the-bytes-signed-for-a-person-key-record/);
  const person = section(read(RFC_0003), "the-person-key");
  assert.match(person, /\(#person-key-record\)/);
  assert.doesNotMatch(person, /open question/i);
});

test("RFC 0003 records decision 21 on NOC-117", () => {
  const text = decision(RFC_0003, "21. ");
  assert.match(text, /^Decided \(Carsten, 2026-11-10\): /m);
  assert.match(text, /NOC-117/);
  assert.match(text, /^Not taken:\n\n- /m);
  assert.ok(historyLine(RFC_0003, "NOC-117"), "no History line for NOC-117");
});

// NOC-110 point 4 A

test("RFC 0003's key-chain subkey gives the bytes of the associated data", () => {
  const subkey = section(read(RFC_0003), "key-chain-subkey");
  assert.match(
    subkey,
    /associated data = space id \(16 bytes\) \|\| new generation id \(8 bytes, big-endian\) \|\|\s+previous generation id \(8 bytes, big-endian\)/,
  );
  assert.match(subkey, /no label/);
  assert.match(subkey, /#20-the-associated-data-of-an-encrypted-previous-key/);
  assert.doesNotMatch(subkey, /open-questions/);
});

test("RFC 0003 records decision 20 on NOC-110, and has no open question left", () => {
  const text = decision(RFC_0003, "20. ");
  assert.match(text, /^Decided \(Carsten, 2026-11-10\): /m);
  assert.match(text, /NOC-110/);
  assert.match(text, /^Not taken:\n\n- /m);
  assert.ok(historyLine(RFC_0003, "NOC-110"), "no History line for NOC-110");
  const open = section(read(RFC_0003), "open-questions");
  assert.match(open, /^None\. The owner has decided every question/m);
  const status = read(RFC_0003)
    .split("\n")
    .find((l) => l.startsWith("**Status.**"));
  assert.match(status, /16 to 21/);
});

// NOC-110 points 1 to 3

test("RFC 0008's current generation says which heads two entries or a later rejection leave", () => {
  const steps = section(
    read(RFC_0008),
    "two-removals-that-rotate-the-key-at-once",
  );
  assert.match(steps, /two key chain entries name is never a head/);
  assert.match(steps, /#m27-a-generation-that-two-key-chain-entries-name/);
  assert.match(steps, /rejected later.*is still a head and can be current/);
  assert.match(steps, /#m28-a-rejected-rotation-can-be-current/);
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
  assert.match(steps, /#m29-who-may-hold-a-generation/);
});

test("RFC 0008 records M27 to M29 on NOC-110, with status and History", () => {
  for (const id of ["M27", "M28", "M29"]) {
    const text = decision(RFC_0008, `${id}. `);
    assert.match(text, /^Decided \(Carsten, 2026-11-10\), on NOC-110: /m, id);
    assert.match(text, /^Not taken:\n\n- /m, id);
  }
  const status = read(RFC_0008)
    .split("\n")
    .find((l) => l.startsWith("**Status.**"));
  assert.match(status, /M27 to M29/);
  assert.match(historyLine(RFC_0008, "NOC-110") ?? "", /\(#\d+\)\.?$/);
});
