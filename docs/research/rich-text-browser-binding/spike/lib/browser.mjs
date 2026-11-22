// Opens pages of the spike in Chromium, the engine this container has.
import { chromium } from "playwright";

export const launch = () => chromium.launch();

/** A page of `context` on `url` with window.spike ready; its errors and warnings collected. */
export const openPage = async (context, url) => {
  const page = await context.newPage();
  const problems = [];
  page.on("pageerror", (e) => problems.push(`error: ${e.message}`));
  page.on("console", (m) => {
    if (m.type() === "error" || m.type() === "warning")
      problems.push(`${m.type()}: ${m.text()}`);
  });
  await page.goto(url);
  await page.waitForFunction(() => window.spikeReady === true);
  return { page, problems };
};

export const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** Composes `steps` with the IME through the DevTools protocol, then commits `commit`. */
export const compose = async (
  page,
  steps,
  commit,
  { stepMs = 120, holdMs = 0 } = {},
) => {
  const cdp = await page.context().newCDPSession(page);
  for (const text of steps) {
    await cdp.send("Input.imeSetComposition", {
      text,
      selectionStart: text.length,
      selectionEnd: text.length,
    });
    await sleep(stepMs);
  }
  if (holdMs) await sleep(holdMs);
  await cdp.send("Input.insertText", { text: commit });
  await cdp.detach();
};
