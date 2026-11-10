// End to end (NOC-174; RFC 0006, "The browser" and "The spike"): the core's
// view runs in a dedicated worker on OPFS, through rusqlite, sqlite-wasm-rs
// and the opfs-sahpool VFS of sqlite-wasm-vfs, in Chromium, Firefox and
// WebKit through Playwright. It folds changes, closes, reopens, survives a
// new page load, and reads back exactly what the same code reads natively.
// In a private window of Firefox, which has no OPFS, it runs on SQLite's
// in-memory VFS (RFC 0006, decision 12).
//
// Not part of `pnpm gates`, which runs without browsers: the task
// `repo:browser-view` and the workflow browser-view.yml run it. A missing
// browser fails its tests, named by engine: install it with
// `pnpm exec playwright install --with-deps chromium firefox webkit`.
import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, before, test } from "node:test";
import { chromium, firefox, webkit } from "playwright";
import { buildHarness, runNative } from "./build.mjs";
import { startServer } from "./server.mjs";

const ENGINES = { chromium, firefox, webkit };
const PATH = "/view.sqlite";
/** The key envelope and the three changes with tasks of the harness. */
const FRAMES = 4;
/** The journal modes of a rollback journal kept in a file. */
const ROLLBACK = ["delete", "truncate", "persist"];

let native;
let server;

before(async () => {
  buildHarness();
  native = runNative();
  server = await startServer({ port: 0 });
});

after(() => server?.close());

const load = async (page) => {
  await page.goto(`${server.url}/`);
  await page.waitForFunction(() => window.ready === true);
  return page;
};

/** Calls the harness export `name` in the page's worker. */
const call = (page, name, ...args) =>
  page.evaluate(([name, args]) => window.inWorker(name, ...args), [name, args]);

const read = async (page) => JSON.parse(await call(page, "read"));

/** A new page load: the old worker ends, a new one starts on first call. */
const reload = async (page) => {
  await page.evaluate(() => window.endWorker());
  await page.reload();
  await page.waitForFunction(() => window.ready === true);
};

test("the native harness reads the tasks back from a file in WAL mode", () => {
  assert.equal(native.read.tasks.length, 30);
  assert.equal(native.read.spaces.length, 1);
  assert.equal(native.journalMode, "wal");
});

for (const [engine, type] of Object.entries(ENGINES)) {
  test(`${engine}: the view on OPFS folds, reopens, survives a new page load and reads what the native view reads`, async () => {
    const browser = await type.launch();
    try {
      const page = await load(await (await browser.newContext()).newPage());
      const opened = await call(page, "open", PATH);
      assert.equal(opened, "opfs", `${engine}: the view is not on OPFS`);
      assert.equal(await call(page, "fold"), FRAMES);
      assert.deepEqual(await read(page), native.read, `${engine}: folded`);
      await call(page, "close");

      assert.equal(await call(page, "open", PATH), "opfs");
      assert.deepEqual(await read(page), native.read, `${engine}: reopened`);
      await call(page, "close");

      await reload(page);
      assert.equal(await call(page, "open", PATH), "opfs");
      const reloaded = await read(page);
      assert.deepEqual(
        reloaded,
        native.read,
        `${engine}: after a new page load`,
      );
      assert.equal(await call(page, "fold"), 0, `${engine}: changes kept`);
      await call(page, "close");

      const mode = await call(page, "journalMode", PATH);
      assert.ok(ROLLBACK.includes(mode), `${engine}: journal mode ${mode}`);
    } finally {
      await browser.close();
    }
  });

  test(`${engine}: the worker's SQLite is the release the native core builds`, async () => {
    const browser = await type.launch();
    try {
      const page = await load(await browser.newPage());
      const sqlite = JSON.parse(await call(page, "sqlite"));
      assert.deepEqual(sqlite, native.sqlite, `${engine}: SQLite differs`);
    } finally {
      await browser.close();
    }
  });

  test(`${engine}: on the page's main thread the view refuses to open`, async () => {
    const browser = await type.launch();
    try {
      const page = await load(await browser.newPage());
      const refused = await page.evaluate(
        (path) =>
          window.onMain("open", path).then(
            () => null,
            (error) => String(error?.message ?? error),
          ),
        PATH,
      );
      assert.match(refused ?? "opened", /dedicated Web Worker/, engine);
    } finally {
      await browser.close();
    }
  });
}

test("firefox: in a private window without OPFS the view runs in memory and is gone after a new page load", async () => {
  const dir = await mkdtemp(join(tmpdir(), "ncdb-browser-view-"));
  const context = await firefox.launchPersistentContext(dir, {
    firefoxUserPrefs: { "browser.privatebrowsing.autostart": true },
  });
  try {
    const page = await load(context.pages()[0] ?? (await context.newPage()));
    assert.equal(await call(page, "open", PATH), "memory");
    assert.equal(await call(page, "fold"), FRAMES);
    assert.deepEqual(await read(page), native.read, "folded in memory");
    await call(page, "close");

    assert.equal(await call(page, "open", PATH), "memory");
    assert.deepEqual(await read(page), native.read, "reopened in the worker");
    await call(page, "close");

    await reload(page);
    assert.equal(await call(page, "open", PATH), "memory");
    assert.deepEqual(await read(page), { spaces: [], tasks: [] });
    await call(page, "close");
  } finally {
    await context.close();
    await rm(dir, { recursive: true, force: true });
  }
});
