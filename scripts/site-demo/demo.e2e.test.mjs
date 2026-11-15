// The landing page's live demo (NOC-319) in Chromium through Playwright:
// the page renders first, and only then does the demo download the SDK,
// its WebAssembly core and its worker; it shows that it is loading, runs a
// task list on the visitor's own OPFS, shares it with a second tab of the
// page (NOC-177), and says so plainly when it cannot run.
//
// Not part of `pnpm gates`, which runs without browsers: the task
// `repo:site-demo` and the workflow browser-view.yml run it, the workflow
// on the SDK built before (SDK_BROWSER_PREBUILT=1). A missing Chromium
// fails it: install it with `pnpm exec playwright install chromium`.
import assert from "node:assert/strict";
import { after, before, test } from "node:test";
import { chromium } from "playwright";
import { build } from "./build.mjs";
import { startServer } from "./server.mjs";

let server;
let browser;

before(async () => {
  build();
  server = await startServer({ port: 0 });
  browser = await chromium.launch();
});

after(async () => {
  await browser?.close();
  await server?.close();
});

/** The demo's state, as its status element says it. */
const state = (page) =>
  page.locator("[data-demo-status]").getAttribute("data-state");

/** Scrolls the demo's section into view, which starts the demo. */
const reach = (page) => page.locator("#demo").scrollIntoViewIfNeeded();

/** Opens the landing page in `context` (a fresh one: a fresh OPFS) and scrolls to the demo. */
const open = async (context) => {
  const page = await (context ?? (await browser.newContext())).newPage();
  const errors = [];
  page.on("pageerror", (error) => errors.push(error));
  await page.goto(`${server.url}/`);
  await page.waitForLoadState("load");
  await reach(page);
  return { page, errors };
};

/** Waits until the demo has opened its database and shows its lists. */
const ready = (page) =>
  page.waitForSelector("[data-demo-status][data-state='ready']", {
    timeout: 60_000,
  });

/** The titles a list of the demo shows, in order. */
const titles = (page, list) =>
  page.locator(`#demo-${list} li .title`).allTextContents();

/** Adds a task by typing it into the demo's form. */
const add = async (page, title) => {
  await page.fill("#demo-add input[name=title]", title);
  await page.click("#demo-add button[type=submit]");
};

test("the page renders and loads before the demo requests anything, and the demo waits to be reached", async () => {
  const context = await browser.newContext();
  const page = await context.newPage();
  const requested = [];
  page.on("request", (request) => {
    if (new URL(request.url()).pathname.startsWith("/demo/"))
      requested.push(request.url());
  });
  await page.goto(`${server.url}/`);
  await page.waitForLoadState("load");
  await page.waitForTimeout(1500);
  assert.deepEqual(
    requested,
    [],
    "nothing of the demo before the visitor reaches it",
  );
  assert.equal(await state(page), "waiting");
  await reach(page);
  await ready(page);
  const timing = await page.evaluate(() => {
    const [nav] = performance.getEntriesByType("navigation");
    const paint = performance
      .getEntriesByType("paint")
      .find((entry) => entry.name === "first-contentful-paint");
    const demo = performance
      .getEntriesByType("resource")
      .filter((entry) => new URL(entry.name).pathname.startsWith("/demo/"))
      .map((entry) => entry.startTime);
    return { paint: paint.startTime, load: nav.loadEventEnd, demo };
  });
  assert.ok(timing.demo.length > 0, "the demo downloads its assets");
  for (const start of timing.demo) {
    assert.ok(
      start >= timing.paint,
      `a demo request at ${start} ms, before the first paint`,
    );
    assert.ok(
      start >= timing.load,
      `a demo request at ${start} ms, before the load event`,
    );
  }
  await context.close();
});

test("it shows that it is loading, and what, while the SDK downloads", async () => {
  const context = await browser.newContext();
  let release;
  const held = new Promise((done) => {
    release = done;
  });
  await context.route("**/demo/sdk/**", async (route) => {
    await held;
    await route.continue();
  });
  const { page, errors } = await open(context);
  await page.waitForSelector("[data-demo-status][data-state='loading']");
  const text = await page.locator("[data-demo-status]").textContent();
  assert.match(text, /loading/i);
  assert.match(text, /WebAssembly|SDK/);
  release();
  await ready(page);
  assert.deepEqual(errors, []);
  await context.close();
});

test("the demo keeps tasks in the visitor's browser, and a tick marks one done", async () => {
  const { page, errors } = await open();
  await ready(page);
  await add(page, "Water the plants");
  await page.waitForFunction(() =>
    [...document.querySelectorAll("#demo-open li .title")].some(
      (title) => title.textContent === "Water the plants",
    ),
  );
  await page
    .locator("#demo-open li", { hasText: "Water the plants" })
    .locator("input[type=checkbox]")
    .check();
  await page.waitForFunction(() =>
    [...document.querySelectorAll("#demo-done li .title")].some(
      (title) => title.textContent === "Water the plants",
    ),
  );
  assert.deepEqual(await titles(page, "open"), []);
  assert.deepEqual(errors, []);
  await page.context().close();
});

test("a second tab of the page shows the same tasks", async () => {
  const context = await browser.newContext();
  const { page: first, errors } = await open(context);
  const { page: second, errors: secondErrors } = await open(context);
  await ready(first);
  await ready(second);
  await add(first, "Seen in both tabs");
  await second.waitForFunction(
    () =>
      [...document.querySelectorAll("#demo-open li .title")].some(
        (title) => title.textContent === "Seen in both tabs",
      ),
    null,
    { timeout: 30_000 },
  );
  await add(second, "Added in the second tab");
  await first.waitForFunction(
    () =>
      [...document.querySelectorAll("#demo-open li .title")].some(
        (title) => title.textContent === "Added in the second tab",
      ),
    null,
    { timeout: 30_000 },
  );
  assert.deepEqual(errors, []);
  assert.deepEqual(secondErrors, []);
  await context.close();
});

test("a failed download shows the failure, and the rest of the page is unaffected", async () => {
  const context = await browser.newContext();
  await context.route("**/demo/sdk/**", (route) => route.abort());
  const { page, errors } = await open(context);
  await page.waitForSelector("[data-demo-status][data-state='failed']", {
    timeout: 30_000,
  });
  const text = await page.locator("[data-demo-status]").textContent();
  assert.match(text, /could not load/i);
  assert.ok(await page.locator("h1").first().isVisible());
  assert.equal(await page.locator("#demo-add").isVisible(), false);
  assert.deepEqual(errors, []);
  await context.close();
});

test("a browser without OPFS is told so, and downloads nothing for the demo", async () => {
  const context = await browser.newContext();
  await context.addInitScript(() => {
    delete StorageManager.prototype.getDirectory;
  });
  const requested = [];
  context.on("request", (request) => {
    if (new URL(request.url()).pathname.startsWith("/demo/sdk/")) {
      requested.push(request.url());
    }
  });
  const { page, errors } = await open(context);
  await page.waitForSelector("[data-demo-status][data-state='unsupported']", {
    timeout: 30_000,
  });
  const text = await page.locator("[data-demo-status]").textContent();
  assert.match(text, /OPFS|private file system/i);
  assert.deepEqual(requested, []);
  assert.deepEqual(errors, []);
  assert.equal(await state(page), "unsupported");
  await context.close();
});
