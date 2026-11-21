// The landing page's scenario in Chromium (NOC-380, owner, 2026-11-20): at a
// phone's, a tablet's and a desktop's width its four steps stand one under the
// other, nothing pins it or holds the wheel, and a phone shows each step's
// drawing as a small schematic without text. Through Playwright; not part of
// `pnpm gates`, as the other files here (site/scripts/scenario-stacked.test.mjs
// checks the built page in the gate).
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

const open = async (width) => {
  const page = await browser.newPage({ viewport: { width, height: 900 } });
  await page.goto(`${server.url}/`, { waitUntil: "load" });
  await page.evaluate(() => document.fonts.ready);
  return page;
};

/** Each step's box, and its title's, text's and drawing's, on the page. */
const steps = (page) =>
  page.evaluate(() =>
    [...document.querySelectorAll("#story li")].map((li) => {
      const box = (el) => {
        const r = el.getBoundingClientRect();
        return {
          top: r.top + scrollY,
          bottom: r.bottom + scrollY,
          width: r.width,
        };
      };
      return {
        step: box(li),
        parts: ["h3", "p", "figure"].map((s) => box(li.querySelector(s))),
      };
    }),
  );

for (const width of [375, 768, 1280]) {
  test(`at ${width}px the four steps stand one under the other`, async () => {
    const page = await open(width);
    const all = await steps(page);
    assert.equal(all.length, 4);
    for (const { parts } of all)
      for (const part of parts) assert.ok(part.width > 0, "shown");
    for (let i = 1; i < all.length; i += 1)
      assert.ok(
        all[i].step.top >= all[i - 1].step.bottom,
        `step ${i + 1} starts at ${all[i].step.top}, step ${i} ends at ${all[i - 1].step.bottom}`,
      );
    await page.close();
  });

  test(`at ${width}px the wheel scrolls through the scenario and nothing pins it`, async () => {
    const page = await open(width);
    await page.evaluate(() =>
      scrollTo(0, document.getElementById("story").offsetTop + 10),
    );
    await page.waitForTimeout(200);
    const before = await page.evaluate(() => ({
      y: scrollY,
      top: document.getElementById("story-h").getBoundingClientRect().top,
    }));
    await page.mouse.move(width / 2, 450);
    for (let i = 0; i < 10; i += 1) {
      await page.mouse.wheel(0, 100);
      await page.waitForTimeout(16);
    }
    await page.waitForTimeout(600);
    const after = await page.evaluate(() => ({
      y: scrollY,
      top: document.getElementById("story-h").getBoundingClientRect().top,
    }));
    const moved = after.y - before.y;
    assert.ok(moved >= 990, `the wheel moved the page ${moved} of 1000 px`);
    // the heading moves with the page: nothing holds the scenario on the screen
    assert.ok(
      Math.abs(before.top - after.top - moved) < 2,
      `the heading moved ${before.top - after.top} px, the page ${moved} px`,
    );
    await page.close();
  });
}

for (const width of [375, 639]) {
  test(`at ${width}px each step's drawing is the phone schematic and shows no text`, async () => {
    const page = await open(width);
    const shown = await page.evaluate(() =>
      [...document.querySelectorAll("#story figure")].map((figure) =>
        [...figure.querySelectorAll("svg.sc")]
          .filter((svg) => svg.getBoundingClientRect().width > 0)
          .map((svg) => ({
            layout: svg.classList.contains("sc--phone") ? "phone" : "wide",
            texts: svg.querySelectorAll("text").length,
          })),
      ),
    );
    assert.equal(shown.length, 4);
    for (const drawings of shown)
      assert.deepEqual(drawings, [{ layout: "phone", texts: 0 }]);
    await page.close();
  });
}

test("at 768px each step's drawing is the wide one", async () => {
  const page = await open(768);
  const layouts = await page.evaluate(() =>
    [...document.querySelectorAll("#story svg.sc")]
      .filter((svg) => svg.getBoundingClientRect().width > 0)
      .map((svg) => svg.classList.contains("sc--wide")),
  );
  assert.deepEqual(layouts, [true, true, true, true]);
  await page.close();
});
