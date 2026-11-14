// Measures the landing page in Chromium (NOC-319): its first contentful
// paint, its Largest Contentful Paint and its load event, each the median
// of several loads in a fresh browser context, and every request under
// /demo/ with when it started. It reads the built site in site/dist, so
// it measures the page before and after a change the same way.
//
//   node scripts/site-demo/measure.mjs [--runs 9] [--throttle]
//
// --throttle runs each load with Chromium's CPU slowed 4x and the network
// at "Fast 3G", closer to a phone than this machine.
import { parseArgs } from "node:util";
import { chromium } from "playwright";
import { startServer } from "./server.mjs";

const median = (values) => {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.floor(sorted.length / 2)];
};

/** One load of `url`: its paint timings and the /demo/ requests. */
export const measureOnce = async (browser, url, { throttle = false } = {}) => {
  const context = await browser.newContext();
  const page = await context.newPage();
  if (throttle) {
    const cdp = await context.newCDPSession(page);
    await cdp.send("Emulation.setCPUThrottlingRate", { rate: 4 });
    await cdp.send("Network.enable");
    await cdp.send("Network.emulateNetworkConditions", {
      offline: false,
      latency: 150,
      downloadThroughput: (1.6 * 1024 * 1024) / 8,
      uploadThroughput: (750 * 1024) / 8,
    });
  }
  await page.addInitScript(() => {
    window.__lcp = 0;
    new PerformanceObserver((list) => {
      for (const entry of list.getEntries()) window.__lcp = entry.startTime;
    }).observe({ type: "largest-contentful-paint", buffered: true });
  });
  const start = Date.now();
  const demo = [];
  page.on("request", (request) => {
    const path = new URL(request.url()).pathname;
    if (path.startsWith("/demo/")) demo.push({ path, at: Date.now() - start });
  });
  await page.goto(url, { waitUntil: "load" });
  // LCP settles at the first input or once the page stops painting.
  await page.waitForTimeout(throttle ? 6000 : 2500);
  const timings = await page.evaluate(() => {
    const [nav] = performance.getEntriesByType("navigation");
    const fcp = performance
      .getEntriesByType("paint")
      .find((entry) => entry.name === "first-contentful-paint");
    return { fcp: fcp?.startTime ?? 0, lcp: window.__lcp, load: nav.loadEventEnd };
  });
  await context.close();
  return { ...timings, demo };
};

export const measure = async ({ runs = 9, throttle = false } = {}) => {
  const server = await startServer({ port: 0 });
  const browser = await chromium.launch();
  try {
    const all = [];
    for (let run = 0; run < runs; run++) {
      all.push(await measureOnce(browser, `${server.url}/`, { throttle }));
    }
    return {
      runs,
      throttle,
      fcp: median(all.map((one) => one.fcp)),
      lcp: median(all.map((one) => one.lcp)),
      load: median(all.map((one) => one.load)),
      demo: all[0].demo,
    };
  } finally {
    await browser.close();
    await server.close();
  }
};

if (process.argv[1] === new URL(import.meta.url).pathname) {
  const { values } = parseArgs({
    options: { runs: { type: "string" }, throttle: { type: "boolean" } },
  });
  const result = await measure({
    runs: Number(values.runs ?? 9),
    throttle: values.throttle ?? false,
  });
  console.log(JSON.stringify(result, null, 2));
}
