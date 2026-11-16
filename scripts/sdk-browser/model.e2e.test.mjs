// nc.open checks the model against its lock in Chromium (NOC-323; RFC
// 0009, "Open and close", 2. "Checks the model against the lock"): a model
// with a field the lock does not have is refused with `lockOutOfDate`,
// whose message says to run `ncdb model build`, and no database is
// opened; a model the lock was built from opens. Vitest checks the
// comparison itself (packages/sdk/test/lockCheck.test.ts).
//
// Not part of `pnpm gates`, which runs without browsers: the task
// `repo:sdk-browser` and the workflow browser-view.yml run it.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { after, before, test } from "node:test";
import { chromium } from "playwright";
import { build, LOCK, prebuilt } from "./build.mjs";
import { startServer } from "./server.mjs";

let server;
let browser;

before(async () => {
  if (process.env.SDK_BROWSER_PREBUILT === "1") prebuilt();
  else build();
  server = await startServer({ port: 0 });
  browser = await chromium.launch();
});

after(async () => {
  await browser?.close();
  await server?.close();
});

const lock = JSON.parse(readFileSync(LOCK, "utf8"));

test("nc.open refuses a model its lock was not built from with lockOutOfDate, and opens one it was", async () => {
  const page = await (await browser.newContext()).newPage();
  const errors = [];
  page.on("pageerror", (error) => errors.push(error));
  await page.goto(`${server.url}/`);
  await page.waitForFunction(() => window.ready === true);
  const result = await page.evaluate(async (lock) => {
    const { nc } = window;
    const tasks = (fields) =>
      nc.model({
        tasks: nc.collection({
          title: nc.string(),
          priority: nc.int(),
          ...fields,
        }),
      });
    let refused;
    try {
      await nc.open({ model: tasks({ done: nc.boolean() }), lock, name: "a" });
    } catch (error) {
      refused = {
        isError: error instanceof nc.Error,
        code: error.code,
        message: error.message,
      };
    }
    const db = await nc.open({ model: tasks(), lock, name: "a" });
    await db.close();
    return { refused };
  }, lock);
  assert.equal(result.refused?.isError, true, JSON.stringify(result));
  assert.equal(result.refused.code, "lockOutOfDate");
  assert.match(result.refused.message, /tasks\.done/);
  assert.match(result.refused.message, /ncdb model build/);
  assert.deepEqual(errors, []);
});
