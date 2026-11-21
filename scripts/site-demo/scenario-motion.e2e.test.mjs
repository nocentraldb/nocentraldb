// The scenario's drawings in Chromium (NOC-384, owner, 2026-11-20): at a tablet's and
// a desktop's width each step's drawing starts when it comes into view and plays by
// itself, with no scrolling; under reduced motion it holds its still picture, and
// the scenario's pause control stops all of its motion. Through Playwright; not part
// of `pnpm gates`, as the other files here (site/scripts/story-motion.test.mjs and
// site/scripts/scenario-stacked.test.mjs pin the same in the gate).
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

const open = async (width, reducedMotion = "no-preference") => {
  const page = await browser.newPage({
    viewport: { width, height: 900 },
    reducedMotion,
  });
  await page.goto(`${server.url}/`, { waitUntil: "load" });
  return page;
};

/** Brings step i's figure to the middle of the screen, at once. */
const show = (page, i) =>
  page.evaluate((i) => {
    document
      .querySelectorAll("#story figure")
      [i].scrollIntoView({ block: "center", behavior: "instant" });
  }, i);

/** Each wide drawing's clock, whether it is held, and where its points stand. */
const clocks = (page) =>
  page.evaluate(() =>
    [...document.querySelectorAll("#story svg.sc--wide")].map((svg) => ({
      time: svg.getCurrentTime(),
      paused: svg.animationsPaused(),
      points: [...svg.querySelectorAll("circle.sc-pulse")].map((c) => {
        const r = c.getBoundingClientRect();
        return `${Math.round(r.x)},${Math.round(r.y)},${Math.round(r.width)}`;
      }),
    })),
  );

for (const width of [768, 1280]) {
  for (const i of [0, 1, 2, 3]) {
    test(`at ${width}px step ${i + 1}'s drawing plays by itself once it is in view`, async () => {
      const page = await open(width);
      await show(page, i);
      await page.waitForTimeout(300);
      const first = (await clocks(page))[i];
      await page.waitForTimeout(700); // no scrolling meanwhile
      const later = (await clocks(page))[i];
      assert.equal(later.paused, false, "plays");
      assert.ok(
        later.time - first.time > 0.5,
        `its clock went from ${first.time} to ${later.time}`,
      );
      assert.notDeepEqual(later.points, first.points, "its points moved");
      await page.close();
    });
  }

  test(`at ${width}px a drawing that has not come into view waits at its start`, async () => {
    const page = await open(width);
    await show(page, 0);
    await page.waitForTimeout(500);
    const last = (await clocks(page))[3];
    assert.equal(last.paused, true);
    assert.equal(last.time, 0);
    await page.close();
  });

  test(`at ${width}px the scenario's pause control stops all of its motion`, async () => {
    const page = await open(width);
    await show(page, 1);
    await page.waitForTimeout(300);
    const button = page.locator("#story figure").nth(1).locator("button");
    assert.equal(await button.isVisible(), true);
    await button.click();
    const held = await clocks(page);
    await page.waitForTimeout(500);
    const still = await clocks(page);
    for (const [k, c] of still.entries()) {
      assert.equal(c.paused, true, `drawing ${k + 1}`);
      assert.equal(c.time, held[k].time, `drawing ${k + 1}'s clock`);
    }
    assert.equal(await button.getAttribute("aria-pressed"), "true");
    await page.close();
  });

  test(`at ${width}px under reduced motion every drawing holds its still picture`, async () => {
    const page = await open(width, "reduce");
    await show(page, 2);
    await page.waitForTimeout(500);
    for (const c of await clocks(page)) {
      assert.equal(c.paused, true);
      assert.ok(Math.abs(c.time - 0.72) < 0.001, `held at ${c.time}`);
    }
    const shown = await page
      .locator("#story [data-story-pause]")
      .evaluateAll((all) => all.filter((b) => !b.hidden).length);
    assert.equal(shown, 0, "no pause control");
    await page.close();
  });
}

test("at 375px each step shows its schematic, and no pause control", async () => {
  const page = await open(375);
  await show(page, 1);
  const shown = await page.evaluate(() =>
    [...document.querySelectorAll("#story figure")].map((f) => ({
      phone: f.querySelector("svg.sc--phone").getBoundingClientRect().width > 0,
      button: f.querySelector("button").getBoundingClientRect().width > 0,
    })),
  );
  for (const s of shown) assert.deepEqual(s, { phone: true, button: false });
  await page.close();
});
