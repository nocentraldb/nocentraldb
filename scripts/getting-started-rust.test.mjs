// Asserts that the Rust getting-started guide, docs/getting-started.md, says
// where a database lives (NOC-330; the owner, 2026-11-15), as RFC 0009's
// Rust section ("Where the database lives") does: the app always chooses
// the place, the member key sits in the platform keystore, a key file is
// for headless agents, memory is for tests, and an app passes its
// platform's app-data directory itself. The doc tests of `nocentraldb`
// compile the guide's Rust snippets.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const guide = readFileSync("docs/getting-started.md", "utf8");

/** The section under the heading "Where the database lives", to the next heading of its level or above. */
const section = () => {
  const start = guide.search(/^#{2,3} Where the database lives$/m);
  assert.ok(start >= 0, "the guide has no section Where the database lives");
  const level = guide.slice(start).match(/^#+/)[0].length;
  const rest = guide.slice(guide.indexOf("\n", start) + 1);
  const end = rest.search(new RegExp(`^#{1,${level}} `, "m"));
  return end < 0 ? rest : rest.slice(0, end);
};

test("the app always chooses the place: a directory the SDK owns, or memory", () => {
  const text = section();
  assert.match(text, /`\.path\(/);
  assert.match(text, /`nocentraldb\.sqlite`/);
  assert.match(text, /no default/);
  assert.match(text, /`ErrorCode::InvalidValue`/);
});

test("the member key is in the platform keystore, one entry per database, and a moved directory loses it", () => {
  const text = section();
  assert.match(text, /keystore/);
  assert.match(text, /one entry per database/);
  assert.match(text, /mov(e|ed|ing)[^.]*(loses|lose)/i);
});

test("a key file is for headless agents, and only its permissions protect it", () => {
  const text = section();
  assert.match(text, /`\.key_file\(/);
  assert.match(text, /headless/);
  assert.match(text, /permissions/);
});

test("memory is for tests", () => {
  assert.match(section(), /`\.in_memory\(\)`[^.]*tests/);
});

test("an app passes the platform's app-data directory itself; the SDK does not pick one", () => {
  const text = section();
  assert.match(text, /app.data directory/);
  assert.match(text, /does not pick/);
});

test("the section shows the choices in a snippet the doc tests compile", () => {
  const text = section();
  assert.match(
    text,
    /```rust no_run[\s\S]*\.key_file\([\s\S]*\.in_memory\(\)[\s\S]*```/,
  );
});
