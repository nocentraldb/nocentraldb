// The landing page's pinned scenario under momentum scrolling (NOC-360, the
// owner's report of 2026-11-19): on a Mac trackpad one flick skipped steps,
// since the four steps share one screen of scroll. One flick must stop at the
// next step, never jump past it, and under reduced motion, where the scenario
// is static, the page scrolls freely. A real trackpad cannot run in CI: a
// flick here is a run of wheel events 16 ms apart with falling deltas, as
// the trackpad's inertia sends them after the fingers leave. Chromium
// through Playwright; not part of `pnpm gates`, as the other files here.
import assert from "node:assert/strict";
import { after, before } from "node:test";
import { chromium } from "playwright";
import { build } from "./build.mjs";
import { startServer } from "./server.mjs";
import { endsWhenDone, test } from "./teardown.mjs";

endsWhenDone();

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

/** About 750 px of wheel deltas, falling off as inertia does. */
const FLICK = [120, 120, 120, 100, 80, 60, 45, 35, 25, 18, 12, 8, 5, 3, 2, 1];
/** About 1750 px: a hard flick, longer than the screen a step holds. */
const HARD = [
  200, 200, 200, 200, 180, 160, 140, 120, 100, 80, 60, 45, 30, 20, 10, 5,
];

/** One flick, down or (direction -1) up, then the time the page takes to come to rest. */
const flick = async (page, direction = 1, deltas = FLICK) => {
  await page.mouse.move(700, 450);
  for (const delta of deltas) {
    await page.mouse.wheel(0, direction * delta);
    await page.waitForTimeout(16);
  }
  await page.waitForTimeout(1200);
};

/** The open step of the scenario (0 to 3), or -1 when none is open. */
const openStep = (page) =>
  page.evaluate(() =>
    [...document.querySelectorAll("#story .mo-pc__step")].findIndex((step) =>
      step.classList.contains("is-on"),
    ),
  );

/** A page at 1440x900, scrolled to 100 px above the scenario. */
const above = async (reducedMotion) => {
  const page = await browser.newPage({
    viewport: { width: 1440, height: 900 },
    reducedMotion,
  });
  await page.goto(`${server.url}/`, { waitUntil: "load" });
  if (reducedMotion === "no-preference") {
    await page.waitForSelector("#story.is-live");
  }
  await page.evaluate(() => {
    const story = document.getElementById("story");
    scrollTo(0, story.getBoundingClientRect().top + scrollY - 100);
  });
  await page.waitForTimeout(500);
  return page;
};

test("each flick through the pinned scenario stops at the next step", async () => {
  const page = await above("no-preference");
  const seen = [];
  for (let i = 0; i < 4; i += 1) {
    await flick(page);
    seen.push(await openStep(page));
  }
  assert.deepEqual(seen, [0, 1, 2, 3]);
  await page.close();
});

/** The page's scroll position, and the scenario's top and bottom on the page. */
const where = (page) =>
  page.evaluate(() => {
    const story = document.getElementById("story").getBoundingClientRect();
    return {
      y: scrollY,
      top: story.top + scrollY,
      bottom: story.bottom + scrollY,
    };
  });

test("each hard flick stops at the next step too", async () => {
  const page = await above("no-preference");
  const seen = [];
  for (let i = 0; i < 4; i += 1) {
    await flick(page, 1, HARD);
    seen.push(await openStep(page));
  }
  assert.deepEqual(seen, [0, 1, 2, 3]);
  await page.close();
});

test("flicks carry the reader past the scenario, down and back up", async () => {
  const page = await above("no-preference");
  for (let i = 0; i < 10; i += 1) await flick(page);
  const below = await where(page);
  assert.ok(
    below.y > below.bottom,
    `stuck at ${below.y}, the scenario ends at ${below.bottom}`,
  );
  for (let i = 0; i < 12; i += 1) await flick(page, -1);
  const back = await where(page);
  assert.ok(
    back.y < back.top,
    `stuck at ${back.y}, the scenario starts at ${back.top}`,
  );
  await page.close();
});

test("under reduced motion the scenario is static and a flick scrolls freely", async () => {
  const page = await above("reduce");
  assert.equal(
    await page.evaluate(() =>
      document.getElementById("story").classList.contains("is-live"),
    ),
    false,
  );
  const start = await page.evaluate(() => scrollY);
  await flick(page);
  const moved = (await page.evaluate(() => scrollY)) - start;
  const total = FLICK.reduce((sum, delta) => sum + delta, 0);
  assert.ok(moved >= total - 10, `moved ${moved} of ${total} px`);
  await page.close();
});
