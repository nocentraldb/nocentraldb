// NOC-444: a database open in a Chromium private window survives eleven
// minutes without a call. In an off-the-record context Chromium keeps the
// origin's OPFS in memory and drops it after about ten minutes without a
// file operation (measured in Chromium 157: kept after 450 s, gone after
// 600 s): every access handle then fails, and the files are gone. The
// next write failed with `storageFailed: the local view failed: SQLite
// failed: disk I/O error`, and the database opened again was empty. A
// worker that holds a database open keeps its OPFS alive (worker.ts,
// keepAlive.ts). A Chromium private window is what Playwright's
// `browser.newContext()` opens, so app test suites run there.
//
// Chromium only: Firefox and WebKit have no OPFS in a private window and
// refuse to open there (private.e2e.test.mjs). The test idles for eleven
// minutes, which no setting of Chromium's shortens.
//
// Not part of `pnpm gates`, which runs without browsers: the task
// `repo:sdk-browser-long` and the long job of browser-view.yml run it
// (NOC-448).
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { after, before, test } from "node:test";
import { build, LOCK, prebuilt } from "../build.mjs";
import { engine, launchPrivate } from "../engine.mjs";
import { startServer } from "../server.mjs";

/** Longer than the about ten minutes after which Chromium drops an idle off-the-record OPFS. */
const IDLE_MS = 11 * 60 * 1000;

let server;
let browser;

const chromium = engine === "chromium";

before(async () => {
  if (!chromium) return;
  if (process.env.SDK_BROWSER_PREBUILT === "1") prebuilt();
  else build();
  server = await startServer({ port: 0 });
  browser = await launchPrivate();
});

after(async () => {
  await browser?.close();
  await server?.close();
});

const lock = JSON.parse(readFileSync(LOCK, "utf8"));

const MODEL = `nc.model({
  tasks: nc.collection({ title: nc.string(), priority: nc.int() }),
})`;

test("a database open in a Chromium private window writes after eleven idle minutes, and holds its data when opened again", {
  skip: chromium
    ? false
    : "Firefox and WebKit have no OPFS in a private window (private.e2e.test.mjs)",
  timeout: IDLE_MS + 5 * 60 * 1000,
}, async () => {
  const page = await (await browser.newContext()).newPage();
  const errors = [];
  page.on("pageerror", (error) => errors.push(error));
  await page.goto(`${server.url}/`);
  await page.waitForFunction(() => window.ready === true);
  const result = await page.evaluate(
    async ({ lock, MODEL, IDLE_MS }) => {
      const { nc } = window;
      const model = new Function("nc", `return ${MODEL}`)(nc);
      const outcome = (promise) =>
        promise.then(
          () => "ok",
          (e) => `${e.code}: ${e.message}`,
        );
      const db = await nc.open({ model, lock, name: "idle" });
      const project = await db.spaces.create();
      const { ids } = await project.create(model.tasks, {
        title: "before",
        priority: 1,
      });
      await new Promise((resolve) => setTimeout(resolve, IDLE_MS));
      const afterIdle = await outcome(
        project.create(model.tasks, { title: "after", priority: 2 }),
      );
      await db.close();
      const again = await nc.open({ model, lock, name: "idle" });
      const title = await (async () =>
        (await (await again.spaces.get(project.id)).get(model.tasks, ids[0]))
          ?.title)().catch((e) => `${e.code}: ${e.message}`);
      await again.close();
      return { afterIdle, title };
    },
    { lock, MODEL, IDLE_MS },
  );
  assert.deepEqual(result, { afterIdle: "ok", title: "before" });
  assert.deepEqual(errors, []);
});
