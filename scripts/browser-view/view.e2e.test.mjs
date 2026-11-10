// End to end (NOC-174; RFC 0006, "The browser" and "The spike"): the core's
// view runs in a dedicated worker on OPFS, through rusqlite, sqlite-wasm-rs
// and the opfs-sahpool VFS of sqlite-wasm-vfs, in Chromium, Firefox and
// WebKit through Playwright. It folds changes, closes, reopens, survives a
// new page load, and reads back exactly what the same code reads natively.
// In a private window of Firefox or WebKit, which have no OPFS, it runs on
// SQLite's in-memory VFS (RFC 0006, decision 12).
//
// Windows are opened as NOC-33's probe opens them
// (scripts/browser-storage/measure.mjs): a normal window is a persistent
// context on a profile directory, since WebKit's non-persistent context is
// an ephemeral data store, its private window. Firefox's private window is
// its private browsing mode, `browser.privatebrowsing.autostart`, which
// takes effect from a profile's second launch.
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

/** A normal window of `type` on a fresh profile; `run` gets its page. */
const inNormalWindow = async (type, run) => {
  const dir = await mkdtemp(join(tmpdir(), "ncdb-browser-view-"));
  const context = await type.launchPersistentContext(dir);
  try {
    await run(await load(context.pages()[0] ?? (await context.newPage())));
  } finally {
    await context.close();
    await rm(dir, { recursive: true, force: true });
  }
};

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
    await inNormalWindow(type, async (page) => {
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
    });
  });

  test(`${engine}: the worker's SQLite is the release the native core builds`, async () => {
    await inNormalWindow(type, async (page) => {
      const sqlite = JSON.parse(await call(page, "sqlite"));
      assert.deepEqual(sqlite, native.sqlite, `${engine}: SQLite differs`);
    });
  });

  test(`${engine}: on the page's main thread the view refuses to open`, async () => {
    await inNormalWindow(type, async (page) => {
      const refused = await page.evaluate(
        (path) =>
          window.onMain("open", path).then(
            () => null,
            (error) => String(error?.message ?? error),
          ),
        PATH,
      );
      assert.match(refused ?? "opened", /dedicated Web Worker/, engine);
    });
  });
}

/** The view in a private window without OPFS: in memory, gone on reload. */
const inMemoryUntilReload = async (page, engine) => {
  assert.equal(await call(page, "open", PATH), "memory", `${engine}: OPFS`);
  assert.equal(await call(page, "fold"), FRAMES);
  assert.deepEqual(await read(page), native.read, `${engine}: in memory`);
  await call(page, "close");

  assert.equal(await call(page, "open", PATH), "memory");
  assert.deepEqual(await read(page), native.read, `${engine}: reopened`);
  await call(page, "close");

  await reload(page);
  assert.equal(await call(page, "open", PATH), "memory");
  const empty = { spaces: [], tasks: [] };
  assert.deepEqual(await read(page), empty, `${engine}: after a reload`);
  await call(page, "close");
};

test("firefox: in a private window without OPFS the view runs in memory and is gone after a new page load", async () => {
  const dir = await mkdtemp(join(tmpdir(), "ncdb-browser-view-"));
  const options = {
    firefoxUserPrefs: { "browser.privatebrowsing.autostart": true },
  };
  // A fresh profile's first launch ignores the autostart pref, as NOC-33's
  // probe found, so one launch sets the profile up first.
  await (await firefox.launchPersistentContext(dir, options)).close();
  const context = await firefox.launchPersistentContext(dir, options);
  try {
    const page = await load(context.pages()[0] ?? (await context.newPage()));
    await inMemoryUntilReload(page, "firefox");
  } finally {
    await context.close();
    await rm(dir, { recursive: true, force: true });
  }
});

test("webkit: in a private window (an ephemeral context) without OPFS the view runs in memory and is gone after a new page load", async () => {
  const browser = await webkit.launch();
  try {
    const page = await load(await (await browser.newContext()).newPage());
    await inMemoryUntilReload(page, "webkit");
  } finally {
    await browser.close();
  }
});
