// The example app in Chromium through Playwright, as a person uses it: it
// adds tasks, marks one done, deletes one, pages a list and reloads. Each
// list is a subscription to a query of the index `byStatus`, so what the
// page shows after a write is what the subscription gave it (RFC 0009,
// "Queries and indexes", "Subscribe").
//
// Not part of `pnpm gates`, which runs without browsers: the task
// `tasks-ts:browser` and the workflow browser-view.yml run it, the workflow
// in Playwright's image on the SDK built before (SDK_BROWSER_PREBUILT=1). A
// missing Chromium fails it: install it with
// `pnpm exec playwright install chromium`.
import assert from "node:assert/strict";
import { after, before, test } from "node:test";
import { chromium } from "playwright";
import { build } from "../build.mjs";
import { startServer } from "../server.mjs";

let server;
let browser;

before(async () => {
  build({ sdk: process.env.SDK_BROWSER_PREBUILT !== "1" });
  server = await startServer({ port: 0 });
  browser = await chromium.launch();
});

after(async () => {
  await browser?.close();
  await server?.close();
});

/** The app in a fresh context, so on a fresh OPFS; `search` sets its options. */
const open = async (search = "") => {
  const context = await browser.newContext();
  const page = await context.newPage();
  const errors = [];
  page.on("pageerror", (error) => errors.push(error));
  await page.goto(`${server.url}/${search}`);
  await ready(page);
  return { page, errors };
};

/** Waits until the app has opened its database and shows its lists. */
const ready = (page) =>
  page.waitForSelector("body[data-ready='true']", { timeout: 30_000 });

/** The titles a list shows, in order. */
const titles = (page, list) =>
  page.locator(`#${list} li .title`).allTextContents();

/** Waits until a list shows exactly these titles. */
const shows = async (page, list, expected) => {
  await page.waitForFunction(
    ({ list, expected }) =>
      JSON.stringify(
        [...document.querySelectorAll(`#${list} li .title`)].map(
          (element) => element.textContent,
        ),
      ) === JSON.stringify(expected),
    { list, expected },
    { timeout: 10_000 },
  );
  assert.deepEqual(await titles(page, list), expected);
};

const add = async (page, title) => {
  await page.fill("#add input[name=title]", title);
  await page.press("#add input[name=title]", "Enter");
};

test("an added task shows in Open, in title order", async () => {
  const { page, errors } = await open();
  await shows(page, "open", []);
  await add(page, "water the plants");
  await add(page, "buy bread");
  await shows(page, "open", ["buy bread", "water the plants"]);
  await shows(page, "done", []);
  assert.deepEqual(errors, []);
});

test("marking a task done moves it to Done, and back", async () => {
  const { page, errors } = await open();
  await add(page, "buy bread");
  await add(page, "call Ana");
  await shows(page, "open", ["buy bread", "call Ana"]);
  await page
    .locator("#open li", { hasText: "buy bread" })
    .locator("input[type=checkbox]")
    .check();
  await shows(page, "open", ["call Ana"]);
  await shows(page, "done", ["buy bread"]);
  await page
    .locator("#done li", { hasText: "buy bread" })
    .locator("input[type=checkbox]")
    .uncheck();
  await shows(page, "done", []);
  await shows(page, "open", ["buy bread", "call Ana"]);
  assert.deepEqual(errors, []);
});

test("a deleted task leaves its list", async () => {
  const { page, errors } = await open();
  await add(page, "buy bread");
  await add(page, "call Ana");
  await shows(page, "open", ["buy bread", "call Ana"]);
  await page
    .locator("#open li", { hasText: "call Ana" })
    .getByRole("button", { name: "Delete" })
    .click();
  await shows(page, "open", ["buy bread"]);
  assert.deepEqual(errors, []);
});

test("a list shows one page and pages on with Show more", async () => {
  const { page, errors } = await open("?page=2");
  for (const title of ["c", "a", "b"]) await add(page, title);
  await shows(page, "open", ["a", "b"]);
  const more = page.locator("#open-more");
  assert.equal(await more.isVisible(), true);
  await more.click();
  await shows(page, "open", ["a", "b", "c"]);
  await page.waitForFunction(
    () => document.querySelector("#open-more")?.hidden === true,
  );
  assert.deepEqual(errors, []);
});

test("the tasks are still there after a reload, from OPFS", async () => {
  const { page, errors } = await open();
  await add(page, "buy bread");
  await shows(page, "open", ["buy bread"]);
  await page.reload();
  await ready(page);
  await shows(page, "open", ["buy bread"]);
  assert.deepEqual(errors, []);
});
