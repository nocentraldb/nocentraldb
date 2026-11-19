// The order of the landing page's demo (NOC-360, the owner's point 17 of
// 2026-11-19): the newest task first, by a creation time and an index that
// sorts by it descending. Visitors keep databases made by the demo's first
// model, which had neither: such a database opens with the new model, whose
// lock is an additive successor of the old one (RFC 0005, "Two checks: at
// build and on the device"), and its tasks, which have no creation time,
// come last, by title. The old demo and its lock are fixtures
// (fixtures/demo-v1.js, fixtures/demo-v1.lock.json), served in place of the
// new ones. Chromium through Playwright; not part of `pnpm gates`, as the
// other files here.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { after, before } from "node:test";
import { chromium } from "playwright";
import { build } from "./build.mjs";
import { startServer } from "./server.mjs";
import { endsWhenDone, test } from "./teardown.mjs";

endsWhenDone();

const fixture = (name) =>
  readFileSync(new URL(`./fixtures/${name}`, import.meta.url), "utf8");

let server;
let browser;

before(async () => {
  build();
  server = await startServer({ port: 0 });
  browser = await chromium.launch();
});

after(async () => {
  try {
    await browser?.close();
  } finally {
    await server?.close();
  }
});

/** Opens the landing page in `context`, scrolls to the demo and waits until it runs. */
const open = async (context) => {
  const page = await context.newPage();
  const errors = [];
  page.on("pageerror", (error) => errors.push(error));
  await page.goto(`${server.url}/`);
  await page.waitForLoadState("load");
  await page.locator("#demo").scrollIntoViewIfNeeded();
  await page.waitForSelector("[data-demo-status][data-state='ready']", {
    timeout: 60_000,
  });
  return { page, errors };
};

/** Adds a task through the demo's form and waits until the open list shows it. */
const add = async (page, title) => {
  await page.fill("#demo-add input[name=title]", title);
  await page.click("#demo-add button[type=submit]");
  await page.waitForFunction(
    (t) =>
      [...document.querySelectorAll("#demo-open li .title")].some(
        (title) => title.textContent === t,
      ),
    title,
  );
};

/** The titles of the open list, top to bottom. */
const openTitles = (page) =>
  page.$$eval("#demo-open li .title", (titles) =>
    titles.map((title) => title.textContent),
  );

test("the open list shows the newest task first", async () => {
  const context = await browser.newContext();
  const { page, errors } = await open(context);
  for (const title of ["Buy milk", "Write the README", "Call the bank"]) {
    await add(page, title);
    await page.waitForTimeout(20);
  }
  assert.deepEqual(await openTitles(page), [
    "Call the bank",
    "Write the README",
    "Buy milk",
  ]);
  assert.deepEqual(errors, []);
  await context.close();
});

test("a database made by the old demo opens with the new one, its tasks last, by title", async () => {
  const context = await browser.newContext();
  await context.route("**/demo/demo.js", (route) =>
    route.fulfill({
      contentType: "text/javascript",
      body: fixture("demo-v1.js"),
    }),
  );
  await context.route("**/demo/model.lock.json", (route) =>
    route.fulfill({
      contentType: "application/json",
      body: fixture("demo-v1.lock.json"),
    }),
  );
  const old = await open(context);
  await add(old.page, "Zebra crossing");
  await add(old.page, "Apple pie");
  assert.deepEqual(old.errors, []);
  await old.page.close();

  await context.unrouteAll();
  const { page, errors } = await open(context);
  await add(page, "New task");
  assert.deepEqual(await openTitles(page), [
    "New task",
    "Apple pie",
    "Zebra crossing",
  ]);
  assert.deepEqual(errors, []);
  await context.close();
});
