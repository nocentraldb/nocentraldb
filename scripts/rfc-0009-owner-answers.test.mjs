// Asserts that RFC 0009 records the six answers the owner gave in NOC-95
// (Carsten, 2026-11-10), as NOC-106 writes them into its text: where a
// database and its member key live, the account key and `agent()`, the Rust
// names, the error codes, the space id in TypeScript, and how an app writes
// a reference. Each stands where it belongs, without an amendment mark, with
// its decision S19 to S24 and a dated line naming the pull request.
import assert from "node:assert/strict";
import { test } from "node:test";
import { assertLinksResolve, read, section } from "./lib/rfc-text.mjs";

const RFC = "docs/rfc/0009-sdk-api.md";

/** The rows of every Markdown table in `text` whose first cell matches `pattern`. */
const rowsStarting = (text, pattern) =>
  text
    .split("\n")
    .filter((l) => l.startsWith("|"))
    .filter((l) => pattern.test(l.split("|")[1] ?? ""));

// Since NOC-101, "Decisions" keeps each decision in short form under its own heading.
const decision = (id) => {
  const text = section(read(RFC), "decisions");
  const found = text.split(new RegExp(`^### ${id}\\. `, "m"))[1];
  assert.ok(found, `${RFC} has no decision ${id}`);
  return found.split(/^### S\d+\. /m)[0];
};

test("the Rust SDK opens a database at a path, in memory, with a key file, or as an agent", () => {
  const rust = section(read(RFC), "the-rust-sdk");
  assert.match(rust, /\.path\(/);
  assert.match(rust, /OpenOptions::path\(dir\)|`path\(dir\)`/);
  assert.match(rust, /`in_memory\(\)`/);
  assert.match(rust, /`key_file\(/);
  assert.match(rust, /`agent\(\)`/);
  assert.match(rust, /platform keystore/);
  assert.match(rust, /0003-keys-and-cryptography\.md#the-member-key/);
  assert.match(rust, /0003-keys-and-cryptography\.md#the-account-key/);
});

test("the account key: each database draws its own until the accounts RFC", () => {
  const accounts = section(read(RFC), "accounts-and-devices");
  assert.match(accounts, /draws its own account key once/);
  assert.match(accounts, /unrelated accounts/);
  assert.match(accounts, /`agent\(\)`/);
});

test("Public names lists the Rust names the owner accepted", () => {
  const names = section(read(RFC), "public-names");
  for (const name of [
    "Space",
    "Spaces",
    "Transaction",
    "Query",
    "Page",
    "Cursor",
    "ObjectId",
    "SpaceId",
    "ChangeId",
    "Object",
    "Written",
    "page.has_more",
    "OpenOptions::path",
    "OpenOptions::in_memory",
    "OpenOptions::key_file",
    "OpenOptions::agent",
  ]) {
    assert.ok(
      rowsStarting(names, new RegExp(`\`${name.replace(/[.:]/g, "\\$&")}[\`(]`))
        .length > 0,
      `Public names has no row for ${name}`,
    );
  }
  const object = rowsStarting(names, /`Object`/)[0];
  for (const accessor of ["id()", "space()", "deleted()", "get::<T>(name)"]) {
    assert.ok(object.includes(accessor), `Object's row names no ${accessor}`);
  }
  const written = rowsStarting(names, /`Written`/)[0];
  assert.match(written, /`change`/);
  assert.match(written, /`ids`/);
});

test("the error codes follow NOC-95's point 4, with storageFailed", () => {
  const errors = section(read(RFC), "errors");
  const row = (code) => {
    const [found] = rowsStarting(errors, new RegExp(`\`${code}\``));
    assert.ok(found, `Errors has no row for ${code}`);
    return found;
  };
  assert.match(row("storageFull"), /full/);
  assert.match(row("storageFailed"), /any other storage failure/i);
  assert.match(row("notFound"), /collection, field or index/);
  assert.match(row("invalidValue"), /equality values/);
  assert.match(row("invalidValue"), /page size/);
  assert.match(row("invalidValue"), /scope/);
  assert.match(row("modelRefused"), /cannot be read/);
  assert.match(
    section(read(RFC), "public-names"),
    /`storageFull`, `storageFailed`/,
  );
});

test("a space id in TypeScript is 32 lowercase hex digits, as an object id", () => {
  const get = section(read(RFC), "get-by-id");
  assert.match(get, /space id.*32 lowercase hex digits/i);
});

test("an app writes a reference as the { space, id } it reads, or null", () => {
  const ops = section(read(RFC), "field-operations-per-field-type");
  const [ref] = rowsStarting(ops, /`nc\.ref`/);
  assert.ok(ref, "Field operations per field type has no nc.ref row");
  assert.match(ref, /`\{ space, id \}`/);
  assert.match(ref, /`null`/);
  assert.match(ref, /set \(1\)/);
});

test("decisions S19 to S24 record the owner's choice, on NOC-95", () => {
  for (const id of ["S19", "S20", "S21", "S22", "S23", "S24"]) {
    const text = decision(id);
    assert.match(text, /^Decided \(Carsten, 2026-11-10\), on NOC-95: /m, id);
    assert.match(text, /^Not taken:\n\n- /m, `${id} lists no option not taken`);
  }
  assert.match(decision("S22"), /`storageFailed`/);
});

test("the new text carries no amendment mark, and the status names S19 to S24", () => {
  const rfc = read(RFC);
  for (const line of rfc.split("\n")) {
    if (/storageFailed|`in_memory\(\)`|`nc\.ref` \|/.test(line)) {
      assert.doesNotMatch(line, /\(Amendment \d/, line.slice(0, 60));
    }
  }
  assert.doesNotMatch(rfc, /^## Amendment \d/m);
  const status = rfc.split("\n").find((l) => l.startsWith("**Status.**"));
  assert.match(status, /S19 to S24.*Carsten, 2026-11-10/);
  assert.match(rfc, /^- \*\*2026-11-10\*\* · NOC-106 · .*\(#\d+\)\.?$/m);
});

test("every relative link in RFC 0009 resolves, anchors included", () => {
  assertLinksResolve(RFC);
});
