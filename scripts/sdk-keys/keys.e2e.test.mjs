// End to end (NOC-175 slice 2; NOC-210 A): the Rust SDK's keys in the
// browser, sealed with AES-GCM under a non-extractable WebCrypto key in
// IndexedDB (crates/nocentraldb/src/keys/browser/), in a dedicated worker
// in Chromium through Playwright, the SDK's database on OPFS there
// (crates/nocentraldb/src/browser/), and syncing from there over the
// browser's WebSocket and fetch() with the local relay server, which this
// test starts on a fresh database at RELAY_URL, from another origin than
// the page's (crates/nocentraldb/src/browser/sync_tests.rs). The SDK's wasm32 library tests are built
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
import { createPrivateKey, createPublicKey } from "node:crypto";
import { mkdtempSync, rmSync } from "node:fs";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, before, test } from "node:test";
import { chromium } from "playwright";
import { buildTests, prebuilt, RELAY, RELAY_URL, RUNNER } from "./build.mjs";

/**
 * The SDK's browser tests: the keys in IndexedDB (keys::browser::tests) and
 * the database on OPFS (browser::tests). The gates run the first module in
 * Node and skip what needs IndexedDB, OPFS or a worker; Chromium runs all.
 */
const FILTER = "browser::tests::";

let runner;
let relay;
let relayDir;
let url;

/**
 * The public key, in hex, of the account whose members may create spaces
 * on the relay: the secret 32 bytes of 0x41 that sync_tests.rs signs with.
 */
const accountKey = () => {
  const pkcs8 = Buffer.concat([
    Buffer.from("302e020100300506032b657004220420", "hex"),
    Buffer.alloc(32, 0x41),
  ]);
  const secret = createPrivateKey({ key: pkcs8, format: "der", type: "pkcs8" });
  const spki = createPublicKey(secret).export({ format: "der", type: "spki" });
  return spki.subarray(-32).toString("hex");
};

/** Starts the local relay server at RELAY_URL and waits until it listens. */
const startRelay = async () => {
  relayDir = mkdtempSync(join(tmpdir(), "sdk-keys-relay-"));
  const listen = new URL(RELAY_URL).host;
  relay = spawn(
    RELAY,
    [
      "--database",
      join(relayDir, "relay.sqlite"),
      "--listen",
      listen,
      "--allow-account",
      accountKey(),
    ],
    { stdio: ["ignore", "pipe", "inherit"] },
  );
  await new Promise((resolve, reject) => {
    let out = "";
    relay.stdout.on("data", (chunk) => {
      out += chunk;
      if (out.includes("\n")) resolve();
    });
    relay.on("exit", (code) =>
      reject(new Error(`ncdb-relay ended (${code}): ${out}`)),
    );
  });
};

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
  await startRelay();
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

after(() => {
  runner?.kill();
  relay?.kill();
  if (relayDir) rmSync(relayDir, { recursive: true, force: true });
});

test("the SDK's browser tests pass in a dedicated worker in Chromium", async () => {
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
    // The sync tests need the relay this test started: each must have run.
    for (const name of [
      "two_databases_share_a_space_through_the_relay_over_the_browsers_websocket",
      "a_large_change_goes_up_and_comes_down_through_fetch_from_another_origin",
    ]) {
      assert.match(
        output,
        new RegExp(`browser::tests::sync::${name} \\.\\.\\. ok`),
        output,
      );
    }
  } finally {
    await browser.close();
  }
});
