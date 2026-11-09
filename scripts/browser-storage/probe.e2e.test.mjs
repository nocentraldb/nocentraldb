// End to end: the probe page runs in Chromium, Firefox and WebKit through
// Playwright and reports every behaviour NOC-33 asks for. Not part of
// `pnpm gates`, which runs without browsers; the task
// `repo:browser-storage` and the workflow browser-storage.yml run it.
// A missing browser fails the test: install it with
// `pnpm exec playwright install --with-deps chromium firefox webkit`.
import assert from "node:assert/strict";
import { test } from "node:test";
import { measure } from "./measure.mjs";
import { startServer } from "./server.mjs";

const SURVIVAL = /^(kept|lost|error: .+)$/;

const assertSurvival = (value, label) => {
  assert.match(value.sqlite, SURVIVAL, `${label}: SQLite`);
  assert.match(value.idb, SURVIVAL, `${label}: IndexedDB`);
};

test("the server serves the page, the wasm build and a cache clear", async () => {
  const server = await startServer({ port: 0 });
  try {
    const page = await fetch(`${server.url}/`);
    assert.equal(page.status, 200);
    assert.match(page.headers.get("content-type"), /^text\/html/);
    assert.match(await page.text(), /<script type="module" src="probe\.js">/);

    const wasm = await fetch(`${server.url}/sqlite/sqlite3.wasm`);
    assert.equal(wasm.status, 200);
    assert.equal(wasm.headers.get("content-type"), "application/wasm");

    const clear = await fetch(`${server.url}/clear-cache`);
    assert.equal(clear.status, 200);
    assert.equal(clear.headers.get("clear-site-data"), '"cache"');

    const outside = await fetch(`${server.url}/sqlite/..%2f..%2fpackage.json`);
    assert.equal(outside.status, 404);
  } finally {
    await server.close();
  }
});

for (const engine of ["chromium", "firefox", "webkit"]) {
  test(`${engine} reports every behaviour`, { timeout: 180_000 }, async () => {
    const result = await measure(engine);
    assert.match(result.version, /^\d+(\.\d+)*/);
    assert.equal(typeof result.persist.answered, "boolean");
    if (result.persist.answered) {
      assert.equal(typeof result.persist.granted, "boolean");
    }
    assert.equal(typeof result.persisted, "boolean");
    assert.equal(typeof result.estimate.quota, "number");
    assert.equal(typeof result.estimate.usage, "number");
    assert.equal(typeof result.opfs, "string");
    assertSurvival(result.reload, "reload");
    assertSurvival(result.restart, "restart");
    assertSurvival(result.clearCache, "cleared cache");
    assert.equal(typeof result.private.opfs, "string");
    assert.equal(typeof result.private.idb, "string");
    assertSurvival(result.private.afterClose, "private window closed");
  });
}
