// Asserts that RFC 0009 and RFC 0003 record the three answers the owner gave
// on NOC-114 (Carsten, 2026-11-10), each as recommended: with neither a
// platform keystore nor a key file, opening fails with `keyStoreUnavailable`,
// whose message names the key-file option; opening with neither a path nor
// memory is refused with `invalidValue`; and the TypeScript SDK takes the same
// options as `path`, `keyFile`, `inMemory` and `agent`, in Node.js only, while
// the browser has no path and keeps its key as RFC 0006 says. Each answer
// stands in the text where it belongs, with its reason and the options not
// taken (rfc-0009-fresh.test.mjs and rfc-0003-fresh.test.mjs).
import assert from "node:assert/strict";
import { test } from "node:test";
import { assertLinksResolve, read, section } from "./lib/rfc-text.mjs";

const RFC_0009 = "docs/rfc/0009-sdk-api.md";
const RFC_0003 = "docs/rfc/0003-keys-and-cryptography.md";

/** The rows of every Markdown table in `text` whose first cell matches `pattern`. */
const rowsStarting = (text, pattern) =>
  text
    .split("\n")
    .filter((l) => l.startsWith("|"))
    .filter((l) => pattern.test(l.split("|")[1] ?? ""));

test("RFC 0009's Open and close names the Node.js options and what a missing one does", () => {
  const open = section(read(RFC_0009), "open-and-close");
  for (const option of ["path", "keyFile", "inMemory", "agent"]) {
    assert.match(open, new RegExp(`\`${option}\``), `no ${option}`);
  }
  assert.match(open, /Node\.js/);
  assert.match(open, /browser has no path/i);
  assert.match(open, /0006-local-view\.md#what-the-view-stores-in-plaintext/);
  assert.match(open, /neither `path` nor `inMemory`.*`invalidValue`/);
  assert.match(open, /`keyStoreUnavailable`/);
});

test("RFC 0009's Rust SDK says what Db::open does without a path or a keystore", () => {
  const rust = section(read(RFC_0009), "the-rust-sdk");
  assert.match(
    rust,
    /neither `path\(dir\)` nor `in_memory\(\)`.*`invalidValue`/,
  );
  assert.match(rust, /`keyStoreUnavailable`.*names `key_file`/);
});

test("RFC 0009's error table has keyStoreUnavailable, and invalidValue for opening", () => {
  const errors = section(read(RFC_0009), "errors");
  const [keystore] = rowsStarting(errors, /`keyStoreUnavailable`/);
  assert.ok(keystore, "Errors has no row for keyStoreUnavailable");
  assert.match(keystore, /`nc\.open`/);
  assert.match(keystore, /`Db::open`/);
  assert.match(keystore, /keystore/);
  assert.match(keystore, /key file/);
  assert.match(keystore, /message names/);
  const [invalid] = rowsStarting(errors, /`invalidValue`/);
  assert.match(invalid, /`nc\.open`/);
  assert.match(invalid, /neither a path nor memory/);
});

test("RFC 0009's Public names lists the Node.js options and keyStoreUnavailable", () => {
  const names = section(read(RFC_0009), "public-names");
  const [open] = rowsStarting(names, /`nc\.open`/);
  for (const option of ["path", "keyFile", "inMemory", "agent"]) {
    assert.match(
      open,
      new RegExp(`\`${option}\``),
      `nc.open names no ${option}`,
    );
  }
  assert.match(open, /Node\.js only/);
  const [keystore] = rowsStarting(names, /`keyStoreUnavailable`/);
  assert.ok(keystore, "Public names has no row for keyStoreUnavailable");
  assert.match(keystore, /0003-keys-and-cryptography\.md#the-member-key/);
});

test("RFC 0003's member key says opening fails with keyStoreUnavailable", () => {
  const key = section(read(RFC_0003), "the-member-key");
  assert.match(key, /`keyStoreUnavailable`/);
  assert.match(key, /names the key-file option/);
});

test("the new text carries no amendment mark", () => {
  for (const path of [RFC_0009, RFC_0003]) {
    for (const line of read(path).split("\n")) {
      if (/keyStoreUnavailable|`keyFile`|`inMemory`/.test(line)) {
        assert.doesNotMatch(line, /Amendment \d/, line.slice(0, 60));
      }
    }
  }
});

test("every relative link in RFC 0009 and RFC 0003 resolves, anchors included", () => {
  assertLinksResolve(RFC_0009);
  assertLinksResolve(RFC_0003);
});
