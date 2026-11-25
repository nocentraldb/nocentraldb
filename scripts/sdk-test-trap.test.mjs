// The trap for the browser test (NOC-499) never reaches the package: the
// feature `test-trap` of crates/nocentraldb-wasm is off by default, the
// marker it traps on is the one packages/sdk/testTrap.mjs names, and the
// package's build refuses a module that holds it.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import * as testTrap from "../packages/sdk/testTrap.mjs";

const { TEST_TRAP } = testTrap;
const refuseTestTrap = (...args) => testTrap.refuseTestTrap(...args);

const ROOT = join(import.meta.dirname, "..");
const read = (path) => readFileSync(join(ROOT, path), "utf8");

test("the package's build refuses a module that holds the marker, and takes one that does not", () => {
  const bytes = (text) => new TextEncoder().encode(text);
  assert.throws(
    () => refuseTestTrap(bytes(`\0asm…${TEST_TRAP}…`), "core.wasm"),
    /core\.wasm holds the marker of the feature test-trap/,
  );
  assert.doesNotThrow(() => refuseTestTrap(bytes("\0asm…"), "core.wasm"));
});

test("the package's build checks the module it ships", () => {
  assert.match(
    read("packages/sdk/build.mjs"),
    /refuseTestTrap\(\s*readFileSync\(MODULE\)/,
  );
});

test("the core traps on the marker testTrap.mjs names, and only with a feature that is off by default", () => {
  const core = read("crates/nocentraldb-wasm/src/lib.rs");
  assert.match(core, /const TEST_TRAP: &\[u8\] = b"([^"]*)";/);
  assert.equal(
    core.match(/const TEST_TRAP: &\[u8\] = b"([^"]*)";/)[1],
    TEST_TRAP,
  );
  const manifest = read("crates/nocentraldb-wasm/Cargo.toml");
  assert.match(manifest, /^test-trap = \[\]$/m);
  assert.doesNotMatch(manifest, /^default = .*test-trap/m);
});
