// Asserts that RFC 0009 and RFC 0003 record the three answers the owner gave
// on NOC-114 (Carsten, 2026-11-10), each as recommended: with neither a
// platform keystore nor a key file, opening fails with `keyStoreUnavailable`,
// whose message names the key-file option; opening with neither a path nor
// memory is refused with `invalidValue`; and the TypeScript SDK takes the same
// options as `path`, `keyFile`, `inMemory` and `agent`, in Node.js only, while
// the browser has no path and keeps its key as RFC 0006 says. Each answer
// stands in the text where it belongs, with its decision in short form and a
// dated History line, and NOC-114 leaves RFC 0009's open questions.
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

/** The decision headed `### <id>. `, up to the next heading of its level. */
const decision = (path, id) => {
  const text = section(read(path), "decisions");
  const found = text.split(new RegExp(`^### ${id}\\. `, "m"))[1];
  assert.ok(found, `${path} has no decision ${id}`);
  return found.split(/^### /m)[0];
};

const DECIDED = /^Decided \(Carsten, 2026-11-10\), on NOC-114: /m;
const NOT_TAKEN = /^Not taken:\n\n- /m;

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

test("RFC 0009 records S26 to S28 as decided on NOC-114, with History and status", () => {
  for (const id of ["S26", "S27", "S28"]) {
    const text = decision(RFC_0009, id);
    assert.match(text, DECIDED, id);
    assert.match(text, NOT_TAKEN, `${id} lists no option not taken`);
  }
  assert.match(decision(RFC_0009, "S26"), /`keyStoreUnavailable`/);
  assert.match(decision(RFC_0009, "S27"), /`invalidValue`/);
  assert.match(decision(RFC_0009, "S28"), /`keyFile`/);
  const rfc = read(RFC_0009);
  const status = rfc.split("\n").find((l) => l.startsWith("**Status.**"));
  assert.match(status, /S26 to S29/);
  assert.match(rfc, /^- \*\*2026-11-10\*\* · NOC-114 · .*\(#\d+\)\.?$/m);
});

test("NOC-114 leaves RFC 0009's open questions, and NOC-115 stays", () => {
  const open = section(read(RFC_0009), "open-questions");
  assert.doesNotMatch(open, /NOC-114/);
  assert.match(
    open,
    /^(?:One question waits|\w+ questions wait) for the owner/m,
  );
  assert.match(open, /\*\*NOC-115, question 10\.\*\*/);
});

test("RFC 0003's member key says opening fails with keyStoreUnavailable", () => {
  const key = section(read(RFC_0003), "the-member-key");
  assert.match(key, /`keyStoreUnavailable`/);
  assert.match(key, /names the key-file option/);
  assert.match(key, /#19-opening-without-a-keystore-or-a-key-file/);
});

test("RFC 0003 records decision 19 as decided on NOC-114, with History and status", () => {
  const text = decision(RFC_0003, "19");
  assert.match(text, /^Decided \(Carsten, 2026-11-10\): /m);
  assert.match(text, /Asked on NOC-114\./);
  assert.match(text, /`keyStoreUnavailable`/);
  assert.match(text, NOT_TAKEN);
  const rfc = read(RFC_0003);
  const status = rfc.split("\n").find((l) => l.startsWith("**Status.**"));
  assert.match(status, /16 to 21/);
  assert.match(
    rfc,
    /^- \*\*2026-11-10\*\* · NOC-114 · .*\(\[#\d+\]\(https:\/\/github\.com\/nocentraldb\/nocentraldb\/pull\/\d+\)\)\.?$/m,
  );
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
