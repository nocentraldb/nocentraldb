// End to end (NOC-175 slice 2; NOC-210 A): the Rust SDK's keys in the
// browser, sealed with AES-GCM under a non-extractable WebCrypto key in
// IndexedDB (crates/nocentraldb/src/keys/browser/), in a dedicated worker
// in Chromium through Playwright. The SDK's wasm32 library tests are built
// by build.mjs; wasm-bindgen-test-runner serves the IndexedDB tests, which
// the gates skip in Node, which has no IndexedDB, and Chromium runs them.
//
// Not part of `pnpm gates`, which runs without browsers: the task
// `repo:sdk-keys` and the workflow browser-view.yml run it, the workflow in
// Playwright's image on tests built before (SDK_KEYS_PREBUILT=1). A
// missing Chromium fails it: install it with
// `pnpm exec playwright install chromium`.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createServer } from "node:net";
import { after, before, test } from "node:test";
import { chromium } from "playwright";
import { buildTests, prebuilt, RUNNER } from "./build.mjs";

/** The tests the gates skip in Node, which only a browser runs. */
const FILTER = "keys::browser::tests::indexeddb";

let runner;
let url;

/** A port no one listens on. */
const freePort = () =>
  new Promise((resolve, reject) => {
    const server = createServer();
    server.on("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const { port } = server.address();
      server.close(() => resolve(port));
    });
  });

before(async () => {
  const wasm = prebuilt() ?? buildTests();
  const port = await freePort();
  url = `http://127.0.0.1:${port}`;
  runner = spawn(RUNNER, [wasm, FILTER, "--include-ignored"], {
    env: {
      ...process.env,
      NO_HEADLESS: "1",
      WASM_BINDGEN_USE_DEDICATED_WORKER: "1",
      WASM_BINDGEN_TEST_ADDRESS: `127.0.0.1:${port}`,
    },
    stdio: ["ignore", "pipe", "inherit"],
  });
  await new Promise((resolve, reject) => {
    let out = "";
    runner.stdout.on("data", (chunk) => {
      out += chunk;
      if (out.includes("available at")) resolve();
    });
    runner.on("exit", (code) =>
      reject(new Error(`wasm-bindgen-test-runner ended (${code}): ${out}`)),
    );
  });
});

after(() => runner?.kill());

test("the IndexedDB key tests pass in a dedicated worker in Chromium", async () => {
  const browser = await chromium.launch();
  try {
    const page = await browser.newPage();
    await page.goto(url);
    await page.waitForFunction(
      () =>
        /test result: /.test(
          document.querySelector("#output")?.textContent ?? "",
        ),
      null,
      { timeout: 120_000 },
    );
    const output = await page.textContent("#output");
    assert.match(
      output,
      /test result: ok\. [1-9]\d* passed; 0 failed; 0 ignored/,
      output,
    );
    assert.doesNotMatch(output, /\.\.\. FAILED/, output);
  } finally {
    await browser.close();
  }
});
