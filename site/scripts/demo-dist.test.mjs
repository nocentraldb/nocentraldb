// Checks that the built site carries the live demo's files (NOC-319): the
// demo module, the example's lock, and the SDK it loads, its worker and its
// WebAssembly core. The deploy (site-pages.yml) sets SITE_DEMO_REQUIRED=1,
// so it never ships the landing page without them; `pnpm gates` builds the
// site without the SDK, so there the test is skipped.
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const demo = join(
  dirname(fileURLToPath(import.meta.url)),
  "..",
  "dist",
  "demo",
);
const FILES = [
  "demo.js",
  "model.lock.json",
  "sdk/index.js",
  "sdk/worker.js",
  "sdk/wasm/nocentraldb_wasm.js",
  "sdk/wasm/nocentraldb_wasm_bg.wasm",
];
const required = process.env.SITE_DEMO_REQUIRED === "1";
const built = existsSync(join(demo, "sdk"));

test("the built site carries the demo, its SDK and its WebAssembly core", {
  skip: !required && !built && "the demo's assets are not built here",
}, () => {
  for (const file of FILES) {
    assert.ok(existsSync(join(demo, file)), `dist/demo/${file} is missing`);
  }
  // A WebAssembly module starts with "\0asm".
  const wasm = readFileSync(join(demo, "sdk/wasm/nocentraldb_wasm_bg.wasm"));
  assert.deepEqual([...wasm.subarray(0, 4)], [0, 0x61, 0x73, 0x6d]);
  assert.deepEqual(
    Object.keys(
      JSON.parse(readFileSync(join(demo, "model.lock.json"), "utf8"))
        .collections ?? {},
    ),
    ["tasks"],
  );
});

test("no type declarations ship with the demo", { skip: !built }, () => {
  assert.ok(!existsSync(join(demo, "sdk/index.d.ts")));
});
