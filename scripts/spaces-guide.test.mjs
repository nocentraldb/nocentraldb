// The guide on cutting spaces and on invitations through an app's backend
// (NOC-473, RFC 0015, "A guide for developers"): its TypeScript snippets
// type-check against the SDK, it names the kinds of spaces the ticket names
// and what adding a member shares, and it points to the example backend,
// whose test runs the journey against the test relay. Its Rust snippets
// compile as doctests of apps/notes-backend.
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { check, snippets } from "./doc-snippets.mjs";
import { root } from "./lib/rfc-text.mjs";

const GUIDE = join(root, "docs", "spaces-and-invitations.md");
const text = () => readFileSync(GUIDE, "utf8");

test("the guide's TypeScript snippets type-check against the SDK", () => {
  const result = check(GUIDE);
  assert.equal(result.ok, true, result.output);
});

test("the guide shows the calls a device uses to tell its spaces apart", () => {
  const code = snippets(text())
    .map((s) => s.code)
    .join("\n");
  for (const name of [
    "admission:",
    "db.account.identity",
    "db.spaces.create()",
    "db.spaces.list()",
    "db.spaces.subscribe(",
    "model.spaceInfo",
    "members.add(",
  ])
    assert.ok(code.includes(name), `no snippet shows ${name}`);
});

test("the guide names each kind of space the ticket names", () => {
  const guide = text();
  for (const kind of [
    "personal space",
    "project space",
    "shared-note space",
    "account space",
  ])
    assert.ok(guide.includes(kind), kind);
});

test("the guide says plainly that adding a member shares everything in the space", () => {
  assert.match(
    text(),
    /[Aa]dding a member to a space shares everything in that space/,
  );
});

test("the guide points to the example backend, which exists with its journey test", () => {
  const guide = text();
  assert.match(guide, /\.\.\/apps\/notes-backend\//);
  for (const file of [
    "apps/notes-backend/src/admission.rs",
    "apps/notes-backend/src/directory.rs",
    "apps/notes-backend/tests/journey.rs",
  ])
    assert.ok(existsSync(join(root, file)), file);
});

test("the example backend compiles the guide's Rust snippets as doctests", () => {
  const lib = readFileSync(join(root, "apps/notes-backend/src/lib.rs"), "utf8");
  assert.match(
    lib,
    /include_str!\("\.\.\/\.\.\/\.\.\/docs\/spaces-and-invitations\.md"\)/,
  );
});
