// Screenshots of the landing page for each candidate loud colour, at
// desktop 1440 px and mobile 390 px, deviceScaleFactor 2 (NOC-211).
// Usage: pnpm build && pnpm preview, then pnpm shots [base-url] in another shell.
import { mkdir } from "node:fs/promises";
import { chromium } from "playwright";

const base = process.argv[2] ?? "http://localhost:4321";
const accents = ["blue", "green", "magenta"];
const widths = { desktop: 1440, mobile: 390 };
const out = new URL("../drafts/", import.meta.url);
await mkdir(out, { recursive: true });

const browser = await chromium.launch();
for (const accent of accents) {
  for (const [name, width] of Object.entries(widths)) {
    const page = await browser.newPage({
      viewport: { width, height: 900 },
      deviceScaleFactor: 2,
    });
    await page.goto(`${base}/accent/${accent}/?shot`, {
      waitUntil: "networkidle",
    });
    await page.evaluate(() => document.fonts.ready);
    await page.waitForTimeout(900);
    const file = new URL(`${accent}-${name}.png`, out);
    await page.screenshot({ path: file.pathname, fullPage: true });
    console.log(file.pathname);
    await page.close();
  }
}
await browser.close();
