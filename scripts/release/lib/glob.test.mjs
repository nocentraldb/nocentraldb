import assert from "node:assert/strict";
import { test } from "node:test";
import { matchesAny } from "./glob.mjs";

const yes = [
  ["docs/a/b.md", "docs/**"],
  ["README.md", "README.md"],
  ["LICENSE-MIT", "LICENSE-*"],
  ["crates/x/tests/t.rs", "**/tests/**"],
  ["packages/sdk/test/a.ts", "**/test/**"],
  ["scripts/a.test.mjs", "**/*.test.*"],
  ["packages/sdk/moon.yml", "**/moon.yml"],
  ["moon.yml", "**/moon.yml"],
  ["changes/x.md", "changes/**"],
];
const no = [
  ["crates/x/src/lib.rs", "**/tests/**"],
  ["Cargo.toml", "docs/**"],
  ["packages/sdk/package.json", "**/*.test.*"],
  ["src/testing.rs", "**/test/**"],
  ["READMEx.md", "README.md"],
  ["a/moon.yml.bak", "**/moon.yml"],
];
for (const [p, g] of yes)
  test(`${p} matches ${g}`, () => assert.ok(matchesAny(p, [g])));
for (const [p, g] of no)
  test(`${p} does not match ${g}`, () => assert.ok(!matchesAny(p, [g])));
