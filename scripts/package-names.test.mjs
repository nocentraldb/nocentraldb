// Holds the "Package names" table of docs/repository.md: the names the
// project holds on the public registries. The owner's NOC-252 A
// (2026-11-12): `ncdb-plugin` and `ncdb-harness` are reserved on crates.io
// as 0.0.0 placeholders, which the crates in crates/ take over with their
// first real release.
import assert from "node:assert/strict";
import { test } from "node:test";
import { read, section } from "./lib/rfc-text.mjs";

const table = () => section(read("docs/repository.md"), "package-names");

/** The row of the table whose Name cell is `name`. */
const row = (name) =>
  table()
    .split("\n")
    .find(
      (l) => l.startsWith("|") && l.split("|")[2]?.trim() === `\`${name}\``,
    );

test("crates.io holds ncdb-plugin and ncdb-harness as placeholders", () => {
  for (const name of ["ncdb-plugin", "ncdb-harness"]) {
    const line = row(name);
    assert.ok(line, `no row for ${name}`);
    assert.match(line, /^\| crates\.io \|/);
    assert.match(line, /placeholder/);
    assert.match(line, new RegExp(`\`crates/${name}\``));
    assert.match(line, /takes it over with its first real release/);
  }
});
