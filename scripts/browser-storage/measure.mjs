// Drives the probe page (page/) with Playwright in one browser engine and
// returns what it saw. Each scenario writes a fresh token and reads it back
// after the event, from SQLite on OPFS and from the IndexedDB control:
//
// - persist(), persisted() and estimate(), in a normal profile on disk;
// - a reload of the page;
// - a visit to /clear-cache, which answers `Clear-Site-Data: "cache"`;
// - a browser restart on the same profile directory;
// - a private window: Chromium's and WebKit's non-persistent contexts (an
//   off-the-record profile and an ephemeral data store), and Firefox with
//   `browser.privatebrowsing.autostart`, which is its private browsing mode
//   (Playwright's Firefox contexts are not private on their own).
//   "After close" reads in a new private window: a new context of the same
//   browser, or for Firefox a restart on the same profile directory.
//
// Playwright's builds are not the branded browsers: its WebKit is not
// Safari (no ITP, so no seven-day deletion) and its Chromium is not Chrome.
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { chromium, firefox, webkit } from "playwright";
import { startServer } from "./server.mjs";

const PERSIST_TIMEOUT_MS = 5000;

const open = async (context, url) => {
  const page = context.pages()[0] ?? (await context.newPage());
  await page.goto(`${url}/`);
  await page.waitForFunction(() => window.probe !== undefined);
  return page;
};

const write = (page, token) =>
  page.evaluate((t) => window.probe.write(t), token);
const read = (page) => page.evaluate(() => window.probe.read());

const survived = (value, token) => {
  if (value === token) return "kept";
  if (value === null || value === undefined) return "lost";
  if (typeof value === "string" && value.startsWith("error: ")) return value;
  return `error: unexpected value ${JSON.stringify(value)}`;
};

const survival = (values, token) => ({
  sqlite: survived(values.sqlite, token),
  idb: survived(values.idb, token),
});

const outcome = (value) =>
  value === "ok" ? "ok" : String(value).replace(/^error: /, "");

let counter = 0;
const token = () => `token-${process.pid}-${Date.now()}-${counter++}`;

/** Writes a token, runs the event, and reads it back on the page it returns. */
const across = async (page, event) => {
  const value = token();
  await write(page, value);
  const after = await event(page);
  return survival(await read(after), value);
};

const normalProfile = async (type, url, dir) => {
  let context = await type.launchPersistentContext(dir);
  try {
    let page = await open(context, url);
    const persist = await page.evaluate(
      (ms) => window.probe.persist(ms),
      PERSIST_TIMEOUT_MS,
    );
    const written = await write(page, token());
    const info = await page.evaluate(() => window.probe.info());
    const reload = await across(page, async (p) => {
      await p.reload();
      await p.waitForFunction(() => window.probe !== undefined);
      return p;
    });
    const clearCache = await across(page, async (p) => {
      await p.goto(`${url}/clear-cache`);
      await p.goto(`${url}/`);
      await p.waitForFunction(() => window.probe !== undefined);
      return p;
    });
    const value = token();
    await write(page, value);
    await context.close();
    context = await type.launchPersistentContext(dir);
    page = await open(context, url);
    const restart = survival(await read(page), value);
    return {
      persist: { answered: persist.answered, granted: persist.granted },
      persisted: info.persisted === true,
      estimate: info.estimate,
      opfs: outcome(written.sqlite),
      reload,
      clearCache,
      restart,
    };
  } finally {
    await context.close();
  }
};

const privateWindow = async (engine, type, url, dir) => {
  if (engine === "firefox") {
    const options = {
      firefoxUserPrefs: { "browser.privatebrowsing.autostart": true },
    };
    // A fresh profile's first launch ignores the autostart pref (OPFS still
    // works there), so one launch sets the profile up before the measured ones.
    await (await type.launchPersistentContext(dir, options)).close();
    let context = await type.launchPersistentContext(dir, options);
    try {
      const page = await open(context, url);
      const value = token();
      const written = await write(page, value);
      await context.close();
      context = await type.launchPersistentContext(dir, options);
      const after = survival(await read(await open(context, url)), value);
      return {
        opfs: outcome(written.sqlite),
        idb: outcome(written.idb),
        afterClose: after,
      };
    } finally {
      await context.close();
    }
  }
  const browser = await type.launch();
  try {
    const first = await browser.newContext();
    const value = token();
    const written = await write(await open(first, url), value);
    await first.close();
    const second = await browser.newContext();
    const after = survival(await read(await open(second, url)), value);
    await second.close();
    return {
      opfs: outcome(written.sqlite),
      idb: outcome(written.idb),
      afterClose: after,
    };
  } finally {
    await browser.close();
  }
};

/** Measures one engine: "chromium", "firefox" or "webkit". */
export const measure = async (engine) => {
  const type = { chromium, firefox, webkit }[engine];
  if (!type) throw new Error(`unknown engine ${engine}`);
  const server = await startServer({ port: 0 });
  const dirs = [];
  const dir = async () => {
    const path = await mkdtemp(join(tmpdir(), `ncdb-probe-${engine}-`));
    dirs.push(path);
    return path;
  };
  try {
    const browser = await type.launch();
    const version = browser.version();
    await browser.close();
    const normal = await normalProfile(type, server.url, await dir());
    return {
      version,
      ...normal,
      private: await privateWindow(engine, type, server.url, await dir()),
    };
  } finally {
    await server.close();
    await Promise.all(
      dirs.map((path) => rm(path, { recursive: true, force: true })),
    );
  }
};
