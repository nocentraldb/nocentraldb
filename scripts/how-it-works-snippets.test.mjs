// The TypeScript snippets of the docs page "How NoCentralDB works"
// (NOC-360, the owner's addition to point 19 of 2026-11-19) type-check
// against the SDK's source, as the browser guide's do (doc-snippets.mjs); the
// doc tests of nocentraldb compile its Rust snippets
// (crates/nocentraldb/src/lib.rs).
import assert from "node:assert/strict";
import { test } from "node:test";
import { check } from "./doc-snippets.mjs";

const PAGE = "site/src/content/docs/docs/how-it-works.md";

test("the TypeScript snippets of How NoCentralDB works type-check", () => {
  const { ok, output, count } = check(PAGE);
  assert.ok(ok, output);
  assert.ok(count >= 3, `only ${count} snippets`);
});
