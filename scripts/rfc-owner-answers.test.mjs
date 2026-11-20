// The owner's answers to NOC-362's seven questions (Carsten, 2026-11-20):
// the glossary's link stays in its space and a reference has its own row;
// ncdb-harness is published after 0.1; 0.1's Db::erase is an interim call
// the work-order RFC replaces; RFC 0003's two later notices stay, marked
// later; plugin fields are Rust only in v1; key 0 is "must understand"; and
// a former member's own change pushed again is answered `duplicate`.
import assert from "node:assert/strict";
import { test } from "node:test";
import { read, section } from "./lib/rfc-text.mjs";

const rfc = (n) =>
  read(
    {
      "0001": "docs/rfc/0001-nocentraldb.md",
      "0002": "docs/rfc/0002-change-and-log-format.md",
      "0003": "docs/rfc/0003-keys-and-cryptography.md",
      "0005": "docs/rfc/0005-schema-and-model-lock.md",
      "0009": "docs/rfc/0009-sdk-api.md",
      "0011": "docs/rfc/0011-plugin-field-types.md",
    }[n],
  );

/** The glossary row whose Term cell is `term`. */
const row = (term) =>
  read("docs/glossary.md")
    .split("\n")
    .find((l) => l.startsWith(`| ${term} |`));

test("1: the glossary keeps a link in its space and has a reference row", () => {
  const link = row("link");
  assert.ok(link, "no link row");
  assert.doesNotMatch(link, /possibly in another space/);
  assert.match(link, /in the same space/);
  const reference = row("reference");
  assert.ok(reference, "no reference row");
  assert.match(reference, /`nc\.ref`/);
  assert.match(reference, /another space/);
  assert.match(row("field type"), /references/);
});

test("2: ncdb-harness is published after 0.1", () => {
  const text = section(rfc("0011"), "checking-a-plugin-ncdb-harness");
  assert.match(text, /published to crates\.io after 0\.1/);
  assert.doesNotMatch(rfc("0011"), /a public crate that runs/);
});

test("3: 0.1's Db::erase is an interim call the work-order RFC replaces", () => {
  for (const n of ["0001", "0003", "0009"]) {
    assert.match(
      rfc(n),
      /`Db::erase`[^.]*interim call[^.]*work-order RFC will replace/,
      `RFC ${n}`,
    );
  }
  assert.doesNotMatch(rfc("0009"), /may wrap/);
});

test("4: RFC 0003 keeps its two notices, marked later", () => {
  assert.match(
    rfc("0003"),
    /notices of moving data between spaces and of invitations come later/,
  );
});

test("5: plugin fields are Rust only in v1, and the example says so", () => {
  const text = section(rfc("0005"), "the-schema-in-typescript");
  assert.match(text, /In v1 a plugin field is Rust only/);
  assert.match(text, /votes: counter\(\), \/\/ Rust only in v1/);
});

test("6: key 0 is must understand", () => {
  assert.match(rfc("0002"), /Keys 0 to 63 are "must understand"/);
});

test("7: RFC 0002 names a former member's own push as the exception", () => {
  assert.match(
    rfc("0002"),
    /its own author[^.]*`duplicate`[^.]*even after it left the space/,
  );
});
